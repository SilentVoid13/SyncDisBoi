use clap::Parser;
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[clap(author, version, about, long_about = None)]
pub struct Args {
    #[clap(long, env, required(true))]
    pub yt_music_secret: String,
    #[clap(long, env, required(true))]
    pub yt_music_cookies: PathBuf,
    #[clap(long, env, required(true))]
    pub spotify_client_id: String,
    #[clap(long, env, required(true))]
    pub spotify_client_secret: String,
}
