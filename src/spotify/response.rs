use std::convert::TryInto;

use color_eyre::eyre::{Error, OptionExt, Result};
use serde::Deserialize;
use tracing::{debug, error};

use super::model::{
    SpotifyPageResponse, SpotifyPlaylistResponse, SpotifySearchResponse, SpotifySongItemResponse,
    SpotifySongResponse,
};
use crate::music_api::{Album, Artist, MusicApiType, Playlist, Playlists, Song, Songs};

// multiples

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
            let playlist = match item.try_into() {
                Ok(p) => p,
                Err(e) => {
                    error!("failed to parse playlist in response, skipping it: {}", e);
                    continue;
                }
            };
            res.push(playlist);
        }
        Ok(Playlists(res))
    }
}

impl<T> TryInto<Songs> for SpotifyPageResponse<T>
where
    T: TryInto<Song, Error = Error> + for<'d> Deserialize<'d> + std::fmt::Debug,
{
    type Error = Error;

    fn try_into(self) -> Result<Songs, Self::Error> {
        let mut res = vec![];
        for item in self.items.into_iter() {
            let song = match item.try_into() {
                Ok(s) => s,
                Err(e) => {
                    error!("failed to parse song in response, skipping it: {}", e);
                    continue;
                }
            };
            // either an invalid or deleted song
            if song.id.is_empty() || song.duration_ms == 0 || song.isrc.is_none() {
                debug!("song with invalid metadata, skipping it: '{}'", song.name);
                continue;
            }
            res.push(song);
        }
        Ok(Songs(res))
    }
}

// singles

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
        self.track.ok_or_eyre("null track metadata")?.try_into()
    }
}

impl TryInto<Song> for SpotifySongResponse {
    type Error = Error;

    fn try_into(self) -> Result<Song, Self::Error> {
        // not a huge fan of this error handling, but it's convenient for generics over
        // SpotifyPageResponse
        let id = if let Some(id) = self.id {
            id
        } else {
            "".to_string()
        };

        let artists = self
            .artists
            .into_iter()
            .filter(|a| a.id.is_some())
            .map(|i| Artist {
                id: Some(i.id.unwrap()),
                name: i.name,
            })
            .collect();
        let album = Album {
            id: self.album.id,
            name: self.album.name,
        };

        let isrc = self.external_ids.isrc.map(|isrc| {
            // the metadata is sometimes inconsistent
            let isrc = isrc.to_uppercase();
            isrc.replace("-", "")
        });

        Ok(Song {
            source: MusicApiType::Spotify,
            id,
            sid: None,
            isrc,
            name: self.name,
            album: Some(album),
            artists,
            duration_ms: self.duration_ms,
        })
    }
}
