use std::collections::{HashMap, HashSet};

use async_trait::async_trait;
use color_eyre::eyre::Result;
use futures::future::try_join_all;
use serde::{Deserialize, Serialize};
use strsim::normalized_levenshtein;
use tracing::debug;

use crate::utils::{generic_name_clean, normalize_name};

pub const PLAYLIST_DESC: &str = "Playlist created by SyncDisBoi";

pub type DynMusicApi = Box<dyn MusicApi + Sync>;

fn name_score(a: &str, b: &str) -> f64 {
    normalized_levenshtein(a, b).abs()
}

fn word_tokens(name: &str) -> HashSet<&str> {
    name.split(|c: char| !c.is_alphanumeric())
        .filter(|s| !s.is_empty())
        .collect()
}

/// Whether two album names plausibly refer to the same album. A whole-string
/// similarity score alone unfairly punishes box-set/compilation/reissue
/// naming (e.g. "Floodland Collection" vs "Floodland", "X Box Set" vs "X")
/// purely for the added length, even when every word of the shorter name is
/// present in the longer one. Treat that word-superset case as a match too,
/// the same way a mismatched ISRC doesn't block a match on its own.
fn album_names_resemble(a: &str, b: &str) -> bool {
    if name_score(a, b) >= 0.8 {
        return true;
    }
    let tokens_a = word_tokens(a);
    let tokens_b = word_tokens(b);
    if tokens_a.is_empty() || tokens_b.is_empty() {
        return false;
    }
    let (smaller, larger) = if tokens_a.len() <= tokens_b.len() {
        (&tokens_a, &tokens_b)
    } else {
        (&tokens_b, &tokens_a)
    };
    // The contained side needs to carry enough signal of its own for the
    // match to be meaningful, not just a coincidence -- either several
    // words, or (if it's a single word) a long, distinctive one. Otherwise
    // a short/generic album name ("Live", "Hits") would "contain"/"be
    // contained by" any album whose title happens to include that word.
    let longest_word_len = smaller.iter().map(|w| w.chars().count()).max().unwrap_or(0);
    if smaller.len() < 2 && longest_word_len < 6 {
        return false;
    }
    smaller.is_subset(larger)
}

#[async_trait]
pub trait MusicApi {
    fn api_type(&self) -> MusicApiType;
    fn country_code(&self) -> &str;

    async fn create_playlist(&self, name: &str, public: bool) -> Result<Playlist>;
    async fn get_playlists_info(&self) -> Result<Vec<Playlist>>;
    async fn get_playlist_songs(&self, id: &str) -> Result<Vec<Song>>;

    async fn get_playlists_full(&self) -> Result<Vec<Playlist>> {
        let mut playlists = self.get_playlists_info().await?;

        let mut requests = vec![];
        for playlist in &mut playlists {
            requests.push(self.get_playlist_songs(&playlist.id));
        }
        // Fetch songs with a bounded number of concurrent requests
        // (order-preserving) instead of firing them all at once, which can
        // trip platform rate limiters (e.g. Spotify 429) on large libraries.
        use futures::stream::StreamExt;
        let results: Vec<_> = futures::stream::iter(requests)
            .buffered(5)
            .collect()
            .await;
        for (i, songs) in results.into_iter().enumerate() {
            match songs {
                Ok(s) => playlists[i].songs = s,
                Err(e) => {
                    tracing::warn!("skipping non-accessible playlist \"{}\": {}", playlists[i].name, e);
                    playlists[i].songs = vec![];
                }
            }
        }

        Ok(playlists)
    }

    async fn add_songs_to_playlist(&self, playlist: &mut Playlist, songs: &[Song]) -> Result<()>;
    async fn remove_songs_from_playlist(
        &self,
        playlist: &mut Playlist,
        songs_ids: &[Song],
    ) -> Result<()>;
    async fn delete_playlist(&self, playlist: Playlist) -> Result<()>;

