use crate::{
    music_api::{Album, Artist, Playlist, Playlists, Song, Songs},
    yt_music::model::PlaylistResponse,
};

use anyhow::{Context, Error, Result};
use serde::Deserialize;

impl TryFrom<serde_json::Value> for Playlists {
    type Error = Error;

    fn try_from(json: serde_json::Value) -> Result<Self, Self::Error> {
        let playlist_response = PlaylistResponse::deserialize(json)?;

        // TODO: Check for continuations

        let mut playlists = vec![];
        for mtrir in playlist_response
            .get_mtrirs()
            .context("Couldn't get response mtrirs")?
            .iter()
            // Ignore the first "New Playlist" playlist
            .skip(1)
        {
            let id = mtrir.get_id().context("no playlist id")?;
            let name = mtrir.get_name().context("no playlist name")?;
            let playlist = Playlist {
                id,
                name,
                songs: Option::None,
            };
            playlists.push(playlist);
        }

        let playlists = Playlists(playlists);
        Ok(playlists)
    }
}

impl TryFrom<serde_json::Value> for Songs {
    type Error = Error;

    fn try_from(json: serde_json::Value) -> Result<Self, Self::Error> {
        let playlist_response = PlaylistResponse::deserialize(json)?;

        let mut songs_vec = vec![];
        for mrlir in playlist_response
            .get_mrlirs()
            .context("Couldn't get response mrlirs")?
            .iter()
            // Ignore unavailable songs
            .filter(|item| item.playlist_item_data.is_some())
        {
            let id = mrlir.get_id().context("no song id")?;
            let set_id = mrlir.get_set_id().context("no song set_id")?;

            // fc0 = song title
            // fc1 = artists
            // fc2 = album

            let name = mrlir.get_flex_run_text(0, 0).context("no name")?;
            let album = mrlir.get_flex_runs(2).and_then(|_| {
                Some(Album {
                    id: mrlir.get_flex_run_id(2, 0),
                    name: mrlir.get_flex_run_text(2, 0)?,
                })
            });
            let mut artists: Vec<Artist> = vec![];
            for run in mrlir
                .get_flex_runs(1)
                .context("no flex col 1")?
                .iter()
                .step_by(2)
            {
                artists.push(Artist {
                    name: run.get_text(),
                    id: run.get_id(),
                });
            }
            let song = Song {
                id,
                set_id,
                name,
                artists,
                album,
            };

            songs_vec.push(song);
        }

        let songs: Songs = Songs(songs_vec);
        Ok(songs)
    }
}
