use async_trait::async_trait;
use color_eyre::Result;
use model::{DeezerJwtResponse, DeezerUserResponse};
use reqwest::header::HeaderMap;
use serde_json::json;
use tracing::info;

use crate::music_api::{MusicApi, MusicApiType, Playlist, Song};
use crate::ConfigArgs;

mod model;
mod response;

pub struct DeezerApi {
    client: reqwest::Client,
    config: ConfigArgs,
    user_id: String,
    country_code: String,
}

impl DeezerApi {
    pub const BASE_URL: &str = "https://api.deezer.com";
    pub const JWT_URL: &str = "https://auth.deezer.com/login/arl";

    pub async fn new(arl: &str, clear_cache: bool, config: ConfigArgs) -> Result<Self> {
        info!("requesting new token");
        let token = Self::request_token(arl).await?;
        dbg!(&token);

        let mut headers = HeaderMap::new();
        headers.insert("cookie", format!("arl={}", arl).parse()?);
        headers.insert("Authorization", format!("Bearer {}", token).parse()?);

        dbg!(&headers);

        let mut client = reqwest::Client::builder()
            .cookie_store(true)
            .default_headers(headers);
        if let Some(proxy) = &config.proxy {
            client = client
                .proxy(reqwest::Proxy::all(proxy)?)
                .danger_accept_invalid_certs(true)
        }
        let client = client.build()?;

        let url = format!("{}/user/me", Self::BASE_URL);
        let res = client.get(&url).send().await?;
        let res = res.error_for_status()?;
        let t = dbg!(&res.text().await?);
        todo!();
        let me_res: DeezerUserResponse = res.json().await?;
        dbg!(&me_res);

        Ok(Self {
            client,
            config,
            user_id: me_res.id.to_string(),
            country_code: me_res.country,
        })
    }

    async fn request_token(arl: &str) -> Result<String> {
        let client = reqwest::Client::new();
        let mut headers = HeaderMap::new();
        headers.insert("user-agent", "curl/8.9.1".parse()?);
        headers.insert("accept", "*/*".parse()?);
        headers.insert("content-length", "0".parse()?);
        headers.insert("cookie", format!("arl={}", arl).parse()?);
        let params = json!({
            "jo": "p",
            "rto": "c",
            "i": "c",
        });
        let res = client
            .post(Self::JWT_URL)
            .query(&params)
            .headers(headers)
            .send()
            .await?;
        let res = res.error_for_status()?;
        let res: DeezerJwtResponse = res.json().await?;

        /*
        let url = "http://www.deezer.com/ajax/gw-light.php";
        let mut headers = HeaderMap::new();
        headers.insert("cookie", format!("arl={}", arl).parse()?);
        let params = json!({
          "api_version": "1.0",
          "api_token": "null",
          "input": "3",
          "method": "deezer.getUserData"
        });
        let res = client
            .post(url)
            .headers(headers)
            .json(&params)
            .send()
            .await?;
        let res = res.error_for_status()?;
        let t = res.text().await?;
        dbg!(&t);
        todo!();
        */

        Ok(res.jwt)
    }
}

#[async_trait]
impl MusicApi for DeezerApi {
    fn api_type(&self) -> MusicApiType {
        MusicApiType::Deezer
    }

    fn country_code(&self) -> &str {
        &self.country_code
    }

    async fn create_playlist(&self, name: &str, public: bool) -> Result<Playlist> {
        todo!()
    }
    async fn get_playlists_info(&self) -> Result<Vec<Playlist>> {
        todo!()
    }
    async fn get_playlist_songs(&self, id: &str) -> Result<Vec<Song>> {
        todo!()
    }

    async fn add_songs_to_playlist(&self, playlist: &mut Playlist, songs: &[Song]) -> Result<()> {
        todo!()
    }
    async fn remove_songs_from_playlist(
        &self,
        playlist: &mut Playlist,
        songs_ids: &[Song],
    ) -> Result<()> {
        todo!()
    }
    async fn delete_playlist(&self, playlist: Playlist) -> Result<()> {
        todo!()
    }

    async fn search_song(&self, song: &Song) -> Result<Option<Song>> {
        todo!()
    }

    async fn add_like(&self, songs: &[Song]) -> Result<()> {
        todo!()
    }
    async fn get_likes(&self) -> Result<Vec<Song>> {
        todo!()
    }
}