    async fn search_song(&self, song: &Song) -> Result<Option<Song>>;

    async fn search_songs(&self, songs: &[Song]) -> Result<Vec<Option<Song>>> {
        let mut requests = vec![];
        for song in songs {
            requests.push(self.search_song(song));
        }
        let results = try_join_all(requests).await?;
        Ok(results)
    }

    async fn add_likes(&self, songs: &[Song]) -> Result<()>;
    async fn get_likes(&self) -> Result<Vec<Song>>;
}

#[derive(Deserialize, Serialize, Clone, Debug, PartialEq, Eq)]
pub enum MusicApiType {
    Spotify,
    YtMusic,
    Tidal,
}

impl MusicApiType {
    pub const fn short_name(&self) -> &'static str {
        match self {
            MusicApiType::Spotify => "spotify",
            MusicApiType::YtMusic => "ytmusic",
            MusicApiType::Tidal => "tidal",
        }
    }
}

#[derive(Deserialize, Serialize, Debug)]
pub struct Playlists(pub Vec<Playlist>);

#[derive(Deserialize, Serialize, Debug)]
pub struct Songs(pub Vec<Song>);

#[derive(Deserialize, Serialize, Debug)]
pub struct Playlist {
    pub id: String,
    pub name: String,
    pub songs: Vec<Song>,
}

#[derive(Deserialize, Serialize, Clone, Debug)]
pub struct Song {
    pub source: MusicApiType,
    pub id: String,
    pub sid: Option<String>,
    /// All valid ISRCs known for this song. Usually a single value, but a
    /// recording can legitimately have more than one (e.g. reissue codes);
    /// `compare()` matches if either side has any code in common.
    pub isrc: Vec<String>,
    pub name: String,
    pub album: Option<Album>,
    pub artists: Vec<Artist>,
    pub duration_ms: usize,
}

impl Song {
    pub fn clean_name(&self) -> String {
        match self.source {
            MusicApiType::Spotify | MusicApiType::Tidal | MusicApiType::YtMusic => {
                let name = generic_name_clean(&self.name);
                let name = name.split(" - ").next().unwrap_or(&name);
                let name = name.split(" pts. ").next().unwrap_or(name);
                let name = name.split(" feat. ").next().unwrap_or(name);
                name.trim_end().to_string()
            }
        }
    }

    /// Basic-normalized name with qualifier stripping skipped: used as a
    /// second comparison signal when `--strip-qualifiers=false`, so
    /// remix/version qualifiers remain available to disambiguate songs
    /// that would otherwise collapse to the same `clean_name()`.
    pub fn raw_clean_name(&self) -> String {
        normalize_name(&self.name)
    }

    pub fn is_single(&self) -> bool {
        // TODO: improve this, leverage metadata from APIs when it exists
        //
        // Compare cleaned names, not raw ones: a single's `album.name` is
        // typically the bare title, while the track's own `name` often
        // carries a "(feat. X) - Y remix" qualifier the album name never
        // does. A raw comparison would miss exactly the remix/feat. singles
        // this check exists to recognize.
        if let Some(album) = &self.album {
            album.clean_name() == self.clean_name()
        } else {
            false
        }
    }

