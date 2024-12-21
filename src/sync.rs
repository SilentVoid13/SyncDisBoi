use color_eyre::eyre::{eyre, Result};
use serde_json::json;
use tracing::{debug, info, warn};

use crate::music_api::{DynMusicApi, MusicApiType, Song};
use crate::utils::dedup_songs;
use crate::ConfigArgs;

// TODO: Parse playlist owner to ignore platform-specific playlists?
const SKIPPED_PLAYLISTS: [&str; 10] = [
    // Yt Music specific
    "New playlist",
    "Your Likes",
    "My Supermix",
    "Discover Mix",
    "Episodes for Later",
    // Spotify specific
    "Liked Songs",
    "Discover Weekly",
    "Big Room House Mix",
    "Motivation Electronic Mix",
    "High Energy Mix",
];

pub async fn synchronize(
    src_api: DynMusicApi,
    dst_api: DynMusicApi,
    config: ConfigArgs,
) -> Result<()> {
    if !config.diff_country
        && src_api.api_type() != MusicApiType::YtMusic
        && dst_api.api_type() != MusicApiType::YtMusic
        && src_api.country_code() != dst_api.country_code()
    {
        return Err(eyre!(
            "source and destination music platforms are in different countries ({} vs {}). \
                You can specify --diff-country to allow it, \
                but this might result in incorrect sync results.",
            src_api.country_code(),
            dst_api.country_code()
        ));
    }

    info!("retrieving playlists...");
    let src_playlists = src_api.get_playlists_full().await?;
    let mut dst_playlists = dst_api.get_playlists_full().await?;
    let dst_likes = dst_api.get_likes().await?;

    let mut all_missing_songs = json!({});
    let mut all_new_songs = json!({});
    let mut no_albums = json!({});
    let mut stats = json!({});

    for mut src_playlist in src_playlists
        .into_iter()
        .filter(|p| !SKIPPED_PLAYLISTS.contains(&p.name.as_str()) && !p.songs.is_empty())
    {
        if src_playlist.songs.is_empty() {
            continue;
        }

        let mut dst_playlist = match dst_playlists
            .iter()
            .position(|p| p.name == src_playlist.name)
        {
            Some(i) => dst_playlists.remove(i),
            None => dst_api.create_playlist(&src_playlist.name, false).await?,
        };

        let mut missing_songs = json!([]);
        let mut new_songs = json!([]);
        let mut no_albums_songs = json!([]);
        let mut dst_songs = vec![];
        let mut success = 0;
        let mut attempts = 0;

        if dedup_songs(&mut src_playlist.songs) {
            warn!(
                "duplicates found in source playlist \"{}\", they will be skipped",
                src_playlist.name
            );
        }

        info!("synchronizing playlist \"{}\" ...", src_playlist.name);
        for src_song in src_playlist.songs.iter() {
            if dst_playlist.songs.contains(src_song) {
                continue;
            }

            if src_song.album.is_none() {
                warn!(
                    "No album metadata for source song \"{}\", skipping",
                    src_song.name
                );
                if config.debug {
                    no_albums_songs
                        .as_array_mut()
                        .unwrap()
                        .push(json!(src_song));
                }
                continue;
            }

            attempts += 1;

            let dst_song = dst_api.search_song(src_song).await?;
            let Some(dst_song) = dst_song else {
                debug!("no match found for song: {}", src_song.name);
                if config.debug {
                    missing_songs.as_array_mut().unwrap().push(json!(src_song));
                }
                continue;
            };
            // HACK: takes into account discrepancy for YtMusic with no ISRC
            if dst_playlist.songs.contains(&dst_song) {
                debug!(
                    "discrepancy, song already in destination playlist: {}",
                    dst_song.name
                );
                continue;
            }
            // Edge case: same song on different album/single that all resolve to the same
            // song on the destination platform
            if dst_songs.contains(&dst_song) {
                debug!("song already in dst_songs: {}", dst_song.name);
                continue;
            }

            if config.debug {
                new_songs.as_array_mut().unwrap().push(json!(dst_song));
            }
            dst_songs.push(dst_song);
            success += 1;
        }
        if !dst_songs.is_empty() {
            dst_api
                .add_songs_to_playlist(&mut dst_playlist, &dst_songs)
                .await?;
            if config.like_all {
                let new_likes = dst_songs
                    .iter()
                    .filter(|s| !dst_likes.contains(s))
                    .cloned()
                    .collect::<Vec<Song>>();
                dst_api.add_like(&new_likes).await?;
            }
        }

        let mut conversion_rate = 1.0;
        if attempts != 0 {
            conversion_rate = success as f64 / attempts as f64;
            info!(
                "synchronizing playlist \"{}\" [ok], conversion rate: {}",
                src_playlist.name, conversion_rate
            );
        } else {
            info!(
                "synchronizing playlist \"{}\" [ok], no new songs to add",
                src_playlist.name
            );
        }

        if config.debug {
            stats.as_object_mut().unwrap().insert(
                src_playlist.name.clone(),
                serde_json::to_value(conversion_rate)?,
            );
            std::fs::write(
                "debug/conversion_rate.json",
                serde_json::to_string_pretty(&stats)?,
            )?;

            if !new_songs.as_array().unwrap().is_empty() {
                all_new_songs
                    .as_object_mut()
                    .unwrap()
                    .insert(src_playlist.name.clone(), new_songs);
                std::fs::write(
                    "debug/new_songs.json",
                    serde_json::to_string_pretty(&all_new_songs)?,
                )?;
            }

            if !missing_songs.as_array().unwrap().is_empty() {
                all_missing_songs
                    .as_object_mut()
                    .unwrap()
                    .insert(src_playlist.name.clone(), missing_songs);
                std::fs::write(
                    "debug/missing_songs.json",
                    serde_json::to_string_pretty(&all_missing_songs)?,
                )?;
            }

            if !no_albums_songs.as_array().unwrap().is_empty() {
                no_albums
                    .as_object_mut()
                    .unwrap()
                    .insert(src_playlist.name.clone(), no_albums_songs);
                std::fs::write(
                    "debug/song_with_no_albums.json",
                    serde_json::to_string_pretty(&no_albums)?,
                )?;
            }
        }
    }

    if config.sync_likes {
        info!("synchronizing likes...");
        let src_likes = src_api.get_likes().await?;
        let dst_likes = dst_api.get_likes().await?;

        let mut new_likes = Vec::new();
        for src_like in src_likes {
            if dst_likes.contains(&src_like) {
                continue;
            }
            let Some(song) = dst_api.search_song(&src_like).await? else {
                continue;
            };
            // HACK: takes into account discrepancy for YtMusic with no ISRC
            if dst_likes.contains(&song) {
                debug!("discrepancy, song already liked: {}", song.name);
                continue;
            }
            new_likes.push(song);
        }
        dst_api.add_like(&new_likes).await?;
    }

    info!("Synchronization complete.");

    Ok(())
}
