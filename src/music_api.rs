use async_trait::async_trait;
use color_eyre::eyre::Result;
use futures::future::try_join_all;
use serde::{Deserialize, Serialize};
use strsim::normalized_levenshtein;
use tracing::debug;

use crate::utils::generic_name_clean;

pub const PLAYLIST_DESC: &'static str = "Playlist created by SyncDisBoi";

pub type DynMusicApi = Box<dyn MusicApi + Sync>;

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
            playlists[i].songs = songs;
        }

        Ok(playlists)
    }

    async fn add_songs_to_playlist(
        &self,
        playlist: &mut Playlist,
        songs: &[Song],
    ) -> Result<()>;
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
}

#[derive(Deserialize, Serialize, Clone, Debug)]
pub enum MusicApiType {
    Spotify,
    YtMusic,
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
    pub name: String,
    pub album: Option<Album>,
    pub artists: Vec<Artist>,
    pub duration: usize,
}

impl Song {
    pub fn clean_name(&self) -> String {
        match self.source {
            MusicApiType::Spotify => {
                let name = generic_name_clean(&self.name);
                let name = name.split(" - ").next().unwrap();
                let name = name.split(" pts. ").next().unwrap();
                let name = name.split(" feat. ").next().unwrap();
                name.trim_end().to_string()
            }
            MusicApiType::YtMusic => {
                let name = generic_name_clean(&self.name);
                let name = name.split(" - ").next().unwrap();
                let name = name.split(" pts. ").next().unwrap();
                let name = name.split(" feat. ").next().unwrap();
                name.trim_end().to_string()
            }
        }
    }

    pub fn compare(&self, other: &Self) -> bool {
        // Check song name resemblance
        let name1 = self.clean_name();
        let name2 = other.clean_name();
        let score = normalized_levenshtein(&name1, &name2).abs();
        if score < 0.8 {
            // TODO: Remove this
            debug!("Song score: {} --> {} vs {}", score, name1, name2);
            return false;
        }

        // Check song duration resemblance
        // Can't do that, since YtMusic duration is garbage
        //let dur1 = self.duration / 1000;
        //let dur2 = other.duration / 1000;
        //if !(dur1 - 1..dur1 + 1).contains(&dur2) {
        //    println!(
        //        "Duration: {}/{} --> {} VS {}",
        //        self.duration, other.duration, self.name, other.name
        //    );
        //    return false;
        //}

        // Check album name resemblance
        if let Some(album1) = &self.album {
            if let Some(album2) = &other.album {
                let name1 = generic_name_clean(&album1.name);
                let name2 = generic_name_clean(&album1.name);
                let score = normalized_levenshtein(&name1, &name2).abs();
                if score < 0.8 {
                    // TODO: Remove this
                    debug!("Album score: {} --> {:?} vs {:?}", score, album1, album2);
                    return false;
                }
            }
        }

        true
    }
}

impl PartialEq for Song {
    fn eq(&self, other: &Self) -> bool {
        self.compare(other)
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
        generic_name_clean(&self.name)
    }
}
