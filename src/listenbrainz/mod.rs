pub mod model;
mod response;

use std::collections::{BTreeSet, HashMap, HashSet, hash_map::Entry};
use std::time::{Duration, Instant};

use async_trait::async_trait;
use color_eyre::eyre::{Result, WrapErr, eyre};
use futures::stream::StreamExt;
use reqwest::header::HeaderMap;
use reqwest::{Response, StatusCode};
use serde::de::DeserializeOwned;
use serde_json::json;
use strsim::normalized_levenshtein;
use tokio::sync::Mutex;
use tracing::{debug, info, warn};

use self::model::{
    ListenBrainzCreateResponse, ListenBrainzFeedbackResponse, ListenBrainzLookupResponse,
    ListenBrainzPlaylistWrapper, ListenBrainzPlaylistsResponse, ListenBrainzStatusResponse,
    ListenBrainzTokenResponse, MusicBrainzRecording, MusicBrainzReleaseSearchResponse,
    MusicBrainzSearchResponse,
};
use self::response::{feedback_to_songs, is_valid_mbid, recording_to_song};
use crate::ConfigArgs;
use crate::music_api::{
    Album, MusicApi, MusicApiType, PLAYLIST_DESC, Playlist, Playlists, Song,
};
use crate::utils::{debug_response_json, generic_name_clean};

/// Songs are identified by their `MusicBrainz` recording MBID.
pub struct ListenBrainzApi {
    client: reqwest::Client,
    config: ConfigArgs,
    base_url: String,
    user_name: String,
    /// Memoised lookup results, keyed by [`song_cache_key`].
    cache: Mutex<HashMap<String, Option<Song>>>,
    mb_client: reqwest::Client,
    mb_last_request: Mutex<Option<Instant>>,
    /// Recording MBID -> ISRC. `None` means `MusicBrainz` has no ISRC for it.
    mb_isrc_cache: Mutex<HashMap<String, Option<String>>>,
    /// Derived from the rate-limit headers of the previous response.
    lb_next_allowed: Mutex<Option<Instant>>,
}

#[derive(Debug)]
enum HttpMethod<'a> {
    Get(&'a [(&'a str, String)]),
    Post(&'a serde_json::Value),
}

/// Which spelling of a song's metadata to send to the lookup endpoint.
///
/// The server normalises case, punctuation and accents itself, so the variants
/// only probe title suffixes and how collaborations are credited.
#[derive(Clone, Copy, Debug)]
enum LookupVariant {
    Raw,
    /// Through `clean_name`, dropping suffixes such as `" - Remastered 2011"`.
    Cleaned,
    /// Every artist, credited the `MusicBrainz` way: `"A feat. B & C"`.
    AllArtists,
}

const LOOKUP_VARIANTS: [LookupVariant; 3] = [
    LookupVariant::Raw,
    LookupVariant::Cleaned,
    LookupVariant::AllArtists,
];

#[derive(Debug)]
struct LookupItem {
    /// The server-side collision key this item normalises to.
    key: String,
    artist: String,
    recording: String,
    release: Option<String>,
}

impl ListenBrainzApi {
    pub const BASE_API: &'static str = "https://api.listenbrainz.org";

    const RECORDING_URI_PREFIX: &'static str = "https://musicbrainz.org/recording/";
    const ARTIST_URI_PREFIX: &'static str = "https://musicbrainz.org/artist/";
    const RELEASE_URI_PREFIX: &'static str = "https://musicbrainz.org/release/";
    const PLAYLIST_URI_PREFIX: &'static str = "https://listenbrainz.org/playlist/";
    const PLAYLIST_EXT_URI: &'static str = "https://musicbrainz.org/doc/jspf#playlist";

    /// `MAX_LOOKUPS_PER_POST` in `listenbrainz/webserver/views/metadata_api.py`.
    const MAX_LOOKUPS_PER_POST: usize = 50;
    /// `MAX_MAPPING_QUERY_LENGTH`, counted in characters, not bytes.
    const MAX_MAPPING_QUERY_LENGTH: usize = 250;
    /// `MAX_RECORDINGS_PER_ADD` in `listenbrainz/webserver/views/playlist_api.py`.
    const MAX_RECORDINGS_PER_ADD: usize = 100;
    /// `MAX_ITEMS_PER_GET`, the server-side clamp on feedback page size.
    const MAX_ITEMS_PER_GET: usize = 1000;
    const PLAYLISTS_PER_PAGE: usize = 100;
    /// Releases per batched recording lookup. Small, because one release
    /// returns a dozen-odd recordings against the 100-result cap.
    const RELEASES_PER_QUERY: usize = 8;
    const DURATION_TOLERANCE_MS: usize = 2000;
    const MUSICBRAINZ_API: &'static str = "https://musicbrainz.org/ws/2";
    /// Values (ISRCs, MBIDs, barcodes) OR-ed into a single Lucene query.
    const MAX_VALUES_PER_QUERY: usize = 100;
    /// `MusicBrainz` allows one request per second but still returns 503 on
    /// sustained bursts near that rate, so stay well under it.
    const MB_MIN_REQUEST_INTERVAL: Duration = Duration::from_millis(3000);
    const MB_MAX_ATTEMPTS: usize = 6;
    const MAX_WAIT: u64 = 120;
    const LB_MAX_ATTEMPTS: usize = 3;
    /// Requests left in the rate window at which we wait for it to reset.
    const LB_RATE_HEADROOM: u32 = 2;
    const RES_DEBUG_FILENAME: &'static str = MusicApiType::ListenBrainz.short_name();

    /// `MusicBrainz` requires an identifying User-Agent; generic ones get throttled.
    fn user_agent() -> String {
        format!(
            "{}/{} ( {} )",
            env!("CARGO_PKG_NAME"),
            env!("CARGO_PKG_VERSION"),
            env!("CARGO_PKG_REPOSITORY")
        )
    }

    pub async fn new(token: &str, base_url: &str, config: ConfigArgs) -> Result<Self> {
        let mut headers = HeaderMap::new();
        headers.insert("authorization", format!("Token {}", token).parse()?);
        headers.insert("content-type", "application/json".parse()?);
        headers.insert("user-agent", Self::user_agent().parse()?);

        let mut client = reqwest::ClientBuilder::new().default_headers(headers);
        if let Some(proxy) = &config.proxy {
            client = client
                .proxy(reqwest::Proxy::all(proxy)?)
                .danger_accept_invalid_certs(true);
        }
        let client = client.build()?;

        let mut mb_headers = HeaderMap::new();
        mb_headers.insert("user-agent", Self::user_agent().parse()?);
        let mut mb_client = reqwest::ClientBuilder::new().default_headers(mb_headers);
        if let Some(proxy) = &config.proxy {
            mb_client = mb_client
                .proxy(reqwest::Proxy::all(proxy)?)
                .danger_accept_invalid_certs(true);
        }
        let mb_client = mb_client.build()?;

        let mut api = Self {
            client,
            config,
            base_url: base_url.trim_end_matches('/').to_string(),
            user_name: String::new(),
            cache: Mutex::new(HashMap::new()),
            mb_client,
            mb_last_request: Mutex::new(None),
            mb_isrc_cache: Mutex::new(HashMap::new()),
            lb_next_allowed: Mutex::new(None),
        };

        let res: ListenBrainzTokenResponse = api
            .make_request_json("/1/validate-token", &HttpMethod::Get(&[]))
            .await?;
        if !res.valid {
            return Err(eyre!("invalid ListenBrainz token: {}", res.message));
        }
        let Some(user_name) = res.user_name else {
            return Err(eyre!("ListenBrainz returned no user name for this token"));
        };
        info!("authenticated to ListenBrainz as \"{}\"", user_name);
        api.user_name = user_name;

        Ok(api)
    }

    fn build_endpoint(&self, path: &str) -> String {
        format!("{}{}", self.base_url, path)
    }

    /// Wait for the rate-limit window to reset if the previous response said
    /// it was nearly spent.
    async fn lb_throttle(&self) {
        let mut next = self.lb_next_allowed.lock().await;
        if let Some(at) = *next {
            if let Some(remaining) = at.checked_duration_since(Instant::now()) {
                debug!("ListenBrainz rate window nearly spent, waiting {:?}", remaining);
                tokio::time::sleep(remaining).await;
            }
            *next = None;
        }
    }

