use clap::Parser;
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
pub struct Args {
    /// The secret for the YouTube Music API
    #[arg(long, env, required = true)]
    pub yt_music_secret: String,
    /// The path to the cookies file for the YouTube Music API
    #[arg(long, env, required = true)]
    pub yt_music_cookies: PathBuf,
    /// The client ID for the Spotify API application
    #[arg(long, env, required = true)]
    pub spotify_client_id: String,
    /// The client secret for the Spotify API application
    #[arg(long, env, required = true)]
    pub spotify_client_secret: String,
}
