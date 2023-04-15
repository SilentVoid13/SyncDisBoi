mod args;

use args::{MusicPlatform, RootArgs};
use sync_dis_boi::music_api::{MusicApi, DynMusicApi};
use sync_dis_boi::spotify::SpotifyApi;
use sync_dis_boi::sync::synchronize;
use sync_dis_boi::yt_music::YtMusicApi;

use anyhow::Result;
use clap::Parser;

async fn parse_api(args: &RootArgs, platform: &MusicPlatform) -> Result<DynMusicApi> {
    let api: Box<dyn MusicApi + Sync> = match platform {
        MusicPlatform::YtMusic => {
            let cookies = args
                .ytmusic
                .cookies
                .as_ref()
                .ok_or_else(|| anyhow::anyhow!("Missing cookies"))?;
            let secret = args
                .ytmusic
                .secret
                .as_ref()
                .ok_or_else(|| anyhow::anyhow!("Missing secret"))?;
            Box::new(YtMusicApi::new(&cookies, &secret)?)
        }
        MusicPlatform::Spotify => {
            let client_id = args
                .spotify
                .client_id
                .as_ref()
                .ok_or_else(|| anyhow::anyhow!("Missing client ID"))?;
            let client_secret = args
                .spotify
                .client_secret
                .as_ref()
                .ok_or_else(|| anyhow::anyhow!("Missing client secret"))?;
            Box::new(SpotifyApi::new(&client_id, &client_secret).await?)
        }
    };
    Ok(api)
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = RootArgs::parse();
    println!("{:?}", args);
    let src_api = parse_api(&args, &args.src).await?;
    let dst_api = parse_api(&args, &args.dst).await?;
    synchronize(src_api, dst_api, args.verbose, args.stats).await?;

    Ok(())
}
