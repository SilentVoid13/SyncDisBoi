mod args;

use crate::args::{MusicPlatform, RootArgs};

use sync_dis_boi::music_api::{DynMusicApi, MusicApi};
use sync_dis_boi::spotify::SpotifyApi;
use sync_dis_boi::sync::synchronize;
use sync_dis_boi::yt_music::YtMusicApi;

use clap::Parser;
use color_eyre::eyre::{eyre, Result};
use std::path::PathBuf;
use tracing::{debug, info, Level};
use tracing_subscriber::{filter::Targets, prelude::*};

async fn build_api(
    args: &RootArgs,
    platform: &MusicPlatform,
    config_dir: &PathBuf,
) -> Result<DynMusicApi> {
    let api: DynMusicApi = match platform {
        MusicPlatform::YtMusic => {
            let client_id = args
                .ytmusic
                .client_id_yt
                .as_ref()
                .ok_or(eyre!("Missing client ID"))?;
            let client_secret = args
                .ytmusic
                .client_secret_yt
                .as_ref()
                .ok_or(eyre!("Missing client secret"))?;
            let oauth_token_path = config_dir.join("ytmusic_oauth.json");
            Box::new(
                YtMusicApi::new(
                    &client_id,
                    &client_secret,
                    oauth_token_path,
                    args.debug,
                    args.proxy.as_deref(),
                )
                .await?,
            )
        }
        MusicPlatform::Spotify => {
            let client_id = args
                .spotify
                .client_id_sp
                .as_ref()
                .ok_or(eyre!("Missing client ID"))?;
            let client_secret = args
                .spotify
                .client_secret_sp
                .as_ref()
                .ok_or(eyre!("Missing client secret"))?;
            Box::new(
                SpotifyApi::new(
                    &client_id,
                    &client_secret,
                    args.debug,
                    args.proxy.as_deref(),
                )
                .await?,
            )
        }
    };
    Ok(api)
}

#[tokio::main]
async fn main() -> Result<()> {
    color_eyre::install()?;

    let args = RootArgs::parse();

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

    let src_api = build_api(&args, &args.src, &config_dir).await?;
    let dst_api = build_api(&args, &args.dst, &config_dir).await?;
    synchronize(src_api, dst_api, args.debug).await?;

    Ok(())
}
