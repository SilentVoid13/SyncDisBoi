use crate::args::{MusicPlatformDst, MusicPlatformSrc, RootArgs};
use async_trait::async_trait;
use color_eyre::eyre::{eyre, Result};
use std::path::PathBuf;
use sync_dis_boi::music_api::{DynMusicApi, MusicApi};
use sync_dis_boi::spotify::SpotifyApi;
use sync_dis_boi::yt_music::YtMusicApi;

#[async_trait]
pub trait BuildApi {
    async fn parse(&self, args: &RootArgs, config_dir: &PathBuf) -> Result<DynMusicApi>;
}

// TODO: Hack to support command chaining with clap
// related issue: https://github.com/clap-rs/clap/issues/2222
impl_build_api!(MusicPlatformSrc);
impl_build_api!(MusicPlatformDst);

impl MusicPlatformSrc {
    pub fn get_dst(&self) -> &MusicPlatformDst {
        match self {
            Self::YtMusic { dst, .. } => dst,
            Self::Spotify { dst, .. } => dst,
        }
    }
}

#[macro_export]
macro_rules! impl_build_api {
    ($id:ident) => {
        #[async_trait]
        impl BuildApi for $id {
            async fn parse(&self, args: &RootArgs, config_dir: &PathBuf) -> Result<DynMusicApi> {
                let api: DynMusicApi = match &self {
                    Self::YtMusic {
                        client_id,
                        client_secret,
                        ..
                    } => {
                        let oauth_token_path = config_dir.join("ytmusic_oauth.json");
                        Box::new(
                            YtMusicApi::new(
                                client_id,
                                client_secret,
                                oauth_token_path,
                                args.debug,
                                args.proxy.as_deref(),
                            )
                            .await?,
                        )
                    }
                    Self::Spotify {
                        client_id,
                        client_secret,
                        ..
                    } => Box::new(
                        SpotifyApi::new(
                            &client_id,
                            &client_secret,
                            args.debug,
                            args.proxy.as_deref(),
                        )
                        .await?,
                    ),
                };
                Ok(api)
            }
        }
    };
}
