use std::path::PathBuf;

use clap::{Parser, Subcommand, ValueEnum};
use sync_dis_boi::{
    ConfigArgs, listenbrainz::ListenBrainzApi, spotify::SpotifyApi, tidal::TidalApi,
};
use tracing::Level;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
pub struct RootArgs {
    /// The source music platform
    #[command(subcommand)]
    pub src: MusicPlatformSrc,

    #[command(flatten)]
    pub config: ConfigArgs,

    /// Logging level
    #[arg(short, long, value_enum, default_value_t = LoggingLevel::Info)]
    pub logging: LoggingLevel,
}

#[derive(Subcommand, Clone, Debug)]
#[command(subcommand_value_name = "SRC_PLATFORM")]
pub enum MusicPlatformSrc {
    YtMusic {
        /// The path to the headers JSON file
        #[arg(long)]
        headers: Option<PathBuf>,
        /// The client ID for the Youtube API application
        #[arg(long, env = "YTMUSIC_CLIENT_ID", requires = "client_secret")]
        client_id: Option<String>,
        /// The client secret for the Youtube API application
        #[arg(long, env = "YTMUSIC_CLIENT_SECRET")]
        client_secret: Option<String>,
        /// Clear the cached `ytmusic_oauth.json` file
        #[arg(long, requires = "client_id", requires = "client_secret")]
        clear_cache: bool,
        /// The destination music platform
        #[command(subcommand)]
        dst: MusicPlatformDst,
    },
    Spotify {
        /// The client ID for the Spotify API application
        #[arg(long, env = "SPOTIFY_CLIENT_ID")]
        client_id: String,
        /// The client secret for the Spotify API application
        #[arg(long, env = "SPOTIFY_CLIENT_SECRET")]
        client_secret: String,
        /// Clear the cached `spotify_oauth.json` file
        #[arg(long)]
        clear_cache: bool,
        /// The redirect URI for the Spotify API application
        #[arg(long, default_value = SpotifyApi::REDIRECT_URI_URL)]
        redirect_uri: String,
        /// The destination music platform
        #[command(subcommand)]
        dst: MusicPlatformDst,
    },
    Tidal {
        /// The client ID for the Tidal API application
        #[arg(long, env = "TIDAL_CLIENT_ID", default_value = TidalApi::DEFAULT_CLIENT_ID)]
        client_id: String,
        /// The client secret for the Tidal API application
        #[arg(long, env = "TIDAL_CLIENT_SECRET", default_value = TidalApi::DEFAULT_CLIENT_SECRET)]
        client_secret: String,
        /// Clear the cached `tidal_oauth.json` file
        #[arg(long)]
        clear_cache: bool,
        /// The destination music platform
        #[command(subcommand)]
        dst: MusicPlatformDst,
    },
    #[allow(clippy::doc_markdown)]
    #[command(name = "listenbrainz", alias = "lb")]
    ListenBrainz {
        /// The user token from https://listenbrainz.org/settings/
        #[arg(long, env = "LISTENBRAINZ_TOKEN")]
        token: String,
        /// The base URL of the ListenBrainz API (for self-hosted instances)
        #[arg(long, env = "LISTENBRAINZ_API_URL", default_value = ListenBrainzApi::BASE_API)]
        api_url: String,
        /// The destination music platform
        #[command(subcommand)]
        dst: MusicPlatformDst,
    },
    /// Merge several canonical exports into a single platform-agnostic file.
    ///
    /// Inputs are processed in priority order (list ISRC-rich platforms such as
    /// Tidal/Spotify first); on a duplicate the earlier input's copy is kept.
    Merge {
        /// Canonical JSON exports to merge, in priority order (ISRC-rich first).
        /// Repeat the flag for each input.
        #[arg(short = 'i', long = "input", required = true)]
        inputs: Vec<PathBuf>,
        /// The path to write the merged canonical file to
        #[arg(short, long)]
        output: PathBuf,
        /// Minify the merged JSON file
        #[arg(long, default_value = "false")]
        minify: bool,
    },
}

// INFO: Hack to support command chaining with clap
// related issue: https://github.com/clap-rs/clap/issues/2222
#[derive(Subcommand, Clone, Debug)]
#[command(subcommand_value_name = "DST_PLATFORM")]
pub enum MusicPlatformDst {
    YtMusic {
        /// The path to the headers JSON file
        #[arg(long)]
        headers: Option<PathBuf>,
        /// The client ID for the Youtube API application
        #[arg(long, env = "YTMUSIC_CLIENT_ID", requires = "client_secret")]
        client_id: Option<String>,
        /// The client secret for the Youtube API application
        #[arg(long, env = "YTMUSIC_CLIENT_SECRET")]
        client_secret: Option<String>,
        /// Clear the cached `ytmusic_oauth.json` file
        #[arg(long, requires = "client_id", requires = "client_secret")]
        clear_cache: bool,
    },
    Spotify {
        /// The client ID for the Spotify API application
        #[arg(long, env = "SPOTIFY_CLIENT_ID")]
        client_id: String,
        /// The client secret for the Spotify API application
        #[arg(long, env = "SPOTIFY_CLIENT_SECRET")]
        client_secret: String,
        /// Clear the cached `spotify_oauth.json` file
        #[arg(long)]
        clear_cache: bool,
        /// The redirect URI for the Spotify API application
        #[arg(long, default_value = SpotifyApi::REDIRECT_URI_URL)]
        redirect_uri: String,
    },
    Tidal {
        /// The client ID for the Tidal API application
        #[arg(long, env = "TIDAL_CLIENT_ID", default_value = TidalApi::DEFAULT_CLIENT_ID)]
        client_id: String,
        #[arg(long, env = "TIDAL_CLIENT_SECRET", default_value = TidalApi::DEFAULT_CLIENT_SECRET)]
        /// The client secret for the Tidal API application
        client_secret: String,
        /// Clear the cached `tidal_oauth.json` file
        #[arg(long)]
        clear_cache: bool,
    },
    #[allow(clippy::doc_markdown)]
    #[command(name = "listenbrainz", alias = "lb")]
    ListenBrainz {
        /// The user token from https://listenbrainz.org/settings/
        #[arg(long, env = "LISTENBRAINZ_TOKEN")]
        token: String,
        /// The base URL of the ListenBrainz API (for self-hosted instances)
        #[arg(long, env = "LISTENBRAINZ_API_URL", default_value = ListenBrainzApi::BASE_API)]
        api_url: String,
    },
    Export {
        /// The path to the file to export the playlists to
        #[arg(short, long)]
        output: PathBuf,
        /// Minify the exported JSON file
        #[arg(long, default_value = "false")]
        minify: bool,
    },
    Import {
        /// The path to the file to import the playlists from
        #[arg(short, long)]
        input: PathBuf,
    },
}

#[derive(ValueEnum, Clone, Debug)]
pub enum LoggingLevel {
    /// Only log errors
    Error,
    /// Log errors and warnings
    Warn,
    /// Log errors, warnings and info
    Info,
    /// Log errors, warnings, info and debug (very verbose)
    Debug,
}

impl From<LoggingLevel> for Level {
    fn from(level: LoggingLevel) -> Self {
        match level {
            LoggingLevel::Warn => Level::WARN,
            LoggingLevel::Error => Level::ERROR,
            LoggingLevel::Info => Level::INFO,
            LoggingLevel::Debug => Level::DEBUG,
        }
    }
}