    pub fn compare(&self, other: &Self, map_singles: bool, strip_qualifiers: bool) -> bool {
        if !self.isrc.is_empty() && !other.isrc.is_empty() {
            let isrc_overlap = self.isrc.iter().any(|i| other.isrc.contains(i));
            if isrc_overlap {
                return true;
            }
            // NOTE: don't bail out here. Reissues, remasters, and regional
            // releases legitimately get a different ISRC for the same
            // recording, so fall through to the name/duration/album checks
            // below instead of treating a mismatched ISRC as conclusive.
        }

        // Check song name resemblance. With strip_qualifiers=false, also
        // check the qualifier-preserving raw name and take the best of the
        // two -- this only ever widens what counts as a name match, so
        // recall improves without weakening the strip_qualifiers=true
        // (default) path. Candidate *ranking* when several names tie is
        // handled separately by pick_best_match, not here.
        let score = if strip_qualifiers {
            name_score(&self.clean_name(), &other.clean_name())
        } else {
            let stripped = name_score(&self.clean_name(), &other.clean_name());
            let raw = name_score(&self.raw_clean_name(), &other.raw_clean_name());
            stripped.max(raw)
        };
        if score < 0.8 {
            return false;
        }

        // INFO: We can't really compare artists names since they are not always the
        // same order.
        // For certain platforms they are included in the song name but not in the
        // metadata

        // Check song duration resemblance
        // NOTE: YtMusic duration is sometimes garbage, it's incorrect on certain songs
        // it's still better to use it for accuracy
        let dur1 = self.duration_ms / 1000;
        let dur2 = other.duration_ms / 1000;

        // we allow a 1 second difference
        if dur1.abs_diff(dur2) > 1 {
            debug!("Duration: {} vs {} --> {} VS {}", dur1, dur2, self, other);
            return false;
        }

        if let (Some(album1), Some(album2)) = (&self.album, &other.album) {
            // With --map-singles: a track released as a single on one platform
            // (album name == track name, e.g. common on Spotify) may only exist
            // filed under its real full album on the other platform, so the
            // album names will legitimately differ. Skip the album check for
            // that case instead of rejecting an otherwise-strong match.
            //
            // This also covers the case where YtMusic maps an album song to a
            // standalone video, or suppresses the album song from the 'Songs'
            // filter, surfacing the single instead.
            let skip_album_check = map_singles && (self.is_single() || other.is_single());
            if !skip_album_check {
                // Check album name resemblance
                let name1 = album1.clean_name();
                let name2 = album2.clean_name();
                if !album_names_resemble(&name1, &name2) {
                    return false;
                }
            }
        }

        true
    }

    pub fn build_queries(&self, strip_qualifiers: bool) -> Vec<String> {
        let mut queries = vec![];
        let track_name = self.clean_name();

        // Query: Track + Album
        if let Some(album) = self.album.as_ref() {
            let album_name = album.clean_name();
            let tr_al_query = format!("{} {}", track_name, album_name);
            queries.push(tr_al_query);
        }
        // Query: Track + Artist
        for artist in self.artists.iter().rev() {
            let artist_name = artist.clean_name();
            let tr_ar_query = format!("{} {}", track_name, artist_name);
            queries.push(tr_ar_query);
        }
        // Query: Track + Artist + Album
        if let Some(album) = self.album.as_ref() {
            let album_name = album.clean_name();
            for artist in self.artists.iter().rev() {
                let artist_name = artist.clean_name();
                let tr_ar_al_query = format!("{} {} {}", track_name, artist_name, album_name);
                queries.push(tr_ar_al_query);
            }
        }

        // With qualifiers preserved, also try the raw (unstripped) track
        // name -- the remix/version text stripped out of `track_name`
        // above is often literally present in the destination platform's
        // title too, so keeping it in the query text improves recall for
        // exactly the tracks that collapse under the stripped name.
        if !strip_qualifiers {
            let raw_track_name = self.raw_clean_name();
            if raw_track_name != track_name {
                if let Some(album) = self.album.as_ref() {
                    let album_name = album.clean_name();
                    queries.push(format!("{} {}", raw_track_name, album_name));
                }
                for artist in self.artists.iter().rev() {
                    let artist_name = artist.clean_name();
                    queries.push(format!("{} {}", raw_track_name, artist_name));
                }
            }
        }

        queries
    }
}

