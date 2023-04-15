use crate::music_api::{DynMusicApi, MusicApi, Playlist};

use anyhow::Result;
use serde_json::json;

pub async fn synchronize(
    src_api: DynMusicApi,
    dst_api: Box<dyn MusicApi + Sync>,
    verbose: bool,
    stats: bool,
) -> Result<()> {
    // TODO: remove this
    let src_playlists = src_api.get_playlists_full().await?;
    let val = json!(src_playlists);
    std::fs::write("src_playlists.json", serde_json::to_string(&val).unwrap()).unwrap();

    // TODO: remove this
    //let src_playlists: Playlists = serde_json::from_str(&std::fs::read_to_string("src_playlists.json").unwrap()).unwrap();
    //let src_playlists = src_playlists.0;

    // TODO: remove this
    // Delete all playlists
    let mut dst_playlists = dst_api.get_playlists_full().await?;
    for p in dst_playlists.iter() {
        dst_api.delete_playlist(&p.id).await?;
    }
    let mut dst_playlists: Vec<Playlist> = vec![];

    // TODO: remove this
    let mut missing_output = json!({});
    let mut no_albums = json!({});
    let mut stats = json!({});

    for src_playlist in src_playlists.iter().skip(5).filter(|p| p.songs.is_some()) {
        if verbose {
            println!("[*] Synchronizing playlist \"{}\" ...", src_playlist.name);
        }

        let src_songs = match &src_playlist.songs {
            Some(s) => s,
            None => {
                if verbose {
                    println!("[!] No songs in source playlist \"{}\".", src_playlist.name);
                }
                continue;
            }
        };

        let dst_playlist = match dst_playlists
            .iter()
            .position(|p| p.name == src_playlist.name)
        {
            Some(i) => dst_playlists.remove(i),
            None => dst_api.create_playlist(&src_playlist.name, false).await?,
        };

        // TODO: remove this
        let mut missing_songs = json!([]);
        let mut no_albums_songs = json!([]);
        let mut dst_songs_ids = vec![];
        let mut success = 0;
        let mut total = 0;

        for src_song in src_songs.iter() {
            if let Some(s) = dst_playlist.songs.as_ref() {
                if s.contains(src_song) {
                    continue;
                }
            }

            if src_song.album.is_none() {
                no_albums_songs
                    .as_array_mut()
                    .unwrap()
                    .push(json!(src_song));
                println!("No album for source song, skipping: {}", src_song.name);
                continue;
            }
            total += 1;

            let dst_song = dst_api.search_song(src_song).await?;
            if let Some(s) = dst_song {
                if src_song.compare(&s) {
                    dst_songs_ids.push(s.id);
                    success += 1;
                } else {
                    // TODO: remove this
                    missing_songs.as_array_mut().unwrap().push(json!(src_song));
                }
            } else {
                // TODO: remove this
                missing_songs.as_array_mut().unwrap().push(json!(src_song));
            }
        }
        if !dst_songs_ids.is_empty() {
            dst_api
                .add_songs_to_playlist(&dst_playlist.id, &dst_songs_ids)
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
        std::fs::write("stats.json", stats.to_string()).unwrap();

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

            std::fs::write("missing.json", missing_output.to_string()).unwrap();
            std::fs::write("no_albums.json", no_albums.to_string()).unwrap();
        }
    }

    println!("[+] Synchronization complete.");

    Ok(())
}