    async fn note_rate_limit(&self, headers: &HeaderMap) {
        let get = |name: &str| -> Option<u64> {
            headers.get(name)?.to_str().ok()?.parse().ok()
        };
        let Some(remaining) = get("X-RateLimit-Remaining") else {
            return;
        };
        if remaining > u64::from(Self::LB_RATE_HEADROOM) {
            return;
        }
        let reset_in = get("X-RateLimit-Reset-In").unwrap_or(1);
        let mut next = self.lb_next_allowed.lock().await;
        *next = Some(Instant::now() + Duration::from_secs(reset_in + 1));
    }

    /// `ListenBrainz` sends no `Retry-After`, only its own rate-limit headers.
    async fn api_rate_wait(&self, res: &Response) -> Result<()> {
        let sleep_time = res
            .headers()
            .get("X-RateLimit-Reset-In")
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.parse::<u64>().ok())
            .unwrap_or(5);
        if sleep_time > Self::MAX_WAIT {
            return Err(eyre!(
                "ListenBrainz rate limit: reset in {sleep_time}s exceeds cap {}s, aborting (try again later)",
                Self::MAX_WAIT
            ));
        }
        debug!("API rate limit reached, sleeping for {} seconds", sleep_time);
        tokio::time::sleep(Duration::from_secs(sleep_time)).await;
        Ok(())
    }

    async fn make_request_json<T>(&self, path: &str, method: &HttpMethod<'_>) -> Result<T>
    where
        T: DeserializeOwned,
    {
        let endpoint = self.build_endpoint(path);
        // Retry rate limits and transient connection failures a bounded number
        // of times. The last 429 falls through and is reported as an error.
        let mut attempt = 0;
        let res = loop {
            attempt += 1;
            self.lb_throttle().await;
            let request = match method {
                HttpMethod::Get(p) => self.client.get(&endpoint).query(p),
                HttpMethod::Post(b) => self.client.post(&endpoint).json(b),
            };
            let retry = attempt < Self::LB_MAX_ATTEMPTS;
            match request.send().await {
                Ok(res) if res.status() == StatusCode::TOO_MANY_REQUESTS && retry => {
                    self.api_rate_wait(&res).await?;
                }
                Ok(res) => {
                    self.note_rate_limit(res.headers()).await;
                    break res;
                }
                Err(e) if (e.is_connect() || e.is_timeout()) && retry => {
                    debug!(
                        "ListenBrainz attempt {}/{} failed, retrying: {}",
                        attempt,
                        Self::LB_MAX_ATTEMPTS,
                        e
                    );
                    tokio::time::sleep(Duration::from_secs(attempt as u64)).await;
                }
                // Name the host: reqwest's "unexpected EOF" alone reads like a
                // configuration mistake.
                Err(e) if e.is_connect() || e.is_timeout() => {
                    return Err(eyre!(e)).wrap_err(format!(
                        "could not reach ListenBrainz at {}: check your network connection, \
                         or whether the service is up (https://listenbrainz.org)",
                        self.base_url
                    ));
                }
                Err(e) => return Err(e.into()),
            }
        };
        let status = res.status();

        // Parse as `Value` first so a failed request surfaces the body's `error`
        // message, which names the offending track index.
        let val: serde_json::Value =
            debug_response_json(&self.config, res, Self::RES_DEBUG_FILENAME).await?;
        if !status.is_success() {
            let msg = val
                .get("error")
                .and_then(serde_json::Value::as_str)
                .unwrap_or("<no error message>");
            return Err(eyre!("ListenBrainz API error {} on {}: {}", status, path, msg));
        }

        Ok(serde_json::from_value(val)?)
    }

    /// Send the same feedback for every song: a score of 1 loves the
    /// recording, 0 clears the feedback. The endpoint accepts exactly one
    /// recording per request, so a few are kept in flight.
    async fn send_feedback(&self, songs: &[Song], score: i8) -> Result<()> {
        let path = "/1/feedback/recording-feedback";
        let bodies: Vec<serde_json::Value> = with_valid_mbid(songs)
            .into_iter()
            .map(|s| json!({ "recording_mbid": s.id, "score": score }))
            .collect();
        let methods: Vec<HttpMethod> = bodies.iter().map(HttpMethod::Post).collect();
        // NOTE: not `.map()`: an async fn called from a closure isn't generic
        // enough over the reference lifetime.
        let mut requests = Vec::with_capacity(methods.len());
        for method in &methods {
            requests.push(self.make_request_json::<ListenBrainzStatusResponse>(path, method));
        }
        let mut stream = futures::stream::iter(requests).buffered(5);
        while let Some(res) = stream.next().await {
            res?;
        }
        Ok(())
    }

    /// Values are quoted: MBIDs contain hyphens, which Lucene parses as
    /// operators, and an unquoted batch silently matches nothing.
    fn mb_batch_query(field: &str, values: &[&str]) -> String {
        values
            .iter()
            .map(|v| format!("{}:\"{}\"", field, v))
            .collect::<Vec<String>>()
            .join(" OR ")
    }

    /// One `MusicBrainz` search. The throttle lock is held for the whole
    /// request, so only one `MusicBrainz` request is ever in flight.
    async fn mb_request<T>(&self, entity: &str, query: &str) -> Result<T>
    where
        T: DeserializeOwned,
    {
        let mut last = self.mb_last_request.lock().await;
        if let Some(prev) = *last
            && let Some(remaining) = Self::MB_MIN_REQUEST_INTERVAL.checked_sub(prev.elapsed())
        {
            tokio::time::sleep(remaining).await;
        }

        let res = self
            .mb_client
            .get(format!("{}/{}", Self::MUSICBRAINZ_API, entity))
            .query(&[("query", query), ("fmt", "json"), ("limit", "100")])
            .send()
            .await?;

        // Space requests from the *end* of the previous one.
        *last = Some(Instant::now());
        drop(last);

        let status = res.status();
        if status == StatusCode::SERVICE_UNAVAILABLE {
            return Err(eyre!(
                "MusicBrainz rate limit hit (503): exceeding one request per second"
            ));
        }
        if !status.is_success() {
            return Err(eyre!("MusicBrainz API error {}", status));
        }
        debug_response_json(&self.config, res, "musicbrainz").await
    }

    /// [`Self::mb_request`] with bounded retries and exponential backoff.
    async fn mb_search<T>(&self, entity: &str, query: &str) -> Result<T>
    where
        T: DeserializeOwned,
    {
        let mut attempt = 1;
        loop {
            match self.mb_request(entity, query).await {
                Ok(res) => return Ok(res),
                Err(e) if attempt < Self::MB_MAX_ATTEMPTS => {
                    let backoff = Duration::from_secs(1 << attempt);
                    debug!(
                        "MusicBrainz attempt {}/{} failed, retrying in {:?}: {:#}",
                        attempt,
                        Self::MB_MAX_ATTEMPTS,
                        backoff,
                        e
                    );
                    tokio::time::sleep(backoff).await;
                    attempt += 1;
                }
                Err(e) => {
                    return Err(e).wrap_err(format!(
                        "MusicBrainz {} search failed {} times, try again later \
                         (https://status.metabrainz.org)",
                        entity,
                        Self::MB_MAX_ATTEMPTS
                    ));
                }
            }
        }
    }

    /// Resolve songs by ISRC against `MusicBrainz`, filling `out` in place.
    async fn resolve_by_isrc(&self, songs: &[Song], out: &mut [Option<Song>]) -> Result<()> {
        // One ISRC can be shared by several songs; ask for each only once.
        let mut by_isrc: HashMap<&str, Vec<usize>> = HashMap::new();
        for (i, song) in songs.iter().enumerate() {
            if out[i].is_none()
                && let Some(isrc) = song.isrc.as_deref()
            {
                by_isrc.entry(isrc).or_default().push(i);
            }
        }
        if by_isrc.is_empty() {
            return Ok(());
        }
        let unique: Vec<&str> = by_isrc.keys().copied().collect();

        let mut matched = 0;
        // A truncated response pushes its two halves back onto the queue.
        let mut queue: Vec<Vec<&str>> = unique
            .chunks(Self::MAX_VALUES_PER_QUERY)
            .map(<[&str]>::to_vec)
            .collect();
        while let Some(chunk) = queue.pop() {
            let query = Self::mb_batch_query("isrc", &chunk);
            let res: MusicBrainzSearchResponse = self.mb_search("recording", &query).await?;

            if res.count > res.recordings.len() && chunk.len() > 1 {
                let mid = chunk.len() / 2;
                debug!(
                    "MusicBrainz truncated {} results to {}, splitting the batch",
                    res.count,
                    res.recordings.len()
                );
                queue.push(chunk[..mid].to_vec());
                queue.push(chunk[mid..].to_vec());
            }

            // Even a truncated response holds exact ISRC matches, so use it.
            matched += match_isrc_results(&res.recordings, &by_isrc, out);
        }

        if matched > 0 {
            debug!("resolved {} songs by ISRC via MusicBrainz", matched);
        }
        Ok(())
    }

    /// Fill in the ISRC of every song that lacks one, from `MusicBrainz`.
    ///
    /// `ListenBrainz` never reports ISRCs, and without one a song only compares
    /// fuzzily against other platforms.
    async fn enrich_isrcs(&self, songs: Vec<&mut Song>) -> Result<()> {
        let mut wanted: Vec<&mut Song> = songs
            .into_iter()
            .filter(|s| s.isrc.is_none() && is_valid_mbid(&s.id))
            .collect();
        if wanted.is_empty() {
            return Ok(());
        }

        let mut todo: Vec<String> = Vec::new();
        {
            let cache = self.mb_isrc_cache.lock().await;
            let mut seen = HashSet::new();
            for song in &wanted {
                if !cache.contains_key(&song.id) && seen.insert(song.id.clone()) {
                    todo.push(song.id.clone());
                }
            }
        }

        for chunk in todo.chunks(Self::MAX_VALUES_PER_QUERY) {
            let mbids: Vec<&str> = chunk.iter().map(String::as_str).collect();
            let query = Self::mb_batch_query("rid", &mbids);
            let found: MusicBrainzSearchResponse = self.mb_search("recording", &query).await?;

            let mut cache = self.mb_isrc_cache.lock().await;
            for recording in &found.recordings {
                // A recording can carry several ISRCs (regional releases,
                // reissues), and other platforms often know only one of
                // them. Attach none rather than guess: an ISRC that
                // differs from theirs makes the songs compare unequal,
                // while no ISRC falls back to comparing metadata.
                let isrc = match recording.isrcs.as_slice() {
                    [isrc] => Some(isrc.clone()),
                    _ => None,
                };
                cache.insert(recording.id.clone(), isrc);
            }
            // Remember the misses too, so they are not asked for again.
            for mbid in chunk {
                cache.entry(mbid.clone()).or_insert(None);
            }
        }

        let cache = self.mb_isrc_cache.lock().await;
        let mut filled = 0;
        for song in &mut wanted {
            if let Some(Some(isrc)) = cache.get(&song.id) {
                song.isrc = Some(isrc.clone());
                filled += 1;
            }
        }
        let missing = wanted.len() - filled;
        if filled > 0 {
            debug!("attached ISRCs to {} ListenBrainz songs", filled);
        }
        if missing > 0 {
            debug!("{} ListenBrainz songs have no ISRC in MusicBrainz", missing);
        }
        Ok(())
    }

    /// Resolve songs by release barcode, for songs whose ISRC `MusicBrainz`
    /// doesn't know. The barcode identifies the release and duration picks the
    /// track within it, so this never overrides an ISRC match.
    async fn resolve_by_barcode(&self, songs: &[Song], out: &mut [Option<Song>]) -> Result<()> {
        let mut by_upc: HashMap<&str, Vec<usize>> = HashMap::new();
        for (i, song) in songs.iter().enumerate() {
            if out[i].is_none()
                && let Some(album) = &song.album
                && let Some(upc) = album.upc.as_deref()
                && !upc.is_empty()
            {
                by_upc.entry(upc).or_default().push(i);
            }
        }
        if by_upc.is_empty() {
            return Ok(());
        }

        // barcode -> release MBIDs
        let upcs: Vec<&str> = by_upc.keys().copied().collect();
        let mut release_to_songs: HashMap<String, Vec<usize>> = HashMap::new();
        for chunk in upcs.chunks(Self::MAX_VALUES_PER_QUERY) {
            let query = Self::mb_batch_query("barcode", chunk);
            let found: MusicBrainzReleaseSearchResponse =
                self.mb_search("release", &query).await?;
            for release in &found.releases {
                // Barcodes appear with varying leading zeros (UPC-12 vs
                // EAN-13 vs GTIN-14), so compare them normalised.
                let Some(barcode) = &release.barcode else {
                    continue;
                };
                let normalised = barcode.trim_start_matches('0');
                for (upc, indices) in &by_upc {
                    if upc.trim_start_matches('0') == normalised {
                        release_to_songs
                            .entry(release.id.clone())
                            .or_default()
                            .extend(indices.iter().copied());
                    }
                }
            }
        }
        if release_to_songs.is_empty() {
            return Ok(());
        }

        // release -> recordings, then pick the track by duration
        let releases: Vec<String> = release_to_songs.keys().cloned().collect();
        let mut matched = 0;
        let mut queue: Vec<Vec<String>> = releases
            .chunks(Self::RELEASES_PER_QUERY)
            .map(<[String]>::to_vec)
            .collect();
        while let Some(chunk) = queue.pop() {
            let refs: Vec<&str> = chunk.iter().map(String::as_str).collect();
            let query = Self::mb_batch_query("reid", &refs);
            let found: MusicBrainzSearchResponse = self.mb_search("recording", &query).await?;
            // Unlike ISRC results, a truncated track list is unusable: the
            // duration pick assumes it sees every track of the release.
            if found.count > found.recordings.len() && chunk.len() > 1 {
                let mid = chunk.len() / 2;
                queue.push(chunk[..mid].to_vec());
                queue.push(chunk[mid..].to_vec());
                continue;
            }
            matched += pick_by_duration(&found.recordings, &release_to_songs, songs, out);
        }

        if matched > 0 {
            debug!("resolved {} songs by release barcode", matched);
        }
        Ok(())
    }

    /// Resolve songs to `MusicBrainz` recordings. The returned vector is
    /// positionally aligned with `songs`.
    async fn resolve(&self, songs: &[Song]) -> Result<Vec<Option<Song>>> {
        let mut out: Vec<Option<Song>> = vec![None; songs.len()];

        self.resolve_by_isrc(songs, &mut out).await?;
        self.resolve_by_barcode(songs, &mut out).await?;

        // Fallback: ListenBrainz's metadata mapper, only for songs with no ISRC.
        for variant in LOOKUP_VARIANTS {
            let pending = name_fallback_candidates(songs, &out);
            if pending.is_empty() {
                break;
            }

            let (items, key_to_idx) = build_lookup_items(&pending, variant);
            if items.is_empty() {
                continue;
            }

            for chunk in items.chunks(Self::MAX_LOOKUPS_PER_POST) {
                let recordings: Vec<serde_json::Value> = chunk
                    .iter()
                    .map(|item| {
                        let mut obj = json!({
                            "artist_name": item.artist,
                            "recording_name": item.recording,
                        });
                        if let Some(release) = &item.release {
                            obj["release_name"] = json!(release);
                        }
                        obj
                    })
                    .collect();
                let body = json!({ "recordings": recordings });

                let res: Vec<ListenBrainzLookupResponse> = self
                    .make_request_json("/1/metadata/lookup/", &HttpMethod::Post(&body))
                    .await?;
                realign(songs, chunk, &key_to_idx, res, &mut out);
            }
        }

        // Songs matched by name carry no ISRC; give them one.
        self.enrich_isrcs(out.iter_mut().filter_map(Option::as_mut).collect())
            .await?;

        Ok(out)
    }
}

