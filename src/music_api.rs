use anyhow::Result;
use async_trait::async_trait;

use futures::future::try_join_all;
use serde::{Deserialize, Serialize};
use strsim::normalized_levenshtein;

use crate::utils::generic_name_clean;

pub const PLAYLIST_DESC: &'static str = "Playlist created by SyncDisBoi";

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

    async fn add_songs_to_playlist<T: AsRef<str> + Sync>(
        &self,
        playlist_id: &str,
        songs_ids: &[T],
    ) -> Result<()>;
    async fn remove_songs_from_playlist<T: AsRef<str> + Sync>(
        &self,
        playlist: &mut Playlist,
        songs_ids: &[T],
    ) -> Result<()>;
    async fn delete_playlist(&self, playlist_id: &str) -> Result<()>;

    async fn search_song(&self, song: &Song) -> Result<Option<Song>>;
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
    pub album: Option<Album>,
    pub artists: Vec<Artist>,
    pub duration: usize,
}

impl Song {
    pub fn compare(&self, other: &Self) -> bool {
        let name1 = generic_name_clean(&self.name);
        let name2 = generic_name_clean(&other.name);
        //let name1 = &self.name;
        //let name2 = &other.name;
        let score = normalized_levenshtein(&name1, &name2).abs();
        if score < 0.8 {
            println!("SCORE: {} --> {} vs {}", score, name1, name2);
            return false;
        } else {
        }

        if self.album.is_some() != other.album.is_some() {
            return false;
        }

        let dur1 = self.duration / 1000;
        let dur2 = self.duration / 1000;
        if !(dur1 - 1..dur1 + 1).contains(&dur2) {
            println!(
                "DURATION: {}/{} --> {} VS {}",
                self.duration, other.duration, self.name, other.name
            );
            return false;
        }

        true
    }
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