/// Speeds up repeated `songs.contains(song)`-style membership checks against
/// a fixed list. `Song::compare` only ever returns `true` via an ISRC overlap
/// or a duration match within 1s, so candidates can be narrowed down by
/// those two keys before running the full (more expensive) comparison,
/// instead of scanning the whole list for every lookup.
pub struct SongIndex<'a> {
    by_isrc: HashMap<&'a str, Vec<&'a Song>>,
    by_duration_bucket: HashMap<i64, Vec<&'a Song>>,
}

impl<'a> SongIndex<'a> {
    pub fn build(songs: &'a [Song]) -> Self {
        let mut by_isrc: HashMap<&str, Vec<&Song>> = HashMap::new();
        let mut by_duration_bucket: HashMap<i64, Vec<&Song>> = HashMap::new();
        for song in songs {
            for isrc in &song.isrc {
                by_isrc.entry(isrc.as_str()).or_default().push(song);
            }
            let bucket: i64 = (song.duration_ms / 1000).try_into().unwrap_or(i64::MAX);
            by_duration_bucket.entry(bucket).or_default().push(song);
        }
        Self {
            by_isrc,
            by_duration_bucket,
        }
    }

    /// Equivalent to `songs.contains(song)` for the slice this index was
    /// built from.
    pub fn contains(&self, song: &Song, map_singles: bool, strip_qualifiers: bool) -> bool {
        for isrc in &song.isrc {
            if let Some(candidates) = self.by_isrc.get(isrc.as_str()) {
                if candidates
                    .iter()
                    .any(|c| song.compare(c, map_singles, strip_qualifiers))
                {
                    return true;
                }
            }
        }
        let bucket: i64 = (song.duration_ms / 1000).try_into().unwrap_or(i64::MAX);
        (bucket - 1..=bucket + 1).any(|b| {
            self.by_duration_bucket.get(&b).is_some_and(|candidates| {
                candidates
                    .iter()
                    .any(|c| song.compare(c, map_singles, strip_qualifiers))
            })
        })
    }
}

/// Picks the best-matching candidate from `candidates` for `song`. With
/// `strip_qualifiers` true (the default), returns the first candidate that
/// passes `compare()` -- identical to the "first result that matches" loops
/// every platform's `search_song()` used to run inline. With it false,
/// evaluates every candidate, keeps the ones passing `compare()`, and
/// returns the one whose *raw* (qualifier-preserving) name is closest to
/// `song`'s. Otherwise a remix and its original both matching on stripped
/// name get picked arbitrarily by search-result order, which is what
/// silently produced wrong/missed matches.
pub fn pick_best_match(
    song: &Song,
    mut candidates: impl Iterator<Item = Song>,
    map_singles: bool,
    strip_qualifiers: bool,
) -> Option<Song> {
    if strip_qualifiers {
        return candidates.find(|c| song.compare(c, map_singles, strip_qualifiers));
    }
    let song_raw = song.raw_clean_name();
    candidates
        .filter(|c| song.compare(c, map_singles, strip_qualifiers))
        .max_by(|a, b| {
            let score_a = name_score(&song_raw, &a.raw_clean_name());
            let score_b = name_score(&song_raw, &b.raw_clean_name());
            score_a.total_cmp(&score_b)
        })
}

impl PartialEq for Song {
    fn eq(&self, other: &Self) -> bool {
        // Neither `map_singles` nor `strip_qualifiers` has a config to read
        // here, so pick the safest defaults for an identity/dedup check with
        // no other context: map_singles=false (don't skip the album check),
        // strip_qualifiers=false (widen name matching to also consider the
        // raw, qualifier-preserving name -- this only ever makes matching
        // more permissive, never less).
        self.compare(other, false, false)
    }
}

impl Eq for Song {}

impl std::fmt::Display for Song {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let artists = self
            .artists
            .iter()
            .map(|a| a.name.as_str())
            .collect::<Vec<&str>>()
            .join(" ");
        let artists = String::from(" - ") + &artists;
        let album = if let Some(a) = &self.album {
            format!(" ({})", a.name)
        } else {
            String::new()
        };
        f.write_fmt(format_args!("{}{}{}", self.name, album, artists))
    }
}

