use clap::{Args, Parser, ValueEnum};
use tracing::Level;

use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
pub struct RootArgs {
    /// The source music platform
    #[arg(short, long, required = true, env = "SRC_PLATFORM")]
    pub src: MusicPlatform,

    /// The destination music platform
    #[arg(short, long, required = true, env = "DST_PLATFORM")]
    pub dst: MusicPlatform,

    #[command(flatten)]
    pub spotify: SpotifyArgs,

    #[command(flatten)]
    pub ytmusic: YtMusicArgs,

    /// Enable debug mode to display and generate debug information during synchronization
    /// This is useful during development
    #[arg(long, default_value = "false")]
    pub debug: bool,

    /// Logging level
    #[arg(short, long, value_enum, default_value_t = LoggingLevel::Info)]
    pub logging: LoggingLevel,
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

#[derive(ValueEnum, Clone, Debug)]
pub enum MusicPlatform {
    YtMusic,
    Spotify,
}

#[derive(Args, Clone, Debug)]
#[group()]
pub struct YtMusicArgs {
    /// The file containing the headers for the YouTube Music API
    #[arg(long, env = "YTMUSIC_HEADERS")]
    pub headers: Option<PathBuf>,
}

#[derive(Args, Debug)]
#[group()]
pub struct SpotifyArgs {
    /// The client ID for the Spotify API application
    #[arg(long, env = "SPOTIFY_CLIENT_ID")]
    pub client_id: Option<String>,
    /// The client secret for the Spotify API application
    #[arg(long, env = "SPOTIFY_CLIENT_SECRET")]
    pub client_secret: Option<String>,
}
