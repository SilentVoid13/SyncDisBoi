mod response;
pub mod model;

use crate::music_api::{MusicApi, Playlists, Playlist, Song, Songs};

use async_trait::async_trait;
use reqwest::header::HeaderMap;
use sha1::{Sha1, Digest};
use std::time::{SystemTime, UNIX_EPOCH};
use anyhow::Result;
use serde_json::json;
use futures::future::try_join_all;

pub struct YtMusicApi {
    client: reqwest::Client,
    context: serde_json::Value,
}

impl YtMusicApi {
    const BASE_API: &'static str = "https://music.youtube.com/youtubei/v1/";
    const BASE_PARAMS: &'static str = "?alt=json&prettyPrint=false&key=AIzaSyC9XL3ZjWddXya6X74dJoCTL-WEYFDNX30";

    pub fn new() -> Self {
        let origin = "https://music.youtube.com";
        // TODO: remove this
        let cookies = include_str!("../../cookies.txt");
        // TODO: remove this
        let authorization = include_str!("../../auth.txt");
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
        headers.insert("accept", "*/*".parse().unwrap());
        headers.insert("content-type",  "application/json; charset=UTF-8".parse().unwrap());
        headers.insert("authorization", authorization.parse().unwrap());
        headers.insert("cookie", cookies.parse().unwrap());
        headers.insert("origin",  origin.parse().unwrap());
        headers.insert("user-agent",  "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/104.0.5112.102 Safari/537.36".parse().unwrap());
        headers.insert("x-goog-authuser", "0".parse().unwrap());
        headers.insert("x-goog-pageid", "100654875389698742898".parse().unwrap());
        headers.insert("x-goog-visitor-id", "CgtzUXZMenhNbFR4MCiI0rmYBg%3D%3D".parse().unwrap());

        let client = reqwest::ClientBuilder::new()
            .cookie_store(true)
            .default_headers(headers)
            .build().unwrap();

        YtMusicApi {client, context}
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

    fn build_endpoint(&self, path: &str) -> String {
        format!("{}{}{}", YtMusicApi::BASE_API, path, YtMusicApi::BASE_PARAMS)
    }

    fn build_body(&self, browse_id: &str) -> serde_json::Value {
        let body = json!({
            "browseId": browse_id,
            "context": self.context,
        });
        return body;
    }

    async fn make_request(&self, path: &str, browse_id: &str) -> Result<serde_json::Value> {
        let endpoint = self.build_endpoint(path);
        let body_json = self.build_body(browse_id);

        let res = self.client.post(endpoint)
            .json(&body_json)
            .send().await?;
        let text = res.text().await?;
        let json = serde_json::from_str(&text)?;
        Ok(json)
    }
}

#[async_trait]
impl MusicApi for YtMusicApi {
    async fn create_playlist(&self) {
        todo!();
    }

    async fn get_playlists_info(&self) -> Result<Vec<Playlist>> {
        let browse_id = "FEmusic_liked_playlists";
        let response = self.make_request("browse", browse_id).await?;
        let playlists: Playlists = response.try_into()?;
        Ok(playlists.0)
    }

    async fn get_playlist_songs(&self, id: &str) -> Result<Vec<Song>> {
        let browse_id = if id.starts_with("VL") {
            id.to_string()
        } else {
            format!("VL{}", id)
        };
        let response = self.make_request("browse", &browse_id).await?;
        let songs: Songs = response.try_into()?;
        Ok(songs.0)
    }

    async fn get_playlists_full(&self) -> Result<Vec<Playlist>> {
        let mut playlists = self.get_playlists_info().await?;

        let mut requests = vec![];
        for playlist in playlists.iter_mut() {
            requests.push(self.get_playlist_songs(&playlist.id));
        }
        let results = try_join_all(requests).await?;
        for (i, songs) in results.into_iter().enumerate() {
            playlists[i].songs = Some(Songs(songs));
        }

        Ok(playlists)
    }
}
