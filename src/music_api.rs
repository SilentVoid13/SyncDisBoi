use async_trait::async_trait;
use anyhow::Result;

use serde::Serialize;

#[async_trait]
pub trait MusicApi {
    async fn create_playlist(&self);
    async fn get_playlists_info(&self) -> Result<Vec<Playlist>>;
    async fn get_playlist_songs(&self, id: &str) -> Result<Vec<Song>>;
    async fn get_playlists_full(&self) -> Result<Vec<Playlist>>;
}

#[derive(Serialize, Debug)]
pub struct Playlists(pub Vec<Playlist>);

#[derive(Serialize, Debug)]
pub struct Playlist {
    pub id: String,
    pub name: String,
    pub songs: Option<Songs>,
}

#[derive(Serialize, Debug)]
pub struct Songs(pub Vec<Song>);

#[derive(Serialize, Debug)]
pub struct Song {
    pub id: String,
    pub set_id: String,
    pub name: String,
    pub album: Option<Album>,
    pub artists: Vec<Artist>,
}

#[derive(Serialize, Debug)]
pub struct Album {
    pub id: Option<String>,
    pub name: String,
}

#[derive(Serialize, Debug)]
pub struct Artist {
    pub id: Option<String>,
    pub name: String,
}
