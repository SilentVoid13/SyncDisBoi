mod args;

use sync_dis_boy::music_api::MusicApi;
use sync_dis_boy::spotify::SpotifyApi;
use sync_dis_boy::yt_music::YtMusicApi;

use anyhow::Result;
use clap::Parser;

#[tokio::main]
async fn main() -> Result<()> {
    let args = args::Args::parse();
    println!("{:?}", args);

    let music_api: YtMusicApi = YtMusicApi::new(&args.yt_music_cookies, &args.yt_music_secret)?;
    //let music_api: SpotifyApi = SpotifyApi::new(&args.spotify_client_id, &args.spotify_client_secret).await?;

    let mut playlist = music_api.create_playlist("Testy", false).await?;
    println!("{:?}", playlist);
    music_api.add_songs_to_playlist(&mut playlist, &["s_jEID5s9nE"]).await?;

    //let playlists = music_api.get_playlists_info().await?;
    //let playlists = music_api.get_playlists_full().await?;
    //println!("{:?}", playlists);
    //std::fs::write("res.json", serde_json::to_string_pretty(&playlists)?)?;

    Ok(())
}
