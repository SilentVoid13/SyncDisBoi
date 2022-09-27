mod args;

use sync_dis_boy::music_api::{MusicApi, Song, Album, Artist};
use sync_dis_boy::spotify::SpotifyApi;
use sync_dis_boy::sync::synchronize;
use sync_dis_boy::utils::{clean_song_name, clean_bad_chars_spotify};
use sync_dis_boy::yt_music::YtMusicApi;

use anyhow::Result;
use clap::Parser;

#[tokio::main]
async fn main() -> Result<()> {
    let args = args::Args::parse();
    println!("{:?}", args);

    let music_api1: YtMusicApi = YtMusicApi::new(&args.yt_music_cookies, &args.yt_music_secret)?;
    let music_api2: SpotifyApi = SpotifyApi::new(&args.spotify_client_id, &args.spotify_client_secret).await?;

    synchronize(music_api1, music_api2).await?;
    
    //let name = "Light (feat. Tropics) - Qrion Remix".to_string();
    //let clean_name = clean_song_name(&name);
    //println!("{}", clean_name);

    //let mut playlist = music_api.create_playlist("Testy", false).await?;
    //println!("{:?}", playlist);
    //music_api.add_songs_to_playlist(&mut playlist, &["s_jEID5s9nE"]).await?;

    //let playlists = music_api.get_playlists_full().await?;
    //let playlist = &playlists[2];
    //println!("{:?}", playlist);
    //for song in playlist.songs.as_ref().unwrap().iter() {
    //    let res = music_api.search_song(&song).await?;
    //    println!("RES: {:?}", res);
    //}

    //let playlists = music_api.get_playlists_info().await?;
    //let playlists = music_api.get_playlists_full().await?;
    //println!("{:?}", playlists);
    //std::fs::write("res.json", serde_json::to_string_pretty(&playlists)?)?;

    Ok(())
}
