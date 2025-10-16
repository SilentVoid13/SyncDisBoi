use std::path::PathBuf;

use clap::{Parser, Subcommand, ValueEnum};
use sync_dis_boi::ConfigArgs;
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

const TIDAL_DEFAULT_CLIENT_ID: &str =
    "\x66\x58\x32\x4a\x78\x64\x6d\x6e\x74\x5a\x57\x4b\x30\x69\x78\x54";
const TIDAL_DEFAULT_CLIENT_SECRET: &str = "\x4d\x55\x35\x75\x4f\x55\x46\x6d\x52\x45\x46\x71\x65\x48\x4a\x6e\x53\x6b\x5a\x4b\x59\x6b\x74\x4f\x56\x30\x78\x6c\x51\x58\x6c\x4c\x52\x31\x5a\x48\x62\x55\x6c\x4f\x64\x56\x68\x51\x55\x45\x78\x49\x56\x6c\x68\x42\x64\x6e\x68\x42\x5a\x7a\x30\x3d";

#[derive(Subcommand, Clone, Debug)]
#[command(subcommand_value_name = "SRC_PLATFORM")]
pub enum MusicPlatformSrc {
    YtMusic {
        /// The path to the headers JSON file
        #[arg(long)]
        headers: Option<PathBuf>,
        /// The client ID for the Youtube API application
        #[arg(
            long,
            env = "YTMUSIC_CLIENT_ID",
            conflicts_with = "headers",
            requires = "client_secret"
        )]
        client_id: Option<String>,
        /// The client secret for the Youtube API application
        #[arg(long, env = "YTMUSIC_CLIENT_SECRET", conflicts_with = "headers")]
        client_secret: Option<String>,
        /// Clear the cached ytmusic_oauth.json file
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
        /// Clear the cached spotify_oauth.json file
        #[arg(long)]
        clear_cache: bool,
        /// The destination music platform
        #[command(subcommand)]
        dst: MusicPlatformDst,
    },
    Tidal {
        /// The client ID for the Tidal API application
        #[arg(long, env = "TIDAL_CLIENT_ID", default_value = TIDAL_DEFAULT_CLIENT_ID)]
        client_id: String,
        /// The client secret for the Tidal API application
        #[arg(long, env = "TIDAL_CLIENT_SECRET", default_value = TIDAL_DEFAULT_CLIENT_SECRET)]
        client_secret: String,
        /// Clear the cached tidal_oauth.json file
        #[arg(long)]
        clear_cache: bool,
        /// The destination music platform
        #[command(subcommand)]
        dst: MusicPlatformDst,
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
        #[arg(
            long,
            env = "YTMUSIC_CLIENT_ID",
            conflicts_with = "headers",
            requires = "client_secret"
        )]
        client_id: Option<String>,
        /// The client secret for the Youtube API application
        #[arg(long, env = "YTMUSIC_CLIENT_SECRET", conflicts_with = "headers")]
        client_secret: Option<String>,
        /// Clear the cached ytmusic_oauth.json file
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
        /// Clear the cached spotify_oauth.json file
        #[arg(long)]
        clear_cache: bool,
    },
    Tidal {
        /// The client ID for the Tidal API application
        #[arg(long, env = "TIDAL_CLIENT_ID", default_value = TIDAL_DEFAULT_CLIENT_ID)]
        client_id: String,
        #[arg(long, env = "TIDAL_CLIENT_SECRET", default_value = TIDAL_DEFAULT_CLIENT_SECRET)]
        /// The client secret for the Tidal API application
        client_secret: String,
        /// Clear the cached tidal_oauth.json file
        #[arg(long)]
        clear_cache: bool,
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