/// Approximate the server's collision key: `re.sub(r'\W+', '', s).lower()` then
/// `unidecode`. Only used client-side, so exact parity is not needed.
fn normalize_lookup_key(s: &str) -> String {
    deaccent(s)
        .chars()
        .filter(|c| c.is_alphanumeric() || *c == '_')
        .flat_map(char::to_lowercase)
        .collect()
}

/// Fold the common Latin-1 accented characters down to ASCII.
fn deaccent(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            'à'..='å' | 'À'..='Å' => out.push('a'),
            'æ' | 'Æ' => out.push_str("ae"),
            'ç' | 'Ç' => out.push('c'),
            'è'..='ë' | 'È'..='Ë' => out.push('e'),
            'ì'..='ï' | 'Ì'..='Ï' => out.push('i'),
            'ñ' | 'Ñ' => out.push('n'),
            'ò'..='ö' | 'Ò'..='Ö' | 'ø' | 'Ø' => out.push('o'),
            'ù'..='ü' | 'Ù'..='Ü' => out.push('u'),
            'ý' | 'ÿ' | 'Ý' => out.push('y'),
            'ß' => out.push_str("ss"),
            'œ' | 'Œ' => out.push_str("oe"),
            _ => out.push(c),
        }
    }
    out
}

/// Stable across lookup variants. The ISRC is part of the key: songs sharing
/// metadata (e.g. explicit and clean versions) must not share a result.
fn song_cache_key(song: &Song) -> String {
    let artist = song.artists.first().map_or("", |a| a.name.as_str());
    let album = song.album.as_ref().map_or("", |a| a.name.as_str());
    let isrc = song.isrc.as_deref().unwrap_or("");
    format!(
        "{}|{}",
        normalize_lookup_key(&format!("{}{}{}", artist, song.name, album)),
        isrc
    )
}

