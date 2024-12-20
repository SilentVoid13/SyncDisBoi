use std::convert::TryInto;

use color_eyre::eyre::{Error, Result};
use serde::Deserialize;
use tracing::debug;

use super::model::{
    SpotifyPageResponse, SpotifyPlaylistResponse, SpotifySearchResponse, SpotifySongItemResponse,
    SpotifySongResponse,
};
use crate::music_api::{Album, Artist, MusicApiType, Playlist, Playlists, Song, Songs};

impl TryInto<Songs> for SpotifySearchResponse {
    type Error = Error;

    fn try_into(self) -> Result<Songs, Self::Error> {
        self.tracks.try_into()
    }
}

impl<T> TryInto<Playlists> for SpotifyPageResponse<T>
where
    T: TryInto<Playlist, Error = Error> + for<'d> Deserialize<'d>,
{
    type Error = Error;

    fn try_into(self) -> Result<Playlists, Self::Error> {
        let mut res = vec![];
        for item in self.items.into_iter() {
            res.push(item.try_into()?);
        }
        Ok(Playlists(res))
    }
}

impl<T> TryInto<Songs> for SpotifyPageResponse<T>
where
    T: TryInto<Song, Error = Error> + for<'d> Deserialize<'d>,
{
    type Error = Error;

    fn try_into(self) -> Result<Songs, Self::Error> {
        let mut res = vec![];
        for item in self.items.into_iter() {
            let song = item.try_into()?;
            // either an invalid or delete song
            if song.duration_ms == 0 || song.isrc.is_none() {
                debug!("song with invalid metadata, skipping it: {:?}", song);
                continue;
            }
            res.push(song);
        }
        Ok(Songs(res))
    }
}

impl TryInto<Playlist> for SpotifyPlaylistResponse {
    type Error = Error;

    fn try_into(self) -> Result<Playlist, Self::Error> {
        Ok(Playlist {
            id: self.id,
            name: self.name.trim().to_string(),
            songs: vec![],
        })
    }
}

impl TryInto<Song> for SpotifySongItemResponse {
    type Error = Error;

    fn try_into(self) -> Result<Song, Self::Error> {
        self.track.try_into()
    }
}

impl TryInto<Song> for SpotifySongResponse {
    type Error = Error;

    fn try_into(self) -> Result<Song, Self::Error> {
        let artists = self
            .artists
            .into_iter()
            .map(|i| Artist {
                id: Some(i.id),
                name: i.name,
            })
            .collect();
        let album = Album {
            id: Some(self.album.id),
            name: self.album.name,
        };

        let isrc = self.external_ids.isrc.map(|isrc| {
            // the metadata is sometimes inconsistent
            let isrc = isrc.to_uppercase();
            isrc.replace("-", "")
        });

        Ok(Song {
            source: MusicApiType::Spotify,
            id: self.id,
            sid: None,
            isrc,
            name: self.name,
            album: Some(album),
            artists,
            duration_ms: self.duration_ms,
        })
    }
}