#[derive(Deserialize, Serialize, Clone, Debug)]
pub struct Album {
    pub id: Option<String>,
    pub name: String,
}

impl Album {
    pub fn clean_name(&self) -> String {
        generic_name_clean(&self.name)
    }
}

#[derive(Deserialize, Serialize, Clone, Debug)]
pub struct Artist {
    pub id: Option<String>,
    pub name: String,
}

impl Artist {
    pub fn clean_name(&self) -> String {
        // TODO: Add ' - ' parsing?
        generic_name_clean(&self.name)
    }
}

#[derive(Serialize, Debug)]
pub struct OAuthReqToken {
    pub client_id: String,
    pub device_code: String,
    pub grant_type: String,
    pub scope: String,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct OAuthToken {
    pub scope: String,
    pub token_type: String,
    pub access_token: String,
    pub refresh_token: String,
    pub expires_in: u64,
}

#[derive(Deserialize, Debug)]
pub struct OAuthRefreshToken {
    pub access_token: String,
    pub expires_in: u64,
    pub scope: String,
    pub token_type: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn song(source: MusicApiType, id: &str, isrc: &[&str], duration_ms: usize) -> Song {
        Song {
            source,
            id: id.to_string(),
            sid: None,
            isrc: isrc.iter().copied().map(str::to_string).collect(),
            name: "Same Name".to_string(),
            album: None,
            artists: vec![],
            duration_ms,
        }
    }

    #[test]
    fn raw_clean_name_preserves_qualifiers_that_clean_name_strips() {
        let mut s = song(MusicApiType::Spotify, "sp-1", &[], 200_000);
        s.name = "Circus Bells (Hardfloor Remix)".to_string();
        assert_eq!(s.clean_name(), "circus bells");
        assert_eq!(s.raw_clean_name(), "circus bells (hardfloor remix)");
    }

    #[test]
    fn build_queries_adds_raw_variants_only_when_strip_qualifiers_false() {
        let mut s = song(MusicApiType::Spotify, "sp-1", &[], 200_000);
        s.name = "Track - Remix".to_string();
        s.album = Some(Album {
            id: None,
            name: "Some Album".to_string(),
        });
        s.artists = vec![Artist {
            id: None,
            name: "Some Artist".to_string(),
        }];

        let stripped_queries = s.build_queries(true);
        assert!(!stripped_queries.iter().any(|q| q.contains("remix")));

        let raw_queries = s.build_queries(false);
        assert!(raw_queries.iter().any(|q| q.contains("track - remix")));
        // stripped variants are still present too -- recall only improves
        assert!(raw_queries.len() > stripped_queries.len());
    }

    #[test]
    fn compare_matches_when_either_side_has_an_overlapping_isrc() {
        // e.g. a Tidal recording tagged with multiple reissue ISRCs, one
        // of which happens to be the single ISRC the other platform reports.
        let a = song(
            MusicApiType::Tidal,
            "sub-1",
            &["GBBTF0400057", "USSM19501100"],
            200_000,
        );
        let b = song(MusicApiType::Spotify, "sp-1", &["USSM19501100"], 999_000);
        assert!(a.compare(&b, false, true));
        assert!(b.compare(&a, false, true));
    }

    #[test]
    fn compare_falls_back_to_name_and_duration_when_isrcs_are_disjoint() {
        // e.g. a remaster or regional reissue: same recording, but the
        // ISRC differs, so ISRC alone can't rule it out.
        let a = song(MusicApiType::Tidal, "sub-1", &["AAAAA1111111"], 200_000);
        let b = song(MusicApiType::Spotify, "sp-1", &["BBBBB2222222"], 200_000);
        assert!(a.compare(&b, false, true));
    }