/// `None` when this song cannot usefully be looked up with this variant.
fn lookup_fields(song: &Song, variant: LookupVariant) -> Option<(String, String, Option<String>)> {
    let (artist, recording, release) = match variant {
        LookupVariant::Raw => (
            song.artists.first()?.name.clone(),
            song.name.clone(),
            song.album.as_ref().map(|a| a.name.clone()),
        ),
        LookupVariant::Cleaned => (
            song.artists.first()?.clean_name(),
            song.clean_name(),
            song.album.as_ref().map(Album::clean_name),
        ),
        LookupVariant::AllArtists => {
            // identical to `Raw` for single-artist songs
            if song.artists.len() < 2 {
                return None;
            }
            let names: Vec<&str> = song.artists.iter().map(|a| a.name.as_str()).collect();
            let (last, middle) = names[1..].split_last()?;
            let featured = if middle.is_empty() {
                (*last).to_string()
            } else {
                format!("{} & {}", middle.join(", "), last)
            };
            (
                format!("{} feat. {}", names[0], featured),
                song.name.clone(),
                None,
            )
        }
    };

    // An empty artist or recording makes the server reject the whole chunk.
    if artist.trim().is_empty() || recording.trim().is_empty() {
        return None;
    }
    Some((artist, recording, release))
}

/// Enforce the server's per-item character budget by dropping the release
/// name. Truncating would guarantee a miss, since matching is exact.
fn fit_query_budget(
    artist: String,
    recording: String,
    release: Option<String>,
) -> Option<(String, String, Option<String>)> {
    let count = |s: &str| s.chars().count();
    let base = count(&artist) + count(&recording);

    if base + release.as_deref().map_or(0, count) <= ListenBrainzApi::MAX_MAPPING_QUERY_LENGTH {
        return Some((artist, recording, release));
    }
    if base <= ListenBrainzApi::MAX_MAPPING_QUERY_LENGTH {
        return Some((artist, recording, None));
    }
    None
}

/// Build the deduplicated request items for one pass, plus a map from each
/// item's key back to every source index that wants it.
///
/// The server keys its results by the normalised lookup string, so two
/// colliding items in one request would leave the earlier one unmatched.
fn build_lookup_items(
    songs: &[(usize, &Song)],
    variant: LookupVariant,
) -> (Vec<LookupItem>, HashMap<String, Vec<usize>>) {
    let mut items: Vec<LookupItem> = Vec::new();
    let mut key_to_idx: HashMap<String, Vec<usize>> = HashMap::new();

    for (idx, song) in songs {
        let Some((artist, recording, release)) = lookup_fields(song, variant) else {
            continue;
        };
        let Some((artist, recording, release)) = fit_query_budget(artist, recording, release)
        else {
            warn!(
                "song metadata exceeds the ListenBrainz lookup budget, skipping: {}",
                song
            );
            continue;
        };

        // Same order and no separator, matching the server's `get_lookup_string`.
        let key = normalize_lookup_key(&format!(
            "{}{}{}",
            artist,
            recording,
            release.as_deref().unwrap_or("")
        ));

        match key_to_idx.entry(key.clone()) {
            Entry::Occupied(mut e) => e.get_mut().push(*idx),
            Entry::Vacant(e) => {
                e.insert(vec![*idx]);
                items.push(LookupItem {
                    key,
                    artist,
                    recording,
                    release,
                });
            }
        }
    }

    (items, key_to_idx)
}

/// Scatter a lookup response back onto the output slots, by each entry's `index`.
/// The mapper can return another recording of the same title (a cover, a
/// live version), so a result is kept only if it matches the source song.
fn realign(
    songs: &[Song],
    chunk: &[LookupItem],
    key_to_idx: &HashMap<String, Vec<usize>>,
    res: Vec<ListenBrainzLookupResponse>,
    out: &mut [Option<Song>],
) {
    for entry in res {
        let index = entry.index;
        let Some(item) = chunk.get(index) else {
            warn!("ListenBrainz lookup returned an out-of-range index: {}", index);
            continue;
        };
        if entry.recording_mbid.is_none() {
            continue;
        }
        let song: Song = match entry.try_into() {
            Ok(s) => s,
            Err(e) => {
                debug!("skipping unusable lookup result: {}", e);
                continue;
            }
        };
        let Some(indices) = key_to_idx.get(&item.key) else {
            continue;
        };
        for &i in indices {
            if out[i].is_none() {
                if songs[i].compare(&song) {
                    out[i] = Some(song.clone());
                } else {
                    debug!("rejecting lookup result {} for {}", song, songs[i]);
                }
            }
        }
    }
}

/// For each song, pick the recording on its matched release with the same
/// duration. Titles legitimately differ across platforms, so they only break
/// ties, and anything still ambiguous is left unresolved.
///
/// Returns how many songs were filled in.
fn pick_by_duration(
    recordings: &[MusicBrainzRecording],
    release_to_songs: &HashMap<String, Vec<usize>>,
    songs: &[Song],
    out: &mut [Option<Song>],
) -> usize {
    let mut matched = 0;

    for (release_id, indices) in release_to_songs {
        let candidates: Vec<&MusicBrainzRecording> = recordings
            .iter()
            .filter(|r| r.releases.iter().any(|rel| &rel.id == release_id))
            .collect();
        if candidates.is_empty() {
            continue;
        }

        for &i in indices {
            if out[i].is_some() {
                continue;
            }
            let song = &songs[i];

            let close: Vec<&&MusicBrainzRecording> = candidates
                .iter()
                .filter(|r| {
                    r.length
                        .is_some_and(|l| l.abs_diff(song.duration_ms) <= ListenBrainzApi::DURATION_TOLERANCE_MS)
                })
                .collect();

            let chosen = match close.len() {
                0 => continue,
                1 => close[0],
                _ => {
                    // same-length tracks: take the title only if one clearly wins
                    let target = generic_name_clean(&song.name);
                    let mut scored: Vec<(f64, &&MusicBrainzRecording)> = close
                        .iter()
                        .map(|r| {
                            let title = generic_name_clean(r.title.as_deref().unwrap_or(""));
                            (normalized_levenshtein(&target, &title).abs(), *r)
                        })
                        .collect();
                    scored.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
                    let clear_winner = scored[0].0 >= 0.8
                        && (scored.len() < 2 || scored[0].0 > scored[1].0 + f64::EPSILON);
                    if !clear_winner {
                        debug!(
                            "ambiguous barcode match for \"{}\", leaving unresolved",
                            song
                        );
                        continue;
                    }
                    scored[0].1
                }
            };

            let mut isrcs = chosen.isrcs.clone();
            isrcs.sort();
            out[i] = Some(recording_to_song(chosen, isrcs.first().map(String::as_str)));
            matched += 1;
        }
    }

    matched
}

