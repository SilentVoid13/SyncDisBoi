mod args;

use args::{MusicPlatform, RootArgs};
use sync_dis_boi::music_api::{DynMusicApi, MusicApi};
use sync_dis_boi::spotify::SpotifyApi;
use sync_dis_boi::sync::synchronize;
use sync_dis_boi::yt_music::YtMusicApi;

use clap::Parser;
use color_eyre::eyre::{eyre, Result};
use tracing::{debug, info, Level};
use tracing_subscriber::{filter::Targets, prelude::*};

async fn parse_api(args: &RootArgs, platform: &MusicPlatform) -> Result<DynMusicApi> {
    let api: Box<dyn MusicApi + Sync> = match platform {
        MusicPlatform::YtMusic => {
            let headers = args
                .ytmusic
                .headers
                .as_ref()
                .ok_or(eyre!("Missing headers file"))?;
            Box::new(YtMusicApi::new(&headers, args.debug)?)
        }
        MusicPlatform::Spotify => {
            let client_id = args
                .spotify
                .client_id
                .as_ref()
                .ok_or(eyre!("Missing client ID"))?;
            let client_secret = args
                .spotify
                .client_secret
                .as_ref()
                .ok_or(eyre!("Missing client secret"))?;
            Box::new(SpotifyApi::new(&client_id, &client_secret, args.debug).await?)
        }
    };
    Ok(api)
}

#[tokio::main]
async fn main() -> Result<()> {
    color_eyre::install()?;

    let args = RootArgs::parse();

    // Only shows logging for current crate, not sure if there is a cleaner way
    let level: Level = args.logging.clone().into();
    let filter = Targets::new().with_target("sync_dis_boi", Level::TRACE);
    tracing_subscriber::fmt()
        .with_max_level(level)
        .finish()
        .with(filter)
        .init();
    debug!("Logging level: {}", level);

    let src_api = parse_api(&args, &args.src).await?;
    let dst_api = parse_api(&args, &args.dst).await?;

    synchronize(src_api, dst_api, args.debug).await?;

    Ok(())
}
