use clap::{Parser, Subcommand, ValueEnum};
use tracing::Level;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
pub struct RootArgs {
    /// The source music platform
    #[command(subcommand)]
    pub src: MusicPlatformSrc,

    /// Enable debug mode to display and generate debug information during synchronization
    /// This is useful during development
    #[arg(long, default_value = "false")]
    pub debug: bool,

    /// Proxy to use for all requests in the format http://<ip>:<port>
    #[arg(long)]
    pub proxy: Option<String>,

    /// Logging level
    #[arg(short, long, value_enum, default_value_t = LoggingLevel::Info)]
    pub logging: LoggingLevel,
}

#[derive(Subcommand, Clone, Debug)]
#[command(subcommand_value_name = "SRC_PLATFORM")]
pub enum MusicPlatformSrc {
    YtMusic {
        #[arg(long, env = "YTMUSIC_CLIENT_ID", default_value = "861556708454-d6dlm3lh05idd8npek18k6be8ba3oc68.apps.googleusercontent.com")]
        client_id: String,
        #[arg(long, env = "YTMUSIC_CLIENT_SECRET", default_value = "SboVhoG9s0rNafixCSGGKXAT")]
        client_secret: String,
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
        /// The destination music platform
        #[command(subcommand)]
        dst: MusicPlatformDst,
    },
}

// INFO: Hack to support command chaining with clap
// related issue: https://github.com/clap-rs/clap/issues/2222
#[derive(Subcommand, Clone, Debug)]
#[command(subcommand_value_name = "DST_PLATFORM")]
pub enum MusicPlatformDst{
    YtMusic {
        #[arg(long, env = "YTMUSIC_CLIENT_ID")]
        client_id: String,
        #[arg(long, env = "YTMUSIC_CLIENT_SECRET")]
        client_secret: String,
    },
    Spotify {
        /// The client ID for the Spotify API application
        #[arg(long, env = "SPOTIFY_CLIENT_ID")]
        client_id: String,
        /// The client secret for the Spotify API application
        #[arg(long, env = "SPOTIFY_CLIENT_SECRET")]
        client_secret: String,
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
