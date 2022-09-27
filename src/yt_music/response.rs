use crate::{
    music_api::{Album, Artist, Playlist, Playlists, Song, Songs},
    yt_music::model::YtMusicResponse, utils::clean_song_name,
};

use anyhow::{Context, Error, Result};

pub fn parse_duration(duration_str: &str) -> Result<usize> {
    let multipliers = [1, 60, 3600];
    let mut seconds = 0;
    let mut i = 0;
    for part in duration_str.rsplit(":") {
        seconds += part.parse::<usize>()? * multipliers[i];
        i+=1;
    }
    Ok(seconds)
}

impl TryInto<Playlists> for YtMusicResponse {
    type Error = Error;

    fn try_into(mut self) -> Result<Playlists, Self::Error> {
        let mut playlists = vec![];
        for mtrir in self
            .get_mtrirs()
            .context("Couldn't get response mtrirs")?
            .iter()
            // Ignore the first "New Playlist" playlist
            // Ignore the second "Your Likes" playlist
            .skip(2)
        {
            let id = mtrir.get_id().context("no playlist id")?;
            let name = mtrir.get_name().context("no playlist name")?;
            let playlist = Playlist {
                id,
                name,
                songs: None,
            };
            playlists.push(playlist);
        }

        let playlists = Playlists(playlists);
        Ok(playlists)
    }
}

impl TryInto<Songs> for YtMusicResponse {
    type Error = Error;

    fn try_into(mut self) -> Result<Songs, Self::Error> {
        let mut songs_vec = vec![];

        let mrlirs = match self.get_mrlirs() {
            Some(x) => x,
            // No songs in the playlist
            None => return Ok(Songs(songs_vec))
        };

        for mrlir in mrlirs
            .iter()
            // Ignore unavailable songs
            .filter(|item| item.playlist_item_data.is_some())
        {
            let id = mrlir.get_id().context("no song id")?;
            let set_id = mrlir.get_set_id().context("no song set_id")?;

            let duration_str = mrlir.get_col_run_text(0, 0, false).context("no duration")?;
            let duration = parse_duration(&duration_str)?;

            // fc0 = song title
            // fc1 = artists
            // fc2 = album

            let name = mrlir.get_col_run_text(0, 0, true).context("no name")?;
            let album = mrlir.get_col_runs(2, true).and_then(|_| {
                Some(Album {
                    id: mrlir.get_col_run_id(2, 0, true),
                    name: mrlir.get_col_run_text(2, 0, true)?,
                })
            });
            let mut artists: Vec<Artist> = vec![];
            for run in mrlir
                .get_col_runs(1, true)
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
                sid: Some(set_id),
                clean_name: clean_song_name(&name),
                name,
                artists,
                album,
                duration,
            };

            songs_vec.push(song);
        }

        let songs: Songs = Songs(songs_vec);
        Ok(songs)
    }
}
