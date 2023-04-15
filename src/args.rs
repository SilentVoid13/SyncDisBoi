use clap::{Parser, Args, ValueEnum};
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

    /// Generate stats about the synchronization in a `stats` folder
    ///
    ///
    /// This includes: 
    /// - conversion rate for each playlist
    /// - list of songs that couldn't be synchronized
    /// - list of songs with no album metadata
    #[arg(long)]
    pub stats: bool,

    /// Verbose output
    #[arg(short, long)]
    pub verbose: bool,
}

#[derive(ValueEnum, Clone, Debug)]
pub enum MusicPlatform {
    YtMusic,
    Spotify,
}

#[derive(Args, Clone, Debug)]
#[group()]
pub struct YtMusicArgs {
    /// The secret for the YouTube Music API
    #[arg(long, env = "YTMUSIC_SECRET")]
    pub secret: Option<String>,
    /// The path to the cookies file for the YouTube Music API
    #[arg(long, env = "YTMUSIC_COOKIES")]
    pub cookies: Option<PathBuf>,
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
