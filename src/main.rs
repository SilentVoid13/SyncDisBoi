mod args;

use sync_dis_boy::yt_music::YtMusicApi;
use sync_dis_boy::music_api::MusicApi;

use clap::Parser;
use anyhow::Result;

#[tokio::main]
async fn main() -> Result<()> {
    let args = args::Args::parse();
    println!("{:?}", args);

    let yt_music_api = YtMusicApi::new();
    let playlists = yt_music_api.get_playlists_full().await?;
    println!("{:?}", playlists);

    Ok(())
}
