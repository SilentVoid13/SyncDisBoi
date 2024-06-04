use crate::music_api::{Album, Artist, MusicApiType, Playlist, Playlists, Song, Songs};
use color_eyre::eyre::{Error, Result};

use super::model::{TidalPageResponse, TidalPlaylistResponse, TidalSearchResponse, TidalSongItemResponse, TidalSongResponse};

impl TryInto<Playlists> for TidalPageResponse<TidalPlaylistResponse> {
    type Error = Error;

    fn try_into(self) -> Result<Playlists, Self::Error> {
        let mut res = vec![];
        for item in self.items.into_iter() {
            res.push(item.try_into()?);
        }
        Ok(Playlists(res))
    }
}

impl TryInto<Playlist> for TidalPlaylistResponse {
    type Error = Error;
    fn try_into(self) -> Result<Playlist, Self::Error> {
        Ok(Playlist {
            id: self.uuid,
            name: self.title,
            songs: vec![],
        })
    }
}

impl TryInto<Songs> for TidalPageResponse<TidalSongItemResponse> {
    type Error = Error;

    fn try_into(self) -> Result<Songs, Self::Error> {
        let mut res = vec![];
        for item in self.items.into_iter() {
            res.push(item.item.try_into()?);
        }
        Ok(Songs(res))
    }
}

impl TryInto<Song> for TidalSongResponse {
    type Error = Error;
    fn try_into(self) -> Result<Song, Self::Error> {
        let album = Album {
            id: Some(self.album.id.to_string()),
            name: self.album.title,
        };
        let artists = self.artists.into_iter().map(|a| Artist {
            id: Some(a.id.to_string()),
            name: a.name,
        }).collect();

        Ok(Song {
            source: MusicApiType::Tidal,
            id: self.id.to_string(),
            sid: None,
            name: self.title,
            album: Some(album),
            artists,
            duration: self.duration,
        })
            
    }
}

impl TryInto<Songs> for TidalSearchResponse {
    type Error = Error;

    fn try_into(self) -> Result<Songs, Self::Error> {
        let mut res = vec![];
        for track in self.tracks.items.into_iter() {
            res.push(track.try_into()?);
        }
        Ok(Songs(res))
    }
}
