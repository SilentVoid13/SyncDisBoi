use anyhow::{Error, Result};

use crate::music_api::{Artist, Playlist, Song, Album, Songs, Playlists};

use super::model::{SpotifyPageResponse, SpotifyPlaylistResponse, SpotifySongItemResponse};

impl TryInto<Playlists> for SpotifyPageResponse<SpotifyPlaylistResponse> {
    type Error = Error;

    fn try_into(self) -> Result<Playlists, Self::Error> {
        Ok(Playlists(
            self.items
                .into_iter()
                .map(|item| Playlist {
                    id: item.id,
                    name: item.name,
                    songs: None,
                })
                .collect(),
        ))
    }
}

impl TryInto<Songs> for SpotifyPageResponse<SpotifySongItemResponse> {
    type Error = Error;

    fn try_into(self) -> Result<Songs, Self::Error> {
        Ok(Songs(
            self.items
                .into_iter()
                .map(|item| {
                    let artists = item
                        .track
                        .artists
                        .into_iter()
                        .map(|i| Artist {
                            id: Some(i.id),
                            name: i.name,
                        })
                        .collect();
                    let album = Album {
                        id: Some(item.track.album.id),
                        name: item.track.album.name,
                    };
                    Song {
                        id: item.track.id,
                        name: item.track.name,
                        sid: None,
                        album: Some(album),
                        artists,
                    }
                })
                .collect(),
        ))
    }
}