    #[test]
    fn compare_rejects_when_isrcs_are_disjoint_and_names_or_durations_differ() {
        let mut a = song(MusicApiType::Tidal, "sub-1", &["AAAAA1111111"], 200_000);
        a.name = "Totally Different Song".to_string();
        let b = song(MusicApiType::Spotify, "sp-1", &["BBBBB2222222"], 200_000);
        assert!(!a.compare(&b, false, true));
    }

    #[test]
    fn compare_matches_same_source_songs_with_different_ids_but_matching_name_and_duration() {
        // e.g. two different destination-platform track IDs (duplicate
        // upload, or a different pick_best_match tie-break on a later run)
        // that are logically the same recording -- dedup checks (playlist
        // sync, like sync) need this to catch same-source near-duplicates,
        // not just literal id equality.
        let a = song(MusicApiType::Spotify, "sp-1", &[], 200_000);
        let b = song(MusicApiType::Spotify, "sp-2", &[], 200_000);
        assert!(a.compare(&b, false, true));
    }

    #[test]
    fn compare_rejects_same_source_songs_with_different_ids_and_different_name_and_duration() {
        let a = song(MusicApiType::Spotify, "sp-1", &[], 200_000);
        let mut b = song(MusicApiType::Spotify, "sp-2", &[], 999_000);
        b.name = "Totally Different Song".to_string();
        assert!(!a.compare(&b, false, true));
    }

    #[test]
    fn compare_does_not_underflow_when_a_duration_is_under_one_second() {
        // duration_ms < 1000 makes `duration_ms / 1000 == 0`; the duration
        // check must not do `0 - 1` on an unsigned type.
        let a = song(MusicApiType::Spotify, "sp-1", &[], 0);
        let b = song(MusicApiType::Spotify, "sp-2", &[], 500);
        assert!(a.compare(&b, false, true));
    }

    #[test]
    fn is_single_recognizes_remix_titled_single_via_clean_name() {
        // A Spotify single's `album.name` is typically the bare title, while
        // the track's own `name` carries a "(feat. X) - Y remix" qualifier
        // the album name never has. Comparing the raw names therefore always
        // misses these, even though the release is unambiguously a single.
        let mut a = song(MusicApiType::Spotify, "sp-1", &[], 200_000);
        a.name = "Let Me - Rave Mix".to_string();
        a.album = Some(Album {
            id: None,
            name: "Let Me".to_string(),
        });
        assert!(a.is_single());
    }

    #[test]
    fn compare_rejects_single_vs_album_track_without_map_singles() {
        let mut a = song(MusicApiType::Spotify, "sp-1", &[], 200_000);
        a.name = "Let Me - Rave Mix".to_string();
        a.album = Some(Album {
            id: None,
            name: "Let Me".to_string(),
        });
        let mut b = song(MusicApiType::Tidal, "sub-1", &[], 200_000);
        b.name = "Let Me - Rave Mix".to_string();
        b.album = Some(Album {
            id: None,
            name: "Some Completely Different Album".to_string(),
        });
        assert!(!a.compare(&b, false, true));
    }

    #[test]
    fn compare_matches_remix_titled_single_vs_album_track_with_map_singles() {
        let mut a = song(MusicApiType::Spotify, "sp-1", &[], 200_000);
        a.name = "Let Me - Rave Mix".to_string();
        a.album = Some(Album {
            id: None,
            name: "Let Me".to_string(),
        });
        let mut b = song(MusicApiType::Tidal, "sub-1", &[], 200_000);
        b.name = "Let Me - Rave Mix".to_string();
        b.album = Some(Album {
            id: None,
            name: "Some Completely Different Album".to_string(),
        });
        assert!(a.compare(&b, true, true));
    }