/// Songs still eligible for the name-matching fallback: unresolved and with no
/// ISRC. A song whose ISRC failed to resolve is left unmatched on purpose, as
/// name matching is what yields wrong remixes and live versions.
fn name_fallback_candidates<'a>(
    songs: &'a [Song],
    out: &[Option<Song>],
) -> Vec<(usize, &'a Song)> {
    songs
        .iter()
        .enumerate()
        .filter(|(i, s)| out[*i].is_none() && s.isrc.is_none())
        .collect()
}

/// Scatter `MusicBrainz` recordings onto the songs that asked for their ISRCs.
/// Recordings arrive sorted by score, so the first one carrying an ISRC wins it.
///
/// Returns how many songs were filled in.
fn match_isrc_results(
    recordings: &[MusicBrainzRecording],
    by_isrc: &HashMap<&str, Vec<usize>>,
    out: &mut [Option<Song>],
) -> usize {
    let mut matched = 0;
    for recording in recordings {
        for isrc in &recording.isrcs {
            let Some(indices) = by_isrc.get(isrc.as_str()) else {
                continue;
            };
            let song = recording_to_song(recording, Some(isrc));
            for &i in indices {
                if out[i].is_none() {
                    out[i] = Some(song.clone());
                    matched += 1;
                }
            }
        }
    }
    matched
}

/// The songs with a valid recording MBID: a single malformed one makes the
/// server reject a whole batch.
fn with_valid_mbid(songs: &[Song]) -> Vec<&Song> {
    songs
        .iter()
        .filter(|s| {
            let valid = is_valid_mbid(&s.id);
            if !valid {
                warn!("song has no valid MusicBrainz recording id, skipping: {}", s);
            }
            valid
        })
        .collect()
}

/// `title` must be non-empty and `public` a JSON boolean, or the server rejects it.
fn create_playlist_body(name: &str, public: bool) -> serde_json::Value {
    let mut extension = serde_json::Map::new();
    extension.insert(
        ListenBrainzApi::PLAYLIST_EXT_URI.to_string(),
        json!({ "public": public }),
    );
    json!({
        "playlist": {
            "title": name,
            "annotation": PLAYLIST_DESC,
            "extension": extension,
        }
    })
}

/// `identifier` is sent as an array; the bare-string form is deprecated.
fn add_tracks_body(songs: &[&Song]) -> serde_json::Value {
    let tracks: Vec<serde_json::Value> = songs
        .iter()
        .map(|s| {
            json!({
                "identifier": [format!("{}{}", ListenBrainzApi::RECORDING_URI_PREFIX, s.id)],
            })
        })
        .collect();
    json!({ "playlist": { "track": tracks } })
}

/// Coalesce sorted indices into contiguous runs, highest first, so positional
/// deletes keep the lower indices valid.
fn contiguous_runs_desc(indices: &BTreeSet<usize>) -> Vec<(usize, usize)> {
    let mut runs: Vec<(usize, usize)> = Vec::new();
    for &i in indices {
        match runs.last_mut() {
            Some((start, count)) if *start + *count == i => *count += 1,
            _ => runs.push((i, 1)),
        }
    }
    runs.reverse();
    runs
}

#[async_trait]
impl MusicApi for ListenBrainzApi {
    fn api_type(&self) -> MusicApiType {
        MusicApiType::ListenBrainz
    }

    fn country_code(&self) -> Option<&str> {
        // MusicBrainz is a global catalogue
        None
    }

    async fn create_playlist(&self, name: &str, public: bool) -> Result<Playlist> {
        let body = create_playlist_body(name, public);

        let res: ListenBrainzCreateResponse = self
            .make_request_json("/1/playlist/create", &HttpMethod::Post(&body))
            .await?;

        Ok(Playlist {
            id: res.playlist_mbid,
            name: name.to_string(),
            songs: vec![],
        })
    }

    async fn get_playlists_info(&self) -> Result<Vec<Playlist>> {
        let path = format!("/1/user/{}/playlists", self.user_name);
        let mut all = vec![];
        let mut offset = 0;

        loop {
            let params = [
                ("count", Self::PLAYLISTS_PER_PAGE.to_string()),
                ("offset", offset.to_string()),
            ];
            let res: ListenBrainzPlaylistsResponse = self
                .make_request_json(&path, &HttpMethod::Get(&params))
                .await?;

            let total = res.playlist_count;
            let received = res.playlists.len();
            let playlists: Playlists = res.try_into()?;
            all.extend(playlists.0);

            offset += received;
            if received == 0 || offset >= total {
                break;
            }
        }

        Ok(all)
    }

    async fn get_playlist_songs(&self, id: &str) -> Result<Vec<Song>> {
        let path = format!("/1/playlist/{}", id);
        let params = [("fetch_metadata", "true".to_string())];
        let res: ListenBrainzPlaylistWrapper = self
            .make_request_json(&path, &HttpMethod::Get(&params))
            .await?;
        let mut playlist: Playlist = res.playlist.try_into()?;
        self.enrich_isrcs(playlist.songs.iter_mut().collect()).await?;
        Ok(playlist.songs)
    }

    async fn add_songs_to_playlist(&self, playlist: &mut Playlist, songs: &[Song]) -> Result<()> {
        if songs.is_empty() {
            return Ok(());
        }

        let valid = with_valid_mbid(songs);
        if valid.is_empty() {
            return Ok(());
        }

        let path = format!("/1/playlist/{}/item/add", playlist.id);
        for chunk in valid.chunks(Self::MAX_RECORDINGS_PER_ADD) {
            let body = add_tracks_body(chunk);
            self.make_request_json::<ListenBrainzStatusResponse>(&path, &HttpMethod::Post(&body))
                .await?;
        }

        // Positional deletes depend on `playlist.songs` tracking server order.
        for song in valid {
            playlist.songs.push(song.clone());
        }
        Ok(())
    }

    async fn remove_songs_from_playlist(
        &self,
        playlist: &mut Playlist,
        songs: &[Song],
    ) -> Result<()> {
        let mut indices = BTreeSet::new();
        for song in songs {
            if let Some(i) = playlist.songs.iter().position(|s| s.id == song.id) {
                indices.insert(i);
            }
        }
        if indices.is_empty() {
            return Ok(());
        }

        let path = format!("/1/playlist/{}/item/delete", playlist.id);
        for (index, count) in contiguous_runs_desc(&indices) {
            let body = json!({ "index": index, "count": count });
            self.make_request_json::<ListenBrainzStatusResponse>(&path, &HttpMethod::Post(&body))
                .await?;
            playlist.songs.drain(index..index + count);
        }
        Ok(())
    }

    async fn delete_playlist(&self, playlist: Playlist) -> Result<()> {
        let path = format!("/1/playlist/{}/delete", playlist.id);
        self.make_request_json::<ListenBrainzStatusResponse>(
            &path,
            &HttpMethod::Post(&json!({})),
        )
        .await?;
        Ok(())
    }

    async fn prefetch_searches(&self, songs: &[Song]) -> Result<()> {
        let mut todo: Vec<Song> = vec![];
        {
            let cache = self.cache.lock().await;
            let mut seen = HashSet::new();
            for song in songs {
                let key = song_cache_key(song);
                if !cache.contains_key(&key) && seen.insert(key) {
                    todo.push(song.clone());
                }
            }
        }
        if todo.is_empty() {
            return Ok(());
        }

        let known = songs.len() - todo.len();
        if known > 0 {
            info!(
                "resolving {} new songs against MusicBrainz ({} already resolved)...",
                todo.len(),
                known
            );
        } else {
            info!("resolving {} songs against MusicBrainz...", todo.len());
        }
        let results = self.resolve(&todo).await?;
        let matched = results.iter().filter(|r| r.is_some()).count();
        info!("resolved {}/{} songs", matched, todo.len());

        let mut cache = self.cache.lock().await;
        for (song, res) in todo.iter().zip(results) {
            cache.insert(song_cache_key(song), res);
        }
        Ok(())
    }

