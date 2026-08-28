use std::convert::TryInto;

use color_eyre::eyre::{Error, OptionExt, Result};
use tracing::error;

use super::ListenBrainzApi;
use super::model::{
    ListenBrainzFeedbackItem, ListenBrainzLookupResponse, ListenBrainzPlaylistResponse,
    ListenBrainzPlaylistsResponse, ListenBrainzTrackResponse, MusicBrainzRecording,
};
use crate::music_api::{Album, Artist, MusicApiType, Playlist, Playlists, Song, Songs};

/// A `MusicBrainz` identifier is a plain UUID.
pub fn is_valid_mbid(id: &str) -> bool {
    let mut groups = id.split('-');
    let lengths = [8, 4, 4, 4, 12];
    for len in lengths {
        match groups.next() {
            Some(g) if g.len() == len && g.chars().all(|c| c.is_ascii_hexdigit()) => (),
            _ => return false,
        }
    }
    groups.next().is_none()
}

// multiples

impl TryInto<Playlists> for ListenBrainzPlaylistsResponse {
    type Error = Error;

    fn try_into(self) -> Result<Playlists, Self::Error> {
        let mut res = vec![];
        for wrapper in self.playlists {
            let playlist = match wrapper.playlist.try_into() {
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

// singles

impl TryInto<Playlist> for ListenBrainzPlaylistResponse {
    type Error = Error;

    fn try_into(self) -> Result<Playlist, Self::Error> {
        let id = self
            .identifier
            .strip_prefix(ListenBrainzApi::PLAYLIST_URI_PREFIX)
            .ok_or_eyre("playlist has no ListenBrainz identifier")?
            .to_string();

        let mut songs = vec![];
        for track in self.track {
            let song: Song = match track.try_into() {
                Ok(s) => s,
                Err(e) => {
                    error!("failed to parse song in response, skipping it: {}", e);
                    continue;
                }
            };
            songs.push(song);
        }

        Ok(Playlist {
            id,
            name: self.title,
            songs,
        })
    }
}

impl TryInto<Song> for ListenBrainzTrackResponse {
    type Error = Error;

    fn try_into(self) -> Result<Song, Self::Error> {
        let id = self
            .identifier
            .first_stripped(ListenBrainzApi::RECORDING_URI_PREFIX)
            .ok_or_eyre("track has no MusicBrainz recording identifier")?
            .to_string();

        let ext = self.extension.and_then(|e| e.track);
        let release_mbid = ext.as_ref().and_then(|e| {
            e.release_identifier
                .as_ref()?
                .strip_prefix(ListenBrainzApi::RELEASE_URI_PREFIX)
                .map(ToString::to_string)
        });
        let artist_mbid = ext.as_ref().and_then(|e| {
            e.artist_identifiers
                .first()?
                .strip_prefix(ListenBrainzApi::ARTIST_URI_PREFIX)
                .map(ToString::to_string)
        });

        Ok(Song {
            source: MusicApiType::ListenBrainz,
            id,
            sid: None,
            isrc: None,
            name: self.title.unwrap_or_default(),
            album: self.album.map(|name| Album {
                id: release_mbid,
                name,
                upc: None,
            }),
            // NOTE: `creator` is the combined artist credit ("A feat. B") and
            // cannot be split back into per-artist names.
            artists: self
                .creator
                .map(|name| {
                    vec![Artist {
                        id: artist_mbid,
                        name,
                    }]
                })
                .unwrap_or_default(),
            duration_ms: self.duration.unwrap_or(0),
        })
    }
}

impl TryInto<Song> for ListenBrainzLookupResponse {
    type Error = Error;

    fn try_into(self) -> Result<Song, Self::Error> {
        let id = self
            .recording_mbid
            .ok_or_eyre("lookup result has no recording mbid")?;

        Ok(Song {
            source: MusicApiType::ListenBrainz,
            id,
            sid: None,
            isrc: None,
            name: self.recording_name.unwrap_or_default(),
            album: self.release_name.map(|name| Album {
                id: self.release_mbid,
                name,
                upc: None,
            }),
            artists: self
                .artist_credit_name
                .map(|name| {
                    vec![Artist {
                        id: self.artist_mbids.into_iter().next(),
                        name,
                    }]
                })
                .unwrap_or_default(),
            // the mapper returns no recording length
            duration_ms: 0,
        })
    }
}

impl TryInto<Song> for ListenBrainzFeedbackItem {
    type Error = Error;

    fn try_into(self) -> Result<Song, Self::Error> {
        let mapping = self.track_metadata.as_ref().and_then(|m| m.mbid_mapping.as_ref());
        let id = self
            .recording_mbid
            .or_else(|| mapping.and_then(|m| m.recording_mbid.clone()))
            .ok_or_eyre("feedback entry has no recording mbid")?;

        let release_mbid = mapping.and_then(|m| m.release_mbid.clone());
        let artist_mbid = mapping.and_then(|m| m.artist_mbids.first().cloned());
        let meta = self.track_metadata;

        Ok(Song {
            source: MusicApiType::ListenBrainz,
            id,
            sid: None,
            isrc: None,
            name: meta.as_ref().map(|m| m.track_name.clone()).unwrap_or_default(),
            album: meta.as_ref().and_then(|m| m.release_name.clone()).map(|name| Album {
                id: release_mbid,
                name,
                upc: None,
            }),
            artists: meta
                .map(|m| {
                    vec![Artist {
                        id: artist_mbid,
                        name: m.artist_name,
                    }]
                })
                .unwrap_or_default(),
            duration_ms: 0,
        })
    }
}

pub fn feedback_to_songs(items: Vec<ListenBrainzFeedbackItem>) -> Songs {
    let mut res = vec![];
    for item in items {
        let song: Song = match item.try_into() {
            Ok(s) => s,
            Err(e) => {
                // expected for msid-only feedback
                tracing::debug!("skipping feedback entry: {}", e);
                continue;
            }
        };
        res.push(song);
    }
    Songs(res)
}

pub fn recording_to_song(recording: &MusicBrainzRecording, isrc: Option<&str>) -> Song {
    let credit = recording.credit_string();
    let artist_mbid = recording
        .artist_credit
        .first()
        .and_then(|c| c.artist.as_ref())
        .map(|a| a.id.clone());

    Song {
        source: MusicApiType::ListenBrainz,
        id: recording.id.clone(),
        sid: None,
        isrc: isrc.map(ToString::to_string),
        name: recording.title.clone().unwrap_or_default(),
        album: recording.releases.first().map(|r| Album {
            id: Some(r.id.clone()),
            name: r.title.clone(),
            upc: None,
        }),
        artists: if credit.is_empty() {
            vec![]
        } else {
            vec![Artist {
                id: artist_mbid,
                name: credit,
            }]
        },
        duration_ms: recording.length.unwrap_or(0),
    }
}