    #[test]
    fn compare_matches_when_album_name_is_a_superset_reissue_variant() {
        // e.g. a box-set/compilation reissue: "Floodland Collection" is
        // unambiguously the same album as "Floodland", just filed under a
        // reissue-specific name. A whole-string similarity score alone
        // (normalized_levenshtein) drops well under 0.8 here purely because
        // of the length difference, even though every word in the shorter
        // name is present in the longer one.
        let mut a = song(MusicApiType::Spotify, "sp-1", &[], 200_000);
        a.album = Some(Album {
            id: None,
            name: "Floodland Collection".to_string(),
        });
        let mut b = song(MusicApiType::Tidal, "sub-1", &[], 200_000);
        b.album = Some(Album {
            id: None,
            name: "Floodland".to_string(),
        });
        assert!(a.compare(&b, false, true));
    }

    #[test]
    fn compare_matches_when_album_name_has_extra_inserted_words_not_just_a_suffix() {
        // Same real-world pattern, but the extra words aren't just appended
        // at the end -- "Hôtel Costes 7" vs "Hôtel Costes, Volume 7" -- so a
        // simple prefix check wouldn't catch it; token-set containment does.
        let mut a = song(MusicApiType::Spotify, "sp-1", &[], 200_000);
        a.album = Some(Album {
            id: None,
            name: "Hôtel Costes 7".to_string(),
        });
        let mut b = song(MusicApiType::Tidal, "sub-1", &[], 200_000);
        b.album = Some(Album {
            id: None,
            name: "Hôtel Costes, Volume 7".to_string(),
        });
        assert!(a.compare(&b, false, true));
    }

    #[test]
    fn compare_rejects_when_album_names_are_unrelated() {
        let mut a = song(MusicApiType::Spotify, "sp-1", &[], 200_000);
        a.album = Some(Album {
            id: None,
            name: "Totally Different Album".to_string(),
        });
        let mut b = song(MusicApiType::Tidal, "sub-1", &[], 200_000);
        b.album = Some(Album {
            id: None,
            name: "Floodland".to_string(),
        });
        assert!(!a.compare(&b, false, true));
    }

    #[test]
    fn compare_rejects_when_shorter_album_name_is_a_short_generic_word() {
        // A single short, generic word ("Live") is a token-subset of almost
        // any album whose title happens to contain it -- unlike a
        // distinctive single word ("Floodland"), it shouldn't be enough on
        // its own to call two unrelated albums a match.
        let mut a = song(MusicApiType::Spotify, "sp-1", &[], 200_000);
        a.album = Some(Album {
            id: None,
            name: "Live".to_string(),
        });
        let mut b = song(MusicApiType::Tidal, "sub-1", &[], 200_000);
        b.album = Some(Album {
            id: None,
            name: "Live From Wembley".to_string(),
        });
        assert!(!a.compare(&b, false, true));
    }

    #[test]
    fn compare_with_strip_qualifiers_false_falls_back_to_raw_name_when_stripped_diverges() {
        // Contrived but demonstrates the mechanic: stripping collapses "Q1"
        // vs "Q2" down to a single differing character (levenshtein ratio
        // 0.5), while the long shared "(Extended Mix)" suffix makes the
        // *raw* names very similar (ratio ~0.94). With strip_qualifiers
        // true, only the stripped score is used and the match is rejected;
        // with it false, the raw score rescues it.
        let mut a = song(MusicApiType::Tidal, "sub-1", &[], 200_000);
        a.name = "Q1 (Extended Mix)".to_string();
        let mut b = song(MusicApiType::Spotify, "sp-1", &[], 200_000);
        b.name = "Q2 (Extended Mix)".to_string();

        assert!(!a.compare(&b, false, true));
        assert!(a.compare(&b, false, false));
    }

    #[test]
    fn song_index_matches_naive_contains_via_isrc() {
        let haystack = vec![
            song(MusicApiType::Tidal, "sub-1", &["AAAAA1111111"], 200_000),
            song(MusicApiType::Tidal, "sub-2", &["BBBBB2222222"], 999_000),
        ];
        let index = SongIndex::build(&haystack);

        let needle = song(MusicApiType::Spotify, "sp-1", &["AAAAA1111111"], 42_000);
        assert!(haystack.contains(&needle));
        assert!(index.contains(&needle, false, true));
    }