    async fn search_song(&self, song: &Song) -> Result<Option<Song>> {
        let key = song_cache_key(song);
        {
            let cache = self.cache.lock().await;
            if let Some(hit) = cache.get(&key) {
                return Ok(hit.clone());
            }
        }

        let res = self
            .resolve(std::slice::from_ref(song))
            .await?
            .into_iter()
            .next()
            .flatten();

        self.cache.lock().await.insert(key, res.clone());
        Ok(res)
    }

    async fn search_songs(&self, songs: &[Song]) -> Result<Vec<Option<Song>>> {
        self.prefetch_searches(songs).await?;
        let cache = self.cache.lock().await;
        Ok(songs
            .iter()
            .map(|s| cache.get(&song_cache_key(s)).cloned().flatten())
            .collect())
    }

    async fn add_likes(&self, songs: &[Song]) -> Result<()> {
        self.send_feedback(songs, 1).await
    }

    async fn remove_likes(&self, songs: &[Song]) -> Result<()> {
        self.send_feedback(songs, 0).await
    }

    async fn get_likes(&self) -> Result<Vec<Song>> {
        let path = format!("/1/feedback/user/{}/get-feedback", self.user_name);
        let mut all = vec![];
        let mut offset = 0;

        loop {
            let params = [
                ("score", "1".to_string()),
                ("metadata", "true".to_string()),
                ("count", Self::MAX_ITEMS_PER_GET.to_string()),
                ("offset", offset.to_string()),
            ];
            let res: ListenBrainzFeedbackResponse = self
                .make_request_json(&path, &HttpMethod::Get(&params))
                .await?;

            let total = res.total_count;
            let received = res.feedback.len();
            all.extend(feedback_to_songs(res.feedback).0);

            offset += received;
            if received == 0 || offset >= total {
                break;
            }
        }

        self.enrich_isrcs(all.iter_mut().collect()).await?;
        Ok(all)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::music_api::Artist;

    fn song(name: &str, artists: &[&str], album: Option<&str>) -> Song {
        Song {
            source: MusicApiType::Spotify,
            id: format!("id-{}", name),
            sid: None,
            isrc: None,
            name: name.to_string(),
            album: album.map(|a| Album {
                id: None,
                name: a.to_string(),
                upc: None,
            }),
            artists: artists
                .iter()
                .map(|a| Artist {
                    id: None,
                    name: (*a).to_string(),
                })
                .collect(),
            duration_ms: 200_000,
        }
    }

    fn pairs(songs: &[Song]) -> Vec<(usize, &Song)> {
        songs.iter().enumerate().collect()
    }

    #[test]
    fn cache_key_separates_songs_that_differ_only_by_isrc() {
        let mut a = song("Song", &["Artist"], Some("Album"));
        let mut b = a.clone();
        assert_eq!(song_cache_key(&a), song_cache_key(&b));
        a.isrc = Some("USAAA0000001".to_string());
        assert_ne!(song_cache_key(&a), song_cache_key(&b));
        b.isrc = Some("USAAA0000002".to_string());
        assert_ne!(song_cache_key(&a), song_cache_key(&b));
    }

    #[test]
    fn valid_mbid_accepts_only_uuids() {
        assert!(is_valid_mbid("e8f9b188-f819-4e43-ab0f-4bd26ce9ff56"));
        assert!(!is_valid_mbid(""));
        assert!(!is_valid_mbid("2x1GoZKREbFkQJ8FUaz3Lc"));
        // right shape, non-hex character
        assert!(!is_valid_mbid("e8f9b18g-f819-4e43-ab0f-4bd26ce9ff56"));
        // trailing group
        assert!(!is_valid_mbid("e8f9b188-f819-4e43-ab0f-4bd26ce9ff56-0"));
        assert!(!is_valid_mbid("e8f9b188f8194e43ab0f4bd26ce9ff56"));
    }

    #[test]
    fn lookup_items_drop_songs_the_server_would_reject() {
        let songs = vec![
            song("ok", &["artist"], Some("album")),
            // no artist: would 400 the whole chunk
            song("no artist", &[], Some("album")),
            // no title: same
            song("", &["artist"], Some("album")),
        ];
        let (items, key_to_idx) = build_lookup_items(&pairs(&songs), LookupVariant::Raw);
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].recording, "ok");
        assert_eq!(key_to_idx.len(), 1);
    }

    #[test]
    fn query_budget_counts_characters_not_bytes() {
        // 200 chars of 'é' is 400 bytes: a byte-based budget would wrongly drop
        // the release name here, even though the server counts code points.
        let artist = "a".repeat(20);
        let recording = "é".repeat(20);
        let release = "b".repeat(20);
        assert!(recording.len() > recording.chars().count());

        let fitted = fit_query_budget(artist, recording, Some(release)).unwrap();
        assert!(fitted.2.is_some(), "release name should have been kept");
    }

    #[test]
    fn query_budget_drops_release_then_gives_up() {
        let long = "x".repeat(200);

        // artist + recording fit, release pushes it over: release is dropped
        let fitted = fit_query_budget("a".repeat(40), long.clone(), Some(long.clone())).unwrap();
        assert_eq!(fitted.2, None);

        // artist + recording alone are already over budget: unusable
        assert!(fit_query_budget(long.clone(), long, None).is_none());
    }

    #[test]
    fn lookup_items_dedup_colliding_keys() {
        let songs = vec![
            song("Déjà Vu", &["Artist"], Some("Album")),
            song("deja  vu!", &["artist"], Some("album")),
            song("Other", &["Artist"], Some("Album")),
        ];
        let (items, key_to_idx) = build_lookup_items(&pairs(&songs), LookupVariant::Raw);
        assert_eq!(items.len(), 2);
        assert_eq!(key_to_idx[&items[0].key], vec![0, 1]);
        assert_eq!(key_to_idx[&items[1].key], vec![2]);
    }

    #[test]
    fn all_artists_variant_skips_single_artist_songs() {
        let songs = vec![
            song("a", &["solo"], None),
            song("b", &["one", "two"], None),
            song("c", &["one", "two", "three", "four"], None),
        ];
        let (items, _) = build_lookup_items(&pairs(&songs), LookupVariant::AllArtists);
        assert_eq!(items.len(), 2);
        assert_eq!(items[0].artist, "one feat. two");
        assert_eq!(items[1].artist, "one feat. two, three & four");
        // the release is deliberately omitted for this variant
        assert_eq!(items[0].release, None);
    }

    #[test]
    fn realign_uses_index_and_fans_out_to_every_source_song() {
        let songs = vec![
            song("Déjà Vu", &["Artist"], Some("Album")),
            song("deja vu", &["artist"], Some("album")),
            song("Other", &["Artist"], Some("Album")),
        ];
        let (items, key_to_idx) = build_lookup_items(&pairs(&songs), LookupVariant::Raw);
        assert_eq!(items.len(), 2);

        // deliberately out of order, with the second entry unmatched and a
        // null recording_mbid, exactly as the server returns misses
        let res: Vec<ListenBrainzLookupResponse> = serde_json::from_value(serde_json::json!([
            {
                "index": 1,
                "recording_mbid": null,
                "artist_name_arg": "Artist",
                "recording_name_arg": "Other"
            },
            {
                "index": 0,
                "recording_mbid": "e8f9b188-f819-4e43-ab0f-4bd26ce9ff56",
                "recording_name": "Deja Vu",
                "artist_credit_name": "Artist",
                "release_name": "Album",
                "release_mbid": "8d3acbb4-c541-4324-a124-a670615f0f77",
                "artist_mbids": ["4c0d9acf-a8a1-4765-9c56-05f92f68c048"]
            }
        ]))
        .unwrap();

        let mut out: Vec<Option<Song>> = vec![None; songs.len()];
        realign(&songs, &items, &key_to_idx, res, &mut out);

        // both colliding source songs get the single result
        assert_eq!(
            out[0].as_ref().unwrap().id,
            "e8f9b188-f819-4e43-ab0f-4bd26ce9ff56"
        );
        assert_eq!(
            out[1].as_ref().unwrap().id,
            "e8f9b188-f819-4e43-ab0f-4bd26ce9ff56"
        );
        assert_eq!(out[0].as_ref().unwrap().source, MusicApiType::ListenBrainz);
        // the miss stays open so the next variant pass retries it
        assert!(out[2].is_none());
    }

    #[test]
    fn realign_rejects_a_result_that_does_not_match_the_source_song() {
        let songs = vec![song("Get Lucky", &["Daft Punk"], Some("Random Access Memories"))];
        let (items, key_to_idx) = build_lookup_items(&pairs(&songs), LookupVariant::Raw);
        // a different recording of the same title, from another release
        let res: Vec<ListenBrainzLookupResponse> = serde_json::from_value(serde_json::json!([{
            "index": 0,
            "recording_mbid": "345e4a72-46b4-48ba-8541-27c6b23c3b8c",
            "recording_name": "Get Lucky",
            "artist_credit_name": "Daft Punk",
            "release_name": "Undercover, Vol. 2",
            "release_mbid": "8d3acbb4-c541-4324-a124-a670615f0f77",
            "artist_mbids": ["056e4f3e-d505-4dad-8ec1-d04f521cbb56"]
        }]))
        .unwrap();

        let mut out: Vec<Option<Song>> = vec![None; songs.len()];
        realign(&songs, &items, &key_to_idx, res, &mut out);
        assert!(out[0].is_none());
    }

    #[test]
    fn contiguous_runs_stay_valid_when_applied_in_order() {
        let mut items: Vec<usize> = (0..10).collect();
        let set: BTreeSet<usize> = [0, 1, 2, 5, 7, 8].into_iter().collect();
        for (index, count) in contiguous_runs_desc(&set) {
            items.drain(index..index + count);
        }
        assert_eq!(items, vec![3, 4, 6, 9]);
    }

    fn track_to_song(value: serde_json::Value) -> Result<Song> {
        let track: model::ListenBrainzTrackResponse = serde_json::from_value(value)?;
        track.try_into()
    }

    #[test]
    fn jspf_track_parses_legacy_string_identifier() {
        let song = track_to_song(serde_json::json!({
            "identifier": "https://musicbrainz.org/recording/e8f9b188-f819-4e43-ab0f-4bd26ce9ff56",
            "title": "Gold"
        }))
        .unwrap();
        assert_eq!(song.id, "e8f9b188-f819-4e43-ab0f-4bd26ce9ff56");
        // absent duration and album must not be fabricated
        assert_eq!(song.duration_ms, 0);
        assert!(song.album.is_none());
        assert!(song.artists.is_empty());
    }

    #[test]
    fn isrc_fan_out_fills_every_song_sharing_an_isrc() {
        const RAW: &str = r#"{
            "count": 1,
            "recordings": [
                {
                    "id": "f2d5d66f-cf72-4e04-b913-9da2cfa1affb",
                    "score": 100,
                    "title": "aLIEz",
                    "isrcs": ["JPU901400900"],
                    "artist-credit": [],
                    "releases": []
                }
            ]
        }"#;
        let res: model::MusicBrainzSearchResponse = serde_json::from_str(RAW).unwrap();

        // three source songs all want the same ISRC
        let mut by_isrc: HashMap<&str, Vec<usize>> = HashMap::new();
        by_isrc.insert("JPU901400900", vec![0, 2, 3]);

        let mut out: Vec<Option<Song>> = vec![None; 4];
        assert_eq!(match_isrc_results(&res.recordings, &by_isrc, &mut out), 3);
        assert!(out[0].is_some() && out[2].is_some() && out[3].is_some());
        assert!(out[1].is_none());
    }

    #[test]
    fn name_fallback_is_reserved_for_songs_without_an_isrc() {
        let mut with_isrc = song("has isrc", &["a"], None);
        with_isrc.isrc = Some("USQY51742992".to_string());
        let without = song("no isrc", &["a"], None);
        let mut resolved = song("already done", &["a"], None);
        resolved.isrc = None;

        let songs = vec![with_isrc, without, resolved];
        let out = vec![None, None, Some(song("x", &["a"], None))];

        let cands = name_fallback_candidates(&songs, &out);
        let idx: Vec<usize> = cands.iter().map(|(i, _)| *i).collect();

        // only the ISRC-less, still-unresolved song qualifies
        assert_eq!(idx, vec![1]);
    }

    fn rec(id: &str, title: &str, length: Option<usize>, release: &str) -> serde_json::Value {
        serde_json::json!({
            "id": id, "score": 100, "title": title, "length": length,
            "isrcs": [], "artist-credit": [],
            "releases": [{"id": release, "title": "Some Release"}]
        })
    }

    fn recs(vals: Vec<serde_json::Value>) -> Vec<model::MusicBrainzRecording> {
        vals.into_iter()
            .map(|v| serde_json::from_value(v).unwrap())
            .collect()
    }

    const REL: &str = "44705b77-434c-4533-b417-f707fdf526b2";

    fn one_release(indices: Vec<usize>) -> HashMap<String, Vec<usize>> {
        let mut m = HashMap::new();
        m.insert(REL.to_string(), indices);
        m
    }

    #[test]
    fn barcode_pick_accepts_the_only_track_of_matching_length() {
        let mut s = song("Again", &["artist"], Some("album"));
        s.duration_ms = 212_000;
        let songs = vec![s];
        let recordings = recs(vec![
            rec("0248ecf1-3a7d-4e26-8fe8-b69009aa0f9f", "Again - Ghibli Piano Version", Some(212_474), REL),
            rec("11111111-1111-1111-1111-111111111111", "Something Else", Some(300_000), REL),
        ]);
        let mut out = vec![None];
        assert_eq!(pick_by_duration(&recordings, &one_release(vec![0]), &songs, &mut out), 1);
        // the title differs substantially, duration is what decides
        assert_eq!(out[0].as_ref().unwrap().id, "0248ecf1-3a7d-4e26-8fe8-b69009aa0f9f");
    }

    #[test]
    fn barcode_pick_rejects_when_nothing_is_close_enough() {
        let mut s = song("Again", &["artist"], Some("album"));
        s.duration_ms = 212_000;
        let songs = vec![s];
        // 3s out, beyond the 2s tolerance
        let recordings = recs(vec![rec("0248ecf1-3a7d-4e26-8fe8-b69009aa0f9f", "Again", Some(215_001), REL)]);
        let mut out = vec![None];
        assert_eq!(pick_by_duration(&recordings, &one_release(vec![0]), &songs, &mut out), 0);
        assert!(out[0].is_none());
    }

    #[test]
    fn barcode_pick_breaks_a_duration_tie_on_title() {
        let mut s = song("Billions", &["artist"], Some("album"));
        s.duration_ms = 248_000;
        let songs = vec![s];
        let recordings = recs(vec![
            rec("aaaaaaaa-1111-1111-1111-111111111111", "Totally Different", Some(248_100), REL),
            rec("bbbbbbbb-2222-2222-2222-222222222222", "Billions", Some(248_200), REL),
        ]);
        let mut out = vec![None];
        assert_eq!(pick_by_duration(&recordings, &one_release(vec![0]), &songs, &mut out), 1);
        assert_eq!(out[0].as_ref().unwrap().id, "bbbbbbbb-2222-2222-2222-222222222222");
    }

    #[test]
    fn barcode_pick_leaves_a_genuine_tie_unresolved() {
        // two same-length tracks whose titles are equally (un)like the source
        let mut s = song("Untitled", &["artist"], Some("album"));
        s.duration_ms = 248_000;
        let songs = vec![s];
        let recordings = recs(vec![
            rec("aaaaaaaa-1111-1111-1111-111111111111", "Alpha", Some(248_100), REL),
            rec("bbbbbbbb-2222-2222-2222-222222222222", "Gamma", Some(248_200), REL),
        ]);
        let mut out = vec![None];
        assert_eq!(pick_by_duration(&recordings, &one_release(vec![0]), &songs, &mut out), 0);
        assert!(out[0].is_none());
    }

    #[test]
    fn barcode_pick_skips_ineligible_candidates() {
        const ID: &str = "0248ecf1-3a7d-4e26-8fe8-b69009aa0f9f";
        let mut s = song("Again", &["artist"], Some("album"));
        s.duration_ms = 212_000;
        let songs = vec![s];
        let pick = |recording: serde_json::Value, out: &mut Vec<Option<Song>>| {
            pick_by_duration(&recs(vec![recording]), &one_release(vec![0]), &songs, out)
        };

        // a recording with no length
        assert_eq!(pick(rec(ID, "Again", None, REL), &mut vec![None]), 0);
        // a recording from another release
        let other = "99999999-9999-9999-9999-999999999999";
        assert_eq!(pick(rec(ID, "Again", Some(212_000), other), &mut vec![None]), 0);
        // a song already matched by ISRC is never overridden
        let already = song("already", &["a"], None);
        let mut out = vec![Some(already.clone())];
        assert_eq!(pick(rec(ID, "Again", Some(212_000), REL), &mut out), 0);
        assert_eq!(out[0].as_ref().unwrap().id, already.id);
    }

    /// A real `MusicBrainz` ISRC-search response, trimmed.
    #[test]
    fn parses_a_real_musicbrainz_isrc_response() {
        const RAW: &str = r#"{
            "count": 1,
            "recordings": [
                {
                    "id": "f2d5d66f-cf72-4e04-b913-9da2cfa1affb",
                    "score": 96,
                    "title": "aLIEz",
                    "length": 268000,
                    "isrcs": [
                        "JPU901400900"
                    ],
                    "artist-credit": [
                        {
                            "name": "SawanoHiroyuki[nZk]",
                            "joinphrase": ":",
                            "artist": {
                                "id": "cb191900-8ad8-46b9-b021-a093ee2b2f9b",
                                "name": "SawanoHiroyuki[nZk]"
                            }
                        },
                        {
                            "name": "mizuki",
                            "joinphrase": "",
                            "artist": {
                                "id": "32539fef-9c05-40a6-b164-39944d33148a",
                                "name": "瑞葵"
                            }
                        }
                    ],
                    "releases": [
                        {
                            "id": "9d4f6c16-4229-4451-9853-33429c01d3c3",
                            "title": "bLACKbLUE"
                        }
                    ]
                }
            ]
        }"#;

        let res: model::MusicBrainzSearchResponse = serde_json::from_str(RAW).unwrap();
        let rec = &res.recordings[0];

        assert_eq!(rec.credit_string(), "SawanoHiroyuki[nZk]:mizuki");

        let song = recording_to_song(rec, Some("JPU901400900"));
        assert_eq!(song.source, MusicApiType::ListenBrainz);
        assert_eq!(song.id, "f2d5d66f-cf72-4e04-b913-9da2cfa1affb");
        assert_eq!(song.name, "aLIEz");
        assert_eq!(song.duration_ms, 268_000);
        // the matched ISRC is carried over so it still compares by ISRC
        assert_eq!(song.isrc.as_deref(), Some("JPU901400900"));
        assert_eq!(
            song.artists[0].id.as_deref(),
            Some("cb191900-8ad8-46b9-b021-a093ee2b2f9b")
        );
    }

    /// A real `GET /1/playlist/{mbid}` response, trimmed to two tracks.
    #[test]
    fn parses_a_real_playlist_response() {
        const RAW: &str = r#"{
            "playlist": {
                "annotation": "We made this Daily Jams playlist from your recommended tracks to create a comfortable playlist of music you've not listened to recently.",
                "creator": "rob",
                "date": "2025-09-19T12:26:32.820489+00:00",
                "extension": {
                    "https://musicbrainz.org/doc/jspf#playlist": {
                        "additional_metadata": {
                            "algorithm_metadata": {
                                "source_patch": "daily-jams"
                            },
                            "expires_at": "2025-10-02T22:00:37.450569",
                            "external_urls": {
                                "spotify": "https://open.spotify.com/playlist/4MpqP8PB1BXkwMQrrWuxL9"
                            }
                        },
                        "copied_from_deleted": true,
                        "creator": "rob",
                        "last_modified_at": "2025-09-19T12:26:32.820489+00:00",
                        "public": true
                    }
                },
                "identifier": "https://listenbrainz.org/playlist/8357d30f-beac-4639-bdf3-b969d3a5c424",
                "title": "Copy of Daily Jams for rob, 2025-09-19 Fri",
                "track": [
                    {
                        "album": "AWAKE/ASLEEP",
                        "creator": "Sløtface",
                        "duration": 170000,
                        "extension": {
                            "https://musicbrainz.org/doc/jspf#track": {
                                "added_at": "2025-09-18T22:00:40.005316+00:00",
                                "added_by": "troi-bot",
                                "additional_metadata": {
                                    "artists": [
                                        {
                                            "artist_credit_name": "Sløtface",
                                            "artist_mbid": "279e7d66-17a4-40fc-b4a6-1999742399df",
                                            "join_phrase": ""
                                        }
                                    ],
                                    "caa_id": 38383739262,
                                    "caa_release_mbid": "1ae28889-7c6e-47f6-b50e-6440e293b5a3"
                                },
                                "artist_identifiers": [
                                    "https://musicbrainz.org/artist/279e7d66-17a4-40fc-b4a6-1999742399df"
                                ]
                            }
                        },
                        "identifier": [
                            "https://musicbrainz.org/recording/273c17b3-9e39-4dd4-bea7-ecaad28202e7"
                        ],
                        "title": "Indoor Kid"
                    },
                    {
                        "album": "Born to be Blue",
                        "creator": "Anne Phillips",
                        "duration": 160000,
                        "extension": {
                            "https://musicbrainz.org/doc/jspf#track": {
                                "added_at": "2025-09-18T22:00:40.005316+00:00",
                                "added_by": "troi-bot",
                                "additional_metadata": {
                                    "artists": [
                                        {
                                            "artist_credit_name": "Anne Phillips",
                                            "artist_mbid": "bbff5cd1-6a22-4ce9-b348-5941aa3f02a7",
                                            "join_phrase": ""
                                        }
                                    ],
                                    "caa_id": 9649768634,
                                    "caa_release_mbid": "d9d9a82d-d895-459f-bfad-3ba1c8d44928"
                                },
                                "artist_identifiers": [
                                    "https://musicbrainz.org/artist/bbff5cd1-6a22-4ce9-b348-5941aa3f02a7"
                                ]
                            }
                        },
                        "identifier": [
                            "https://musicbrainz.org/recording/44d4f2c5-e9fa-4561-af70-39676e37e4f6"
                        ],
                        "title": "You Don't Know What Love Is"
                    }
                ]
            }
        }"#;

        let wrapper: model::ListenBrainzPlaylistWrapper = serde_json::from_str(RAW).unwrap();
        let playlist: Playlist = wrapper.playlist.try_into().unwrap();

        assert_eq!(playlist.id, "8357d30f-beac-4639-bdf3-b969d3a5c424");
        assert_eq!(playlist.name, "Copy of Daily Jams for rob, 2025-09-19 Fri");
        assert_eq!(playlist.songs.len(), 2);

        let song = &playlist.songs[0];
        assert_eq!(song.source, MusicApiType::ListenBrainz);
        assert_eq!(song.id, "273c17b3-9e39-4dd4-bea7-ecaad28202e7");
        assert_eq!(song.duration_ms, 170_000);
        assert_eq!(song.artists[0].name, "Sl\u{f8}tface");
        assert_eq!(
            song.artists[0].id.as_deref(),
            Some("279e7d66-17a4-40fc-b4a6-1999742399df")
        );
        // this track carries an album name but no release_identifier
        let album = song.album.as_ref().unwrap();
        assert_eq!(album.name, "AWAKE/ASLEEP");
        assert_eq!(album.id, None);
    }
}
