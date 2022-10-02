mod args;

use sync_dis_boi::music_api::{MusicApi, Song, Album, Artist};
use sync_dis_boi::spotify::SpotifyApi;
use sync_dis_boi::sync::synchronize;
use sync_dis_boi::utils::{generic_name_clean, clean_enclosure};
use sync_dis_boi::yt_music::YtMusicApi;

use anyhow::Result;
use clap::Parser;

#[tokio::main]
async fn main() -> Result<()> {
    let args = args::Args::parse();
    println!("{:?}", args);

    let music_api1: YtMusicApi = YtMusicApi::new(&args.yt_music_cookies, &args.yt_music_secret)?;
    let music_api2: SpotifyApi = SpotifyApi::new(&args.spotify_client_id, &args.spotify_client_secret).await?;
    
    let clean = clean_enclosure("BoJack's Theme (Full Length) (feat. Ralph Carney)", "(", ")");
    println!("CLEAN: {:?}", clean);

    synchronize(music_api1, music_api2).await?;
    
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