    #[test]
    fn song_index_matches_naive_contains_via_duration_bucket() {
        let haystack = vec![song(MusicApiType::Tidal, "sub-1", &[], 200_000)];
        let index = SongIndex::build(&haystack);

        // within the 1s tolerance, no shared ISRC
        let mut needle = song(MusicApiType::Spotify, "sp-1", &[], 200_900);
        needle.name = "Same Name".to_string();
        assert!(haystack.contains(&needle));
        assert!(index.contains(&needle, false, true));

        // outside the 1s tolerance
        let mut far = song(MusicApiType::Spotify, "sp-2", &[], 210_000);
        far.name = "Same Name".to_string();
        assert!(!haystack.contains(&far));
        assert!(!index.contains(&far, false, true));
    }

    #[test]
    fn song_index_rejects_absent_song() {
        let haystack = vec![song(MusicApiType::Tidal, "sub-1", &["AAAAA1111111"], 200_000)];
        let index = SongIndex::build(&haystack);

        let mut needle = song(MusicApiType::Spotify, "sp-1", &["ZZZZZ9999999"], 999_000);
        needle.name = "Nothing Like It".to_string();
        assert!(!haystack.contains(&needle));
        assert!(!index.contains(&needle, false, true));
    }

    #[test]
    fn pick_best_match_returns_first_pass_when_strip_qualifiers_true() {
        let mut query = song(MusicApiType::Spotify, "sp-1", &[], 538_700);
        query.name = "Circus Bells - Hardfloor Mix".to_string();

        let mut wrong_first = song(MusicApiType::Tidal, "sub-1", &[], 538_900);
        wrong_first.name = "Circus Bells (Totally Unrelated Remix)".to_string();
        let mut correct = song(MusicApiType::Tidal, "sub-2", &[], 538_200);
        correct.name = "Circus Bells (Hardfloor Remix)".to_string();

        let candidates = vec![wrong_first.clone(), correct.clone()];

        // With qualifiers stripped, both candidates' names collapse to
        // "circus bells" and the first one in iteration order wins -- the
        // exact bug this feature addresses.
        let picked = pick_best_match(&query, candidates.into_iter(), false, true).unwrap();
        assert_eq!(picked.id, wrong_first.id);
    }

    #[test]
    fn pick_best_match_prefers_closest_raw_name_when_strip_qualifiers_false() {
        let mut query = song(MusicApiType::Spotify, "sp-1", &[], 538_700);
        query.name = "Circus Bells - Hardfloor Mix".to_string();

        let mut wrong_first = song(MusicApiType::Tidal, "sub-1", &[], 538_900);
        wrong_first.name = "Circus Bells (Totally Unrelated Remix)".to_string();
        let mut correct = song(MusicApiType::Tidal, "sub-2", &[], 538_200);
        correct.name = "Circus Bells (Hardfloor Remix)".to_string();

        let candidates = vec![wrong_first, correct.clone()];

        // With qualifiers preserved, the raw name ranks the two candidates
        // and correctly prefers the one that actually shares "hardfloor".
        let picked = pick_best_match(&query, candidates.into_iter(), false, false).unwrap();
        assert_eq!(picked.id, correct.id);
    }

    #[test]
    fn pick_best_match_returns_none_when_no_candidate_passes() {
        let mut query = song(MusicApiType::Spotify, "sp-1", &[], 200_000);
        query.name = "Totally Different".to_string();
        let mut candidate = song(MusicApiType::Tidal, "sub-1", &[], 900_000);
        candidate.name = "Unrelated".to_string();

        assert!(pick_best_match(&query, vec![candidate.clone()].into_iter(), false, true).is_none());
        assert!(pick_best_match(&query, vec![candidate].into_iter(), false, false).is_none());
    }
}
