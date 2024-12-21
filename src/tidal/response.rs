use color_eyre::eyre::{eyre, Error, OptionExt, Result};
use tracing::error;

use super::model::{
    TidalMediaResponse, TidalPageResponse, TidalPlaylistResponse, TidalSearchResponse,
    TidalSongItemResponse, TidalSongResponse,
};
use crate::music_api::{Album, Artist, MusicApiType, Playlist, Playlists, Song, Songs};

// multiples

impl TryInto<Playlists> for TidalPageResponse<TidalPlaylistResponse> {
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

impl TryInto<Songs> for TidalPageResponse<TidalSongItemResponse> {
    type Error = Error;

    fn try_into(self) -> Result<Songs, Self::Error> {
        let mut res = vec![];
        for item in self.items.into_iter() {
            let song = match item.item.try_into() {
                Ok(s) => s,
                Err(e) => {
                    error!("failed to parse song in response, skipping it: {}", e);
                    continue;
                }
            };
            res.push(song);
        }
        Ok(Songs(res))
    }
}

impl TryInto<Songs> for TidalSearchResponse {
    type Error = Error;

    fn try_into(self) -> Result<Songs, Self::Error> {
        let mut res = vec![];
        for track in self.tracks.items.into_iter() {
            match track.try_into() {
                Ok(s) => res.push(s),
                Err(e) => {
                    error!("failed to parse song in response, skipping it: {}", e);
                    continue;
                }
            }
        }
        Ok(Songs(res))
    }
}

// singles

impl TryInto<Playlist> for TidalPlaylistResponse {
    type Error = Error;
    fn try_into(self) -> Result<Playlist, Self::Error> {
        Ok(Playlist {
            id: self.uuid,
            name: self.title.trim().to_string(),
            songs: vec![],
        })
    }
}

impl TryInto<Song> for TidalSongResponse {
    type Error = Error;
    fn try_into(self) -> Result<Song, Self::Error> {
        let album = Album {
            id: Some(self.album.id.to_string()),
            name: self.album.title,
        };
        let artists = self
            .artists
            .into_iter()
            .map(|a| Artist {
                id: Some(a.id.to_string()),
                name: a.name,
            })
            .collect();

        Ok(Song {
            source: MusicApiType::Tidal,
            id: self.id.to_string(),
            sid: None,
            isrc: Some(self.isrc.to_uppercase()),
            name: self.title,
            album: Some(album),
            artists,
            duration_ms: self.duration * 1000,
        })
    }
}

impl TryInto<Song> for TidalMediaResponse {
    type Error = Error;

    fn try_into(mut self) -> Result<Song, Self::Error> {
        let mut artists = Vec::new();
        let mut album = None;
        let included = self.included.ok_or_eyre("missing included data")?;
        for inc in included {
            match inc.typ.as_str() {
                "artists" => {
                    let name = inc.attributes.name.ok_or_eyre("missing song artist name")?;
                    artists.push(Artist {
                        id: Some(inc.id),
                        name,
                    });
                }
                "albums" => {
                    if album.is_some() {
                        return Err(eyre!("found multiple song albums"));
                    }
                    let title = inc
                        .attributes
                        .title
                        .ok_or_eyre("missing song album title")?;
                    album = Some(Album {
                        id: Some(inc.id),
                        name: title,
                    });
                }
                _ => return Err(eyre!("unknown tidal included type: {}", inc.typ)),
            }
        }
        assert_eq!(self.data.len(), 1);
        let data = self.data.remove(0);
        let duration = &data
            .attributes
            .duration
            .ok_or_eyre("missing song duration")?;
        let duration = iso8601::duration(duration).map_err(|e| eyre!(e))?;
        let iso8601::Duration::YMDHMS {
            year,
            month,
            day,
            hour,
            minute,
            second,
            millisecond,
        } = duration
        else {
            unreachable!("invalid iso8601 duration");
        };
        assert!(year == 0 && month == 0 && day == 0);
        // convert to ms
        let duration = hour as usize * 60 * 60 * 1000
            + minute as usize * 60 * 1000
            + second as usize * 1000
            + millisecond as usize;

        Ok(Song {
            source: MusicApiType::Tidal,
            id: data.id,
            sid: None,
            isrc: Some(data.attributes.isrc.ok_or_eyre("missing song isrc")?),
            name: data.attributes.title.ok_or_eyre("missing song title")?,
            album,
            artists,
            duration_ms: duration,
        })
    }
}
