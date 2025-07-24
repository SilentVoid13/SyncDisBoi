pub mod export;
pub mod import;
pub mod music_api;
pub mod spotify;
pub mod sync;
pub mod tidal;
pub mod utils;
pub mod yt_music;

use clap::Parser;

// TODO: I don't really like depending on clap for the library,
// but it's the easiest way to share a configuration structure with the bin
#[derive(Parser, Debug, Clone)]
pub struct ConfigArgs {
    /// Enable debug mode to display and generate debug information during
    /// synchronization This is useful during development
    #[arg(long, default_value = "false")]
    pub debug: bool,

    /// Like all songs that will be synchronized on the destination platform
    #[arg(long, default_value = "false")]
    pub like_all: bool,

    /// Sync likes from the source platform to the destination platform.
    #[arg(long, default_value = "false")]
    pub sync_likes: bool,

    /// Allow the synchronization between platforms with different countries.
    /// Be aware that this can lead to invalid sync results, as some songs will
    /// have different ISRC codes.
    #[arg(long, default_value = "false")]
    pub diff_country: bool,

    /// Proxy to use for all requests in the format http://<ip>:<port>
    #[arg(long)]
    pub proxy: Option<String>,
}
