use anyhow::Result;
use async_trait::async_trait;

use futures::future::try_join_all;
use serde::{Serialize, Deserialize};

pub const PLAYLIST_DESC: &'static str = "Playlist created by SyncDisBoy";

pub enum MusicApiType {
    Spotify,
    YoutubeMusic,
}

#[async_trait]
pub trait MusicApi {
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
            playlists[i].songs = Some(songs);
        }

        Ok(playlists)
    }

    async fn add_songs_to_playlist<T: AsRef<str> + Sync>(&self, playlist_id: &str, songs_ids: &[T]) -> Result<()>;
    async fn remove_songs_from_playlist<T: AsRef<str> + Sync>(&self, playlist: &mut Playlist, songs_ids: &[T]) -> Result<()>;
    async fn delete_playlist(&self, playlist_id: &str) -> Result<()>;

    async fn search_song(&self, song: &Song, precise: bool) -> Result<Option<Song>>;
}

#[derive(Deserialize, Serialize, Debug)]
pub struct Playlists(pub Vec<Playlist>);

#[derive(Deserialize, Serialize, Debug)]
pub struct Songs(pub Vec<Song>);

#[derive(Deserialize, Serialize, Debug)]
pub struct Playlist {
    //pub source: MusicApiType,
    pub id: String,
    pub name: String,
    pub songs: Option<Vec<Song>>,
}

#[derive(Deserialize, Serialize, Debug)]
pub struct Song {
    pub id: String,
    pub sid: Option<String>,
    pub name: String,
    pub clean_name: String,
    pub album: Option<Album>,
    pub artists: Vec<Artist>,
    pub duration: usize,
}

#[derive(Deserialize, Serialize, Debug)]
pub struct Album {
    pub id: Option<String>,
    pub name: String,
}

#[derive(Deserialize, Serialize, Debug)]
pub struct Artist {
    pub id: Option<String>,
    pub name: String,
}
