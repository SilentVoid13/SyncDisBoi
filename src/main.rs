mod args;
mod build_api;

use std::path::Path;

use args::{MusicPlatformDst, RootArgs};
use build_api::BuildApi;
use clap::Parser;
use color_eyre::eyre::{eyre, Result};
use sync_dis_boi::export::export;
use sync_dis_boi::sync::synchronize;
use tracing::{debug, info, Level};
use tracing_subscriber::filter::Targets;
use tracing_subscriber::prelude::*;

#[tokio::main]
async fn main() -> Result<()> {
    color_eyre::install()?;
    // CLI arguments
    let args = RootArgs::parse();
    // Setup logging
    const CRATE_NAME: &str = env!("CARGO_PKG_NAME");
    let level: Level = args.logging.clone().into();
    let filter = Targets::new().with_target(CRATE_NAME, Level::TRACE);
    tracing_subscriber::fmt()
        .with_max_level(level)
        .with_target(true)
        .finish()
        .with(filter)
        .init();
    debug!("logging level: {}", level);

    let config_dir = dirs::config_dir().ok_or(eyre!("couldn't find system config dir"))?;
    let config_dir = config_dir.join("SyncDisBoi");
    if !config_dir.exists() {
        info!("creating SyncDisBoi config directory: {:?}", config_dir);
        std::fs::create_dir_all(&config_dir)?;
    }

    if args.config.debug {
        let debug_dir = Path::new("debug");
        if !debug_dir.exists() {
            debug!("creating debug directory");
            std::fs::create_dir("debug")?;
        }
    }

    let src_api = args.src.parse(&args, &config_dir).await?;
    match args.src.get_dst() {
        MusicPlatformDst::Export { dest, minify } => {
            export(src_api, dest, *minify).await?;
        }
        _ => {
            let dst_api = args.src.get_dst().parse(&args, &config_dir).await?;
            synchronize(src_api, dst_api, args.config).await?;
        }
    }

    Ok(())
}
