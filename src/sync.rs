use color_eyre::eyre::{eyre, Result};
use serde_json::json;
use tracing::{debug, info, warn};

use crate::music_api::{DynMusicApi, MusicApiType};

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
    debug: bool,
) -> Result<()> {
    if src_api.api_type() != MusicApiType::YtMusic && dst_api.api_type() != MusicApiType::YtMusic && src_api.country_code() != dst_api.country_code() {
        return Err(eyre!("source and destination music platforms are in different countries: {} vs {}, this can lead to unexpected results", src_api.country_code(), dst_api.country_code()));
    }

    info!("retrieving playlists...");
    let src_playlists = src_api.get_playlists_full().await?;
    let mut dst_playlists = dst_api.get_playlists_full().await?;

    let mut all_missing_songs = json!({});
    let mut all_new_songs = json!({});
    let mut no_albums = json!({});
    let mut stats = json!({});

    for src_playlist in src_playlists
        .iter()
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
                if debug {
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
                if debug {
                    missing_songs.as_array_mut().unwrap().push(json!(src_song));
                }
                continue;
            };
            if debug {
                new_songs.as_array_mut().unwrap().push(json!(dst_song));
            }
            dst_songs.push(dst_song);
            success += 1;
        }
        if !dst_songs.is_empty() {
            dst_api
                .add_songs_to_playlist(&mut dst_playlist, &dst_songs)
                .await?;
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

        if debug {
            stats.as_object_mut().unwrap().insert(
                src_playlist.name.clone(),
                serde_json::to_value(conversion_rate).unwrap(),
            );
            std::fs::write(
                "debug/conversion_rate.json",
                serde_json::to_string_pretty(&stats)?,
            )
            .unwrap();

            if !new_songs.as_array().unwrap().is_empty() {
                all_new_songs
                    .as_object_mut()
                    .unwrap()
                    .insert(src_playlist.name.clone(), new_songs);
                std::fs::write(
                    "debug/new_songs.json",
                    serde_json::to_string_pretty(&all_new_songs)?,
                )
                .unwrap();
            }

            if !missing_songs.as_array().unwrap().is_empty() {
                all_missing_songs
                    .as_object_mut()
                    .unwrap()
                    .insert(src_playlist.name.clone(), missing_songs);
                std::fs::write(
                    "debug/missing_songs.json",
                    serde_json::to_string_pretty(&all_missing_songs)?,
                )
                .unwrap();
            }

            if !no_albums_songs.as_array().unwrap().is_empty() {
                no_albums
                    .as_object_mut()
                    .unwrap()
                    .insert(src_playlist.name.clone(), no_albums_songs);
                std::fs::write(
                    "debug/song_with_no_albums.json",
                    serde_json::to_string_pretty(&no_albums)?,
                )
                .unwrap();
            }
        }
    }

    info!("Synchronization complete.");

    Ok(())
}
