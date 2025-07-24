use std::path::Path;

use color_eyre::eyre::Result;
use tracing::info;

use crate::ConfigArgs;
use crate::music_api::{DynMusicApi, Playlist};
use crate::sync::synchronize_playlists;

pub async fn import(src_json: &Path, dst_api: DynMusicApi, config: ConfigArgs) -> Result<()> {
    let src_playlists: Vec<Playlist> = serde_json::from_reader(std::fs::File::open(src_json)?)?;

    info!("importing playlists...");
    synchronize_playlists(src_playlists, &dst_api, &config).await?;
    info!(
        "successfully imported playlists to {:?}",
        dst_api.api_type()
    );

    Ok(())
}
