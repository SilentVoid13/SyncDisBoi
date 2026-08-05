pub mod export;
pub mod import;
pub mod merge;
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

    /// Maximum number of song searches to run concurrently against the
    /// destination platform. Higher values speed up synchronization but
    /// increase the chance of hitting the destination API's rate limits.
    #[arg(long, default_value = "8")]
    pub search_concurrency: usize,

    /// Match singles (a track whose album name equals its own name, as
    /// Spotify often releases them) against a same-named track filed under
    /// a full album on the other platform, skipping the album-name check
    /// for that comparison. Without this, a single only matches another
    /// single, so a track released as a single on one platform but only
    /// available on its full album on the other will be reported as
    /// missing even when the recording itself is present.
    #[arg(long, default_value = "false")]
    pub map_singles: bool,

    /// Strip remix/mix/version qualifiers and feat. credits from track
    /// titles before comparing/searching (default: on, matching today's
    /// behavior). Disabling this keeps the raw title available alongside
    /// the stripped one, using both to avoid conflating a track with its
    /// remixes when they collapse to the same stripped name -- at the cost
    /// of being pickier about cross-platform title-formatting differences
    /// (e.g. feat.-credit lists) that the stripped comparison used to paper
    /// over.
    #[arg(long, default_value_t = true, action = clap::ArgAction::Set)]
    pub strip_qualifiers: bool,
}
