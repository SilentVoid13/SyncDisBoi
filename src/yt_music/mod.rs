pub mod model;
mod response;

use crate::music_api::{MusicApi, Playlist, Playlists, Song, Songs, PLAYLIST_DESC};

use anyhow::{Context, Result};
use async_trait::async_trait;
use reqwest::header::HeaderMap;
use serde::de::DeserializeOwned;
use serde_json::json;
use sha1::{Digest, Sha1};
use std::fmt::Write;
use std::{
    path::PathBuf,
    time::{SystemTime, UNIX_EPOCH},
};

use self::model::{YtMusicContinuationResponse, YtMusicResponse};

pub struct YtMusicApi {
    client: reqwest::Client,
    context: serde_json::Value,
}

impl YtMusicApi {
    const BASE_API: &'static str = "https://music.youtube.com/youtubei/v1/";
    const BASE_PARAMS: &'static str =
        "?alt=json&prettyPrint=false&key=AIzaSyC9XL3ZjWddXya6X74dJoCTL-WEYFDNX30";

    pub fn new(cookies: &PathBuf, secret: &str) -> Result<Self> {
        let origin = "https://music.youtube.com";
        let cookies = std::fs::read_to_string(cookies)?;

        // TODO: find a way to create constant dyn value without putting it into the struct
        let context: serde_json::Value = json!({
            "client": {
                "clientName": "WEB_REMIX",
                "clientVersion": "0.1",
                "hl": "en"
            },
            "user": {}
        });

        let mut headers = HeaderMap::new();
        headers.insert("accept", "*/*".parse()?);
        headers.insert("content-type", "application/json; charset=UTF-8".parse()?);
        headers.insert("authorization", secret.parse()?);
        headers.insert("cookie", cookies.parse()?);
        headers.insert("origin", origin.parse()?);
        headers.insert("user-agent",  "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/104.0.5112.102 Safari/537.36".parse()?);
        headers.insert("x-goog-authuser", "0".parse()?);
        headers.insert("x-goog-pageid", "100654875389698742898".parse()?);
        headers.insert(
            "x-goog-visitor-id",
            "CgtzUXZMenhNbFR4MCiI0rmYBg%3D%3D".parse()?,
        );

        let client = reqwest::ClientBuilder::new()
            .cookie_store(true)
            .default_headers(headers)
            .build()
            .unwrap();

        Ok(YtMusicApi { client, context })
    }

    pub fn get_authorization_hash(&self, sapisid: &str, origin: &str) -> String {
        let unix_timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        let input = format!("{} {} {}", unix_timestamp, sapisid, origin);

        let mut hasher = Sha1::new();
        hasher.update(input);
        let result = hasher.finalize();
        let hash = hex::encode(&result);

        let authorization = format!("SAPISIDHASH {}_{}", unix_timestamp, hash);
        return authorization;
    }

    fn build_endpoint(&self, path: &str, ctoken: Option<&str>) -> String {
        let mut endpoint = format!(
            "{}{}{}",
            YtMusicApi::BASE_API,
            path,
            YtMusicApi::BASE_PARAMS,
        );
        if let Some(c) = ctoken {
            std::write!(&mut endpoint, "&ctoken={c}&continuation={c}", c = c).unwrap();
        }
        endpoint
    }

    fn build_body(&self, value: &serde_json::Value) -> serde_json::Value {
        // TODO: find out how to merge
        /*
        let body = json!({
            ...value,
            "context": self.context,
        });
        */
        return json!({});
    }

    async fn paginated_request(&self, path: &str, body: &serde_json::Value) -> Result<YtMusicResponse> {
        let mut response: YtMusicResponse = self.make_request(path, body, None).await?;
        let mut continuation = response.get_continuation();

        while let Some(cont) = continuation {
            let mut response2: YtMusicContinuationResponse = self
                .make_request(path, body, Some(&cont))
                .await?;
            response.merge(&mut response2);
            continuation = response2.get_continuation();
        }
        Ok(response)
    }

    async fn make_request<T>(&self, path: &str, body: &serde_json::Value, ctoken: Option<&str>) -> Result<T>
    where
        T: DeserializeOwned + std::fmt::Debug,
    {
        let endpoint = self.build_endpoint(path, ctoken);

        let res = self.client.post(endpoint).json(body).send().await?;
        // TODO: remove this
        let text = res.text().await?;
        std::fs::write("data.json", &text).unwrap();
        //let obj = res.json().await?;
        let obj = serde_json::from_str(&text)?;
        Ok(obj)
    }
}

#[async_trait]
impl MusicApi for YtMusicApi {
    async fn create_playlist(&self, name: &str, public: bool) -> Result<Playlist> {
        let privacy_status = match public {
            true => "PUBLIC",
            false => "PRIVATE"
        };
        let body = json!({
            "title": name,
            "description": PLAYLIST_DESC,
            "privacyStatus": privacy_status,
        });
        let res = self.make_request("playlist/create", body, ctoken)
    }

    async fn get_playlists_info(&self) -> Result<Vec<Playlist>> {
        let browse_id = "FEmusic_liked_playlists";
        let body = self.build_body(browse_id);
        // TODO: Find a way to impl Deserialize for Playlists to avoid the .try_into
        let response = self.paginated_request("browse", &body).await?;
        let playlists: Playlists = response.try_into()?;
        Ok(playlists.0)
    }

    async fn get_playlist_songs(&self, id: &str) -> Result<Vec<Song>> {
        let browse_id = if id.starts_with("VL") {
            id.to_string()
        } else {
            format!("VL{}", id)
        };
        let body = self.build_body(&browse_id);
        // TODO: Find a way to impl Deserialize for Songs to avoid the .try_into
        let response = self.paginated_request("browse", &body).await?;
        let songs: Songs = response.try_into()?;
        Ok(songs.0)
    }
}
