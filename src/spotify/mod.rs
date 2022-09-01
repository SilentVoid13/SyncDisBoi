mod model;

use crate::music_api::MusicApi;
use crate::music_api::Playlist;
use crate::music_api::Song;

use anyhow::Result;
use async_trait::async_trait;

pub struct SpotifyApi {}

impl SpotifyApi {}

#[async_trait]
impl MusicApi for SpotifyApi {
    async fn create_playlist(&self) {
        todo!();
    }

    async fn get_playlists_info(&self) -> Result<Vec<Playlist>> {
        Ok(vec![])
    }

    async fn get_playlist_songs(&self, id: &str) -> Result<Vec<Song>> {
        Ok(vec![])
    }

    async fn get_playlists_full(&self) -> Result<Vec<Playlist>> {
        Ok(vec![])
    }
}
