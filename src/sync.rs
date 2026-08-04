use color_eyre::eyre::{Result, eyre};
use futures::{StreamExt, stream};
use serde_json::json;
use tracing::{debug, info, warn};

use crate::ConfigArgs;
use crate::music_api::{DynMusicApi, MusicApiType, Playlist, Song, SongIndex};
use crate::utils::dedup_songs;

// TODO: Parse playlist owner to ignore platform-specific playlists?
pub const SKIPPED_PLAYLISTS: [&str; 12] = [
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
    // test playlists
    "TestSpotify",
    "SyncDisBoi-TestPlaylist",
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

    if config.debug {
        std::fs::create_dir_all("debug")?;
    }

    info!("retrieving source playlists...");
    let src_playlists = src_api.get_playlists_full().await?;

    synchronize_playlists(src_playlists, &dst_api, &config).await?;

    if config.sync_likes {
        synchronize_likes(&src_api, &dst_api, &config).await?;
    }

    Ok(())
}

pub async fn synchronize_playlists(
    src_playlists: Vec<Playlist>,
    dst_api: &DynMusicApi,
    config: &ConfigArgs,
) -> Result<()> {
    let mut all_missing_songs = json!({});
    let mut all_new_songs = json!({});
    let mut no_albums = json!({});
    let mut stats = json!({});

    info!("retrieving destination playlists...");
    let mut dst_playlists = dst_api.get_playlists_full().await?;
    let mut dst_likes = vec![];
    if config.like_all {
        info!("retrieving destination likes...");
        dst_likes = dst_api.get_likes().await?;
    }
    let dst_likes_index = SongIndex::build(&dst_likes);

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

        let dst_playlist_index = SongIndex::build(&dst_playlist.songs);

        // 1. Search for each song in the destination playlist
        // First, filter out songs that don't need a search (cheap, sequential).
        let to_search: Vec<&Song> = src_playlist
            .songs
            .iter()
            .filter(|src_song| {
                // already in destination playlist
                if dst_playlist_index.contains(src_song, config.strip_qualifiers) {
                    return false;
                }
                // no album metadata == youtube video
                if src_song.album.is_none() {
                    warn!(
                        "No album metadata for source song \"{}\", skipping",
                        src_song
                    );
                    if config.debug {
                        no_albums_songs
                            .as_array_mut()
                            .unwrap()
                            .push(json!(src_song));
                    }
                    return false;
                }
                true
            })
            .collect();

        // Then run the remaining searches concurrently (bounded, order-preserving)
        // instead of awaiting them one at a time, since each search can involve
        // several sequential HTTP round-trips on the destination platform.
        attempts += i32::try_from(to_search.len()).unwrap_or(i32::MAX);
        let search_results: Vec<(&Song, Result<Option<Song>>)> = stream::iter(to_search)
            .map(|src_song| async move { (src_song, dst_api.search_song(src_song).await) })
            .buffered(config.search_concurrency.max(1))
            .collect()
            .await;

        for (src_song, result) in search_results {
            let dst_song = result?;
            let Some(dst_song) = dst_song else {
                debug!("no match found for song: {}", src_song);
                if config.debug {
                    missing_songs.as_array_mut().unwrap().push(json!(src_song));
                }
                continue;
            };
            dst_songs.push(dst_song);
            success += 1;
        }

        // 2. Add missing songs to the destination playlist
        if !dst_songs.is_empty() {
            let mut to_sync = Vec::new();
            for dst_song in &dst_songs {
                // HACK: takes into account discrepancy for YtMusic with no ISRC
                if dst_playlist_index.contains(dst_song, config.strip_qualifiers) {
                    debug!(
                        "discrepancy, song already in destination playlist: {}",
                        dst_song
                    );
                    attempts -= 1;
                    success -= 1;
                    continue;
                }
                // Edge case: same song on different album/single that all resolve to the same
                // song on the destination platform resulting in duplicates
                if to_sync.contains(dst_song) {
                    debug!(
                        "discrepancy, duplicate song in songs to synchronize: {}",
                        dst_song
                    );
                    attempts -= 1;
                    success -= 1;
                    continue;
                }
                if config.debug {
                    new_songs.as_array_mut().unwrap().push(json!(dst_song));
                }
                to_sync.push(dst_song.clone());
            }
            debug!(
                "adding {} songs to destination playlist \"{}\"",
                to_sync.len(),
                dst_playlist.name
            );
            dst_api
                .add_songs_to_playlist(&mut dst_playlist, &to_sync)
                .await?;

            // like all songs that were added
            if config.like_all {
                let new_likes = to_sync
                    .iter()
                    .filter(|s| !dst_likes_index.contains(s, config.strip_qualifiers))
                    .cloned()
                    .collect::<Vec<Song>>();
                dst_api.add_likes(&new_likes).await?;
            }
        }

        let mut conversion_rate = 1.0;
        if attempts != 0 {
            conversion_rate = f64::from(success) / f64::from(attempts);
            info!(
                "synchronizing playlist \"{}\" [ok], {}/{} songs ({:.2}%)",
                src_playlist.name,
                success,
                attempts,
                conversion_rate * 100.0
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
                json!({
                    "percentage": conversion_rate,
                    "number": format!("{}/{}", success, attempts),
                }),
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

    info!("Synchronization complete!");

    Ok(())
}

pub async fn synchronize_likes(
    src_api: &DynMusicApi,
    dst_api: &DynMusicApi,
    config: &ConfigArgs,
) -> Result<()> {
    info!("retrieving source likes...");
    let src_likes = src_api.get_likes().await?;
    info!("retrieving destination likes...");
    let dst_likes = dst_api.get_likes().await?;

    let mut new_likes = Vec::new();
    let mut success = 0;
    let mut attempts = 0;

    info!("searching for all missing likes on destination platform...");
    let dst_likes_index = SongIndex::build(&dst_likes);
    let to_search: Vec<Song> = src_likes
        .into_iter()
        .filter(|src_like| !dst_likes_index.contains(src_like, config.strip_qualifiers))
        .collect();

    attempts += i32::try_from(to_search.len()).unwrap_or(i32::MAX);
    let search_results: Vec<(Song, Result<Option<Song>>)> = stream::iter(to_search)
        .map(|src_like| async move {
            let result = dst_api.search_song(&src_like).await;
            (src_like, result)
        })
        .buffered(config.search_concurrency.max(1))
        .collect()
        .await;

    for (src_like, result) in search_results {
        let Some(song) = result? else {
            debug!("no match found for song: {}", src_like);
            continue;
        };
        // HACK: takes into account discrepancy for YtMusic with no ISRC
        if dst_likes_index.contains(&song, config.strip_qualifiers) {
            attempts -= 1;
            debug!("discrepancy, song already liked: {}", song);
            continue;
        }
        success += 1;
        new_likes.push(song);
    }

    if attempts != 0 {
        let conversion_rate = f64::from(success) / f64::from(attempts);
        info!(
            "synchronizing {}/{} ({:.2}%) new likes",
            success,
            attempts,
            conversion_rate * 100.0
        );
        dst_api.add_likes(&new_likes).await?;
        info!("[ok] synchronized new likes");
    } else {
        info!("[ok] no new likes to synchronize");
    }

    Ok(())
}
