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

    let yt_music_api = YtMusicApi::new(&args.yt_music_cookies, &args.yt_music_secret)?;
    //let spotify_api = SpotifyApi::new(&args.spotify_client_id, &args.spotify_client_secret).await?;

    let playlists = yt_music_api.get_playlists_full().await?;
    //let playlists = spotify_api.get_playlists_info().await?;
    println!("{:?}", playlists);

    std::fs::write("res.json", serde_json::to_string_pretty(&playlists)?)?;

    Ok(())
}
