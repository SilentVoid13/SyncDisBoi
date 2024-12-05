use std::path::{Path, PathBuf};

use async_trait::async_trait;
use color_eyre::eyre::{eyre, Result};
use sync_dis_boi::music_api::DynMusicApi;
use sync_dis_boi::spotify::SpotifyApi;
use sync_dis_boi::tidal::TidalApi;
use sync_dis_boi::yt_music::YtMusicApi;

use crate::args::{MusicPlatformDst, MusicPlatformSrc, RootArgs};

#[async_trait]
pub trait BuildApi {
    async fn parse(&self, args: &RootArgs, config_dir: &Path) -> Result<DynMusicApi>;
}

#[macro_export]
macro_rules! impl_build_api {
    ($id:ident) => {
        #[async_trait]
        impl BuildApi for $id {
            async fn parse(&self, args: &RootArgs, config_dir: &Path) -> Result<DynMusicApi> {
                let api: DynMusicApi = match &self {
                    Self::YtMusic {
                        /*
                        client_id,
                        client_secret,
                        clear_cache,
                        */
                        headers,
                        ..
                    } => {
                        Box::new(
                            YtMusicApi::new(headers, args.config.clone())
                                .await?,
                        )
                        /*
                        let oauth_token_path = config_dir.join("ytmusic_oauth.json");
                        Box::new(
                            YtMusicApi::new(
                                client_id,
                                client_secret,
                                oauth_token_path,
                                *clear_cache,
                                args.config.clone(),
                            )
                            .await?,
                        )
                        */
                    }
                    Self::Tidal {
                        client_id,
                        client_secret,
                        clear_cache,
                        ..
                    } => {
                        let oauth_token_path = config_dir.join("tidal_oauth.json");
                        Box::new(
                            TidalApi::new(
                                client_id,
                                client_secret,
                                oauth_token_path,
                                *clear_cache,
                                args.config.clone(),
                            )
                            .await?,
                        )
                    }
                    Self::Spotify {
                        client_id,
                        client_secret,
                        ..
                    } => Box::new(
                        SpotifyApi::new(&client_id, &client_secret, args.config.clone()).await?,
                    ),
                    #[allow(unreachable_patterns)]
                    _ => return Err(eyre!("Invalid API type: {:?}", self)),
                };
                Ok(api)
            }
        }
    };
}

// INFO: Hack to support command chaining with clap
// related issue: https://github.com/clap-rs/clap/issues/2222
impl_build_api!(MusicPlatformSrc);
impl_build_api!(MusicPlatformDst);

impl MusicPlatformSrc {
    pub fn get_dst(&self) -> &MusicPlatformDst {
        match self {
            Self::YtMusic { dst, .. } => dst,
            Self::Spotify { dst, .. } => dst,
            Self::Tidal { dst, .. } => dst,
        }
    }
}
