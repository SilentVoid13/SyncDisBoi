use crate::music_api::{MusicApi, Playlists};

use anyhow::Result;

use serde_json::json;

pub async fn synchronize<T, U>(src_api: T, dest_api: U) -> Result<()>
where
    T: MusicApi + Sync,
    U: MusicApi + Sync,
{
    // TODO: remove this
    //let src_playlists = src_api.get_playlists_full().await?;
    //let val = json!(src_playlists);
    //std::fs::write("src_playlists.json", serde_json::to_string(&val).unwrap()).unwrap();

    let src_playlists: Playlists = serde_json::from_str(&std::fs::read_to_string("src_playlists.json").unwrap()).unwrap();
    let src_playlists = src_playlists.0;

    let mut dest_playlists = dest_api.get_playlists_full().await?;
    for p in dest_playlists.iter() {
        dest_api.delete_playlist(&p.id).await?;
    }
    let mut dest_playlists = dest_api.get_playlists_full().await?;

    // TODO: remove this
    let mut missing_output = json!({});
    let mut stats = json!({});

    for src_playlist in src_playlists.iter().skip(5) {
        let src_songs = match &src_playlist.songs {
            Some(s) => s,
            None => continue,
        };

        // TODO: improve this check
        let dest_playlist = match dest_playlists
            .iter()
            .position(|p| p.name == src_playlist.name)
        {
            Some(i) => dest_playlists.remove(i),
            None => dest_api.create_playlist(&src_playlist.name, false).await?,
        };
        // TODO: remove this
        let mut missing_songs = json!([]);
        let mut dest_songs_ids = vec![];
        for src_song in src_songs.iter() {
            // TODO: improve this check
            if dest_playlist.songs.as_ref().map_or(false, |v| {
                v.iter().any(|s| s.clean_name == src_song.clean_name)
            }) {
                continue;
            }

            let dest_song = dest_api.search_song(src_song, true).await?;
            if let Some(s) = dest_song {
                if !(src_song.duration-1..src_song.duration+1).contains(&s.duration) {
                    println!("{}: {} vs {}", s.name, s.duration, src_song.duration);
                }
                dest_songs_ids.push(s.id);
            } else {
                // TODO: remove this
                missing_songs.as_array_mut().unwrap().push(json!(src_song));
            }
        }
        if !dest_songs_ids.is_empty() {
            dest_api
                .add_songs_to_playlist(&dest_playlist.id, &dest_songs_ids)
                .await?;
        }

        let conversion_rate = dest_songs_ids.len() as f64 / src_songs.len() as f64;
        println!("conversion rate {}/{}: {}", dest_songs_ids.len(), src_songs.len(), conversion_rate);
        stats
            .as_object_mut()
            .unwrap()
            .insert(src_playlist.name.clone(), serde_json::to_value(conversion_rate).unwrap());

        // TODO: remove this
        if !missing_songs.as_array().unwrap().is_empty() {
            missing_output
                .as_object_mut()
                .unwrap()
                .insert(src_playlist.name.clone(), missing_songs);
            std::fs::write("missing.json", missing_output.to_string()).unwrap();
            std::fs::write("stats.json", stats.to_string()).unwrap();
        }
    }

    Ok(())
}
