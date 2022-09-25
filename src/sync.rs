use crate::music_api::MusicApi;

use anyhow::Result;

pub async fn synchronize<T, U>(src_api: T, dest_api: U) -> Result<()>
where
    T: MusicApi + Sync,
    U: MusicApi + Sync,
{
    let src_playlists = src_api.get_playlists_full().await?;
    let dest_playlists = dest_api.get_playlists_info().await?;

    let src_playlist = src_playlists.get(6).unwrap();

    //for src_playlist in src_playlists.iter() {
        let src_songs = match &src_playlist.songs {
            Some(s) => s,
            //None => continue,
            None => std::process::exit(1),
        };
        let dest_playlist = match dest_playlists.iter().find(|p| p.name == src_playlist.name) {
            Some(p) => p.id.clone(),
            None => {
                dest_api
                    .create_playlist(&src_playlist.name, false)
                    .await?
                    .id
            }
        };
        let mut dest_songs_ids = vec![];
        for src_song in src_songs.iter() {
            // TODO: check if song is already in playlist
            let dest_song = dest_api.search_song(src_song).await?;
            if let Some(s) = dest_song {
                dest_songs_ids.push(s.id);
            } else {
                println!("NOT FOUND: {:?}", src_song);
            }
        }
        dest_api
            .add_songs_to_playlist(&dest_playlist, &dest_songs_ids)
            .await?;
    //}
    Ok(())
}
