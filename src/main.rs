mod args;
mod build_api;

use args::RootArgs;
use build_api::BuildApi;
use sync_dis_boi::music_api::{DynMusicApi, MusicApi};
use sync_dis_boi::spotify::SpotifyApi;
use sync_dis_boi::sync::synchronize;
use sync_dis_boi::yt_music::YtMusicApi;

use clap::{FromArgMatches, Parser};
use color_eyre::eyre::{eyre, Result};
use std::path::PathBuf;
use tracing::{debug, info, Level};
use tracing_subscriber::{filter::Targets, prelude::*};

#[tokio::main]
async fn main() -> Result<()> {
    color_eyre::install()?;

    let args = RootArgs::parse();
    dbg!(&args);
    //let dst = MusicPlatform::parse_from(&args.dst_args);
    //dbg!(&dst);

    let config_dir = dirs::config_dir().ok_or(eyre!("couldn't find system config dir"))?;
    let config_dir = config_dir.join("SyncDisBoi");
    if !config_dir.exists() {
        info!("Creating config directory");
        std::fs::create_dir_all(&config_dir)?;
    }

    // Only shows logging for current crate, not sure if there is a cleaner way
    let level: Level = args.logging.clone().into();
    let filter = Targets::new().with_target("sync_dis_boi", Level::TRACE);
    tracing_subscriber::fmt()
        .with_max_level(level)
        .finish()
        .with(filter)
        .init();
    debug!("Logging level: {}", level);

    let src_api = args.src.parse(&args, &config_dir).await?;
    let dst_api = args.src.get_dst().parse(&args, &config_dir).await?;
    synchronize(src_api, dst_api, args.debug).await?;

    Ok(())
}
