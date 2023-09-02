use crate::music_api::{DynMusicApi, MusicApi, Playlist};

use color_eyre::eyre::Result;
use serde_json::json;
use tracing::{info, warn};

// TODO: Parse playlist owner to ignore platform-specific playlists?
const SKIPPED_PLAYLISTS: [&str; 8] = [
    "New playlist",
    "Your Likes",
    "My Supermix",
    "Liked Songs",
    "Discover Weekly",
    "Big Room House Mix",
    "High Energy Mix",
    "Motivation Electronic Mix",
];

pub async fn synchronize(
    src_api: DynMusicApi,
    dst_api: Box<dyn MusicApi + Sync>,
    stats: bool,
) -> Result<()> {
    // TODO: remove this
    info!("Retrieving src_playlist songs...");
    let src_playlists = src_api.get_playlists_full().await?;
    let val = json!(src_playlists);
    std::fs::write("json/src_playlists.json", serde_json::to_string(&val).unwrap()).unwrap();

    // TODO: remove this
    //let src_playlists: Playlists = serde_json::from_str(&std::fs::read_to_string("json/src_playlists.json").unwrap()).unwrap();
    //let src_playlists = src_playlists.0;

    let dst_playlists = dst_api.get_playlists_full().await?;

    // TODO: remove this
    // Delete all playlists
    info!("Deleting all playlists on destination ...");
    for p in dst_playlists.into_iter().filter(|p| !SKIPPED_PLAYLISTS.contains(&p.name.as_str())) {
        info!("Deleting playlist \"{}\" ...", p.name);
        dst_api.delete_playlist(p).await?;
    }
    let mut dst_playlists: Vec<Playlist> = vec![];
    info!("Finished deletion");

    // TODO: remove this
    let mut missing_output = json!({});
    let mut no_albums = json!({});
    let mut stats = json!({});

    for src_playlist in src_playlists
        .iter()
        .filter(|p| !SKIPPED_PLAYLISTS.contains(&p.name.as_str()) && !p.songs.is_empty())
    {
        info!("Synchronizing playlist \"{}\" ...", src_playlist.name);

        let mut dst_playlist = match dst_playlists
            .iter()
            .position(|p| p.name == src_playlist.name)
        {
            Some(i) => dst_playlists.remove(i),
            None => dst_api.create_playlist(&src_playlist.name, false).await?,
        };

        // TODO: remove this
        let mut missing_songs = json!([]);
        let mut no_albums_songs = json!([]);
        let mut dst_songs = vec![];
        let mut success = 0;
        let mut total = 0;

        for src_song in src_playlist.songs.iter() {
            if dst_playlist.songs.contains(src_song) {
                continue;
            }

            if src_song.album.is_none() {
                no_albums_songs
                    .as_array_mut()
                    .unwrap()
                    .push(json!(src_song));
                warn!(
                    "No album metadata for source song, skipping: {}",
                    src_song.name
                );
                continue;
            }
            total += 1;

            let dst_song = dst_api.search_song(src_song).await?;
            if let Some(s) = dst_song {
                if src_song.compare(&s) {
                    dst_songs.push(s);
                    success += 1;
                } else {
                    // TODO: remove this
                    missing_songs.as_array_mut().unwrap().push(json!(src_song));
                }
            } else {
                info!("Song not found on destination: {}", src_song.name);
                // TODO: remove this
                missing_songs.as_array_mut().unwrap().push(json!(src_song));
            }
        }
        if !dst_songs.is_empty() {
            dst_api
                .add_songs_to_playlist(&mut dst_playlist, &dst_songs)
                .await?;
        }

        if total == 0 {
            total = 1;
        }
        let conversion_rate = success as f64 / total as f64;
        stats.as_object_mut().unwrap().insert(
            src_playlist.name.clone(),
            serde_json::to_value(conversion_rate).unwrap(),
        );
        std::fs::write("json/stats.json", stats.to_string()).unwrap();

        // TODO: remove this
        if !missing_songs.as_array().unwrap().is_empty() {
            missing_output
                .as_object_mut()
                .unwrap()
                .insert(src_playlist.name.clone(), missing_songs);
            no_albums
                .as_object_mut()
                .unwrap()
                .insert(src_playlist.name.clone(), no_albums_songs);

            std::fs::write("json/missing.json", missing_output.to_string()).unwrap();
            std::fs::write("json/no_albums.json", no_albums.to_string()).unwrap();
        }
    }

    info!("Synchronization complete.");

    Ok(())
}
