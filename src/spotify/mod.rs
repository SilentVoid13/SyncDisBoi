mod model;
mod response;

use crate::music_api::MusicApi;
use crate::music_api::Playlist;
use crate::music_api::Playlists;
use crate::music_api::Song;
use crate::music_api::Songs;
use crate::music_api::PLAYLIST_DESC;

use anyhow::{anyhow, Context, Result};
use async_trait::async_trait;
use reqwest::header::HeaderMap;
use serde::de::DeserializeOwned;
use serde_json::json;
use std::collections::HashMap;
use tokio::net::TcpListener;

use self::model::SpotifyPageResponse;
use self::model::SpotifyPlaylistResponse;
use self::model::SpotifySongItemResponse;
use self::model::SpotifyToken;

pub struct SpotifyApi {
    client: reqwest::Client,
}

impl SpotifyApi {
    const BASE_API: &'static str = "https://api.spotify.com/v1";
    const REDIRECT_URI_HOST: &'static str = "localhost:8888";
    const REDIRECT_URI_URL: &'static str = "http://localhost:8888/callback";
    const SCOPES: &'static [&'static str] = &[
        "user-read-email",
        "user-read-private",
        "playlist-read-collaborative",
        "playlist-modify-public",
        "playlist-read-private",
        "playlist-modify-private",
    ];

    pub async fn new(client_id: &str, client_secret: &str) -> Result<Self> {
        let auth_url = SpotifyApi::build_authorization_url(client_id)?;
        let auth_code = SpotifyApi::listen_for_code(&auth_url).await?;

        let mut params = HashMap::new();
        params.insert("grant_type", "authorization_code");
        params.insert("code", &auth_code);
        params.insert("redirect_uri", SpotifyApi::REDIRECT_URI_URL);

        let client = reqwest::Client::new();
        let res = client
            .post("https://accounts.spotify.com/api/token")
            .basic_auth(client_id, Some(client_secret))
            .form(&params)
            .send()
            .await?;
        let token: SpotifyToken = res.json().await?;

        let bearer = format!("Bearer {}", token.access_token);
        let mut headers = HeaderMap::new();
        headers.insert("authorization", bearer.parse()?);
        headers.insert("content-type", "application/json".parse()?);

        let client = reqwest::ClientBuilder::new()
            .default_headers(headers)
            .cookie_store(true)
            .build()?;

        Ok(SpotifyApi { client })
    }

    fn build_authorization_url(client_id: &str) -> Result<String> {
        let mut params = HashMap::new();
        params.insert("response_type", "code");
        let scopes = SpotifyApi::SCOPES.iter().as_slice().join(" ").to_string();
        params.insert("scope", &scopes);
        params.insert("client_id", client_id);
        params.insert("redirect_uri", SpotifyApi::REDIRECT_URI_URL);

        Ok(
            reqwest::Url::parse_with_params("https://accounts.spotify.com/authorize", params)?
                .to_string(),
        )
    }

    async fn listen_for_code(auth_url: &str) -> Result<String> {
        let listener = TcpListener::bind(SpotifyApi::REDIRECT_URI_HOST).await?;
        webbrowser::open(auth_url)?;

        let (socket, _) = listener.accept().await?;

        socket.readable().await?;
        let mut buffer = [0; 1024];
        let _ = socket.try_read(&mut buffer);

        let data = String::from_utf8(buffer.to_vec())?;
        let splits: Vec<&str> = data.split_whitespace().collect();
        if splits.len() <= 1 {
            return Err(anyhow!("Invalid spotify server callback"));
        }
        let url = format!("{}{}", SpotifyApi::REDIRECT_URI_HOST, splits[1]);
        let auth_code = reqwest::Url::parse(&url)?
            .query_pairs()
            .find(|pair| pair.0 == "code")
            .context("Spotify server returned no autorization code")?
            .1
            .to_string();

        Ok(auth_code)
    }

    fn build_endpoint(&self, path: &str, limit: u32, offset: u32) -> String {
        format!(
            "{}{}?limit={}&offset={}",
            SpotifyApi::BASE_API,
            path,
            limit,
            offset
        )
    }

    async fn paginated_request<T>(&self, path: &str, limit: u32) -> Result<SpotifyPageResponse<T>>
    where
        T: DeserializeOwned,
    {
        let mut offset = 0;
        let mut response: SpotifyPageResponse<T> =
            self.make_request(path, limit, offset, None).await?;
        let mut total = response.total;

        while offset < total {
            offset += limit;
            let mut response2: SpotifyPageResponse<T> =
                self.make_request(path, limit, offset, None).await?;
            total = response2.total;
            response.merge(&mut response2);
        }
        Ok(response)
    }

    async fn make_request<T>(
        &self,
        path: &str,
        limit: u32,
        offset: u32,
        body: Option<serde_json::Value>,
    ) -> Result<T>
    where
        T: DeserializeOwned,
    {
        let endpoint = self.build_endpoint(path, limit, offset);
        let res = match body {
            Some(b) => self.client.post(endpoint).json(&b).send().await?,
            None => self.client.get(endpoint).send().await?,
        };
        // TODO: change this
        let text = res.text().await?;
        std::fs::write("data.json", &text).unwrap();
        let obj = serde_json::from_str(&text)?;
        //let json = res.json().await?;
        Ok(obj)
    }
}

#[async_trait]
impl MusicApi for SpotifyApi {
    async fn create_playlist(&self, name: &str, public: bool) -> Result<Playlist> {
        let path = "/me/playlists";
        let body = json!({
            "name": name,
            "public": public,
            "description": PLAYLIST_DESC,
        });
        let res: SpotifyPlaylistResponse = self.make_request(&path, 50, 0, Some(body)).await?;
        let playlist: Playlist = res.try_into()?;
        Ok(playlist)
    }

    async fn get_playlists_info(&self) -> Result<Vec<Playlist>> {
        let path = "/me/playlists";
        let res: SpotifyPageResponse<SpotifyPlaylistResponse> =
            self.paginated_request(&path, 50).await?;
        let playlists: Playlists = res.try_into()?;
        Ok(playlists.0)
    }

    async fn get_playlist_songs(&self, id: &str) -> Result<Vec<Song>> {
        let path = format!("/playlists/{}/tracks", id);
        let res: SpotifyPageResponse<SpotifySongItemResponse> =
            self.paginated_request(&path, 50).await?;
        let songs: Songs = res.try_into()?;
        Ok(songs.0)
    }

    async fn add_songs_to_playlist<'a>(
        &self,
        playlist: &mut Playlist,
        songs_ids: &[&'a str],
    ) -> Result<()> {
        let uris: Vec<String> = songs_ids
            .iter()
            .map(|id| format!("spotify:track:{}", id))
            .collect();
        let path = format!("/playlists/{}/tracks", playlist.id);
        let body = json!({
            "uris": uris,
        });
        println!("BODY: {:?}", body);
        self.make_request(&path, 50, 0, Some(body)).await?;
        // TODO: add Song object to Playlist
        Ok(())
    }

    async fn remove_songs_from_playlist<'a>(
        &self,
        playlist: &mut Playlist,
        songs_ids: &[&'a str],
    ) -> Result<()> {
        todo!();
    }
}
