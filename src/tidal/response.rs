use color_eyre::eyre::{Error, Result};

use super::model::{
    TidalMediaResponse, TidalPageResponse, TidalPlaylistResponse, TidalSearchResponse,
    TidalSongItemResponse, TidalSongResponse,
};
use crate::music_api::{Album, Artist, MusicApiType, Playlist, Playlists, Song, Songs};

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

impl TryInto<Song> for TidalMediaResponse {
    type Error = Error;

    fn try_into(mut self) -> Result<Song, Self::Error> {
        let mut artists = Vec::new();
        let mut album = None;
        let included = self.included.unwrap();
        for inc in included {
            match inc.typ.as_str() {
                "artists" => {
                    let name = inc.attributes.name.unwrap();
                    artists.push(Artist {
                        id: Some(inc.id),
                        name,
                    });
                }
                "albums" => {
                    assert!(album.is_none());
                    let title = inc.attributes.title.unwrap();
                    album = Some(Album {
                        id: Some(inc.id),
                        name: title,
                    });
                }
                _ => unreachable!("unknown tidal included type: {}", inc.typ),
            }
        }
        assert!(self.data.len() == 1);
        let data = self.data.remove(0);
        let duration = &data.attributes.duration.unwrap();
        let duration = iso8601::duration(duration).unwrap();
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
            isrc: Some(data.attributes.isrc.unwrap()),
            name: data.attributes.title.unwrap(),
            album,
            artists,
            duration_ms: duration,
        })
    }
}
