use std::collections::HashMap;

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
        if let Some(album) = &self.album {
            album.name == self.name
        } else {
            false
        }
    }

    pub fn compare(&self, other: &Self) -> bool {
        if self.source == other.source {
            return self.id == other.id;
        }
        if !self.isrc.is_empty() && !other.isrc.is_empty() {
            return self.isrc.iter().any(|i| other.isrc.contains(i));
        }

        // Check song name resemblance
        let name1 = self.clean_name();
        let name2 = other.clean_name();
        let score = normalized_levenshtein(&name1, &name2).abs();
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
        if !(dur1 - 1..=dur1 + 1).contains(&dur2) {
            debug!("Duration: {} vs {} --> {} VS {}", dur1, dur2, self, other);
            return false;
        }

        if let (Some(album1), Some(album2)) = (&self.album, &other.album) {
            // INFO: Sometimes Youtube Music maps the album song to the Youtube Video
            // Sometimes, the album song is just suppressed from the 'Songs' filter
            // In these cases, we can get the single instead so we shouldn't compare album
            // names
            if !self.is_single() && !other.is_single() {
                // Check album name resemblance
                let name1 = album1.clean_name();
                let name2 = album2.clean_name();
                let score = normalized_levenshtein(&name1, &name2).abs();
                if score < 0.8 {
                    return false;
                }
            }
        }

        true
    }

    pub fn build_queries(&self) -> Vec<String> {
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
    pub fn contains(&self, song: &Song) -> bool {
        for isrc in &song.isrc {
            if let Some(candidates) = self.by_isrc.get(isrc.as_str()) {
                if candidates.iter().any(|c| song.compare(c)) {
                    return true;
                }
            }
        }
        let bucket: i64 = (song.duration_ms / 1000).try_into().unwrap_or(i64::MAX);
        (bucket - 1..=bucket + 1).any(|b| {
            self.by_duration_bucket
                .get(&b)
                .is_some_and(|candidates| candidates.iter().any(|c| song.compare(c)))
        })
    }
}

impl PartialEq for Song {
    fn eq(&self, other: &Self) -> bool {
        self.compare(other)
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
        assert!(a.compare(&b));
        assert!(b.compare(&a));
    }

    #[test]
    fn compare_rejects_when_isrcs_are_present_but_disjoint() {
        let a = song(MusicApiType::Tidal, "sub-1", &["AAAAA1111111"], 200_000);
        let b = song(MusicApiType::Spotify, "sp-1", &["BBBBB2222222"], 200_000);
        assert!(!a.compare(&b));
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
        assert!(index.contains(&needle));
    }

    #[test]
    fn song_index_matches_naive_contains_via_duration_bucket() {
        let haystack = vec![song(MusicApiType::Tidal, "sub-1", &[], 200_000)];
        let index = SongIndex::build(&haystack);

        // within the 1s tolerance, no shared ISRC
        let mut needle = song(MusicApiType::Spotify, "sp-1", &[], 200_900);
        needle.name = "Same Name".to_string();
        assert!(haystack.contains(&needle));
        assert!(index.contains(&needle));

        // outside the 1s tolerance
        let mut far = song(MusicApiType::Spotify, "sp-2", &[], 210_000);
        far.name = "Same Name".to_string();
        assert!(!haystack.contains(&far));
        assert!(!index.contains(&far));
    }

    #[test]
    fn song_index_rejects_absent_song() {
        let haystack = vec![song(MusicApiType::Tidal, "sub-1", &["AAAAA1111111"], 200_000)];
        let index = SongIndex::build(&haystack);

        let mut needle = song(MusicApiType::Spotify, "sp-1", &["ZZZZZ9999999"], 999_000);
        needle.name = "Nothing Like It".to_string();
        assert!(!haystack.contains(&needle));
        assert!(!index.contains(&needle));
    }
}
