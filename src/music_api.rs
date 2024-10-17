use async_trait::async_trait;
use color_eyre::eyre::Result;
use futures::future::try_join_all;
use serde::{Deserialize, Serialize};
use strsim::normalized_levenshtein;
use tracing::debug;

use crate::utils::generic_name_clean;

pub const PLAYLIST_DESC: &str = "Playlist created by SyncDisBoi";

pub type DynMusicApi = Box<dyn MusicApi + Sync>;

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
        for playlist in playlists.iter_mut() {
            requests.push(self.get_playlist_songs(&playlist.id));
        }
        let results = try_join_all(requests).await?;
        for (i, songs) in results.into_iter().enumerate() {
            playlists[i].songs = songs;
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
        for song in songs.iter() {
            requests.push(self.search_song(song));
        }
        let results = try_join_all(requests).await?;
        Ok(results)
    }

    async fn add_like(&self, songs: &[Song]) -> Result<()>;
    async fn get_likes(&self) -> Result<Vec<Song>>;
}

#[derive(Deserialize, Serialize, Clone, Debug, PartialEq, Eq)]
pub enum MusicApiType {
    Spotify,
    YtMusic,
    Tidal,
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
    pub isrc: Option<String>,
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

    pub fn is_single(&self) -> bool {
        // TODO: improve this, leverage metadata from APIs when it exists
        if let Some(album) = &self.album {
            album.name == self.name
        } else {
            false
        }
    }

    pub fn compare(&self, other: &Self) -> bool {
        if self.isrc.is_some() && other.isrc.is_some() {
            return self.isrc == other.isrc;
        }

        // Check song name resemblance
        let name1 = self.clean_name();
        let name2 = other.clean_name();
        let score = normalized_levenshtein(&name1, &name2).abs();
        if score < 0.8 {
            return false;
        }

        // INFO: We can't really compare artists names since they are not always the
        // same order For certain platforms they are included in the song name
        // but not the metadata

        // Check song duration resemblance
        // NOTE: YtMusic duration is sometimes garbage, it's incorrect on certain songs
        // it's still better to use it for accuracy
        let dur1 = self.duration_ms / 1000;
        let dur2 = other.duration_ms / 1000;

        // we allow a 1 second difference
        if !(dur1 - 1..=dur1 + 1).contains(&dur2) {
            debug!(
                "Duration: {} vs {} --> {} VS {}",
                dur1, dur2, self.name, other.name
            );
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
