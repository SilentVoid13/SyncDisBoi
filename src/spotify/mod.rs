mod model;
mod response;

use crate::music_api::MusicApi;
use crate::music_api::Playlist;
use crate::music_api::Playlists;
use crate::music_api::Song;
use crate::music_api::Songs;
use crate::music_api::PLAYLIST_DESC;
use crate::spotify::model::SpotifySearchResponse;

use anyhow::{anyhow, Context, Result};
use async_recursion::async_recursion;
use async_trait::async_trait;
use reqwest::header::HeaderMap;
use reqwest::Response;
use reqwest::StatusCode;
use serde::de::DeserializeOwned;
use serde_json::json;
use tracing::debug;
use tracing::info;
use tracing::warn;
use std::collections::HashMap;
use std::time::Duration;
use tokio::net::TcpListener;

use self::model::SpotifyPageResponse;
use self::model::SpotifyPlaylistResponse;
use self::model::SpotifySnapshotResponse;
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

    fn build_endpoint(&self, path: &str) -> String {
        format!("{}{}", SpotifyApi::BASE_API, path,)
    }

    async fn paginated_request<T>(
        &self,
        path: &str,
        get_params: Option<&[(&str, &str)]>,
        limit: u32,
    ) -> Result<SpotifyPageResponse<T>>
    where
        T: DeserializeOwned,
    {
        let mut offset = 0;
        let mut response: SpotifyPageResponse<T> = self
            .make_request(path, get_params, None, limit, offset)
            .await?;
        let mut total = response.total;

        while offset < total {
            offset += limit;
            let mut response2: SpotifyPageResponse<T> = self
                .make_request(path, get_params, None, limit, offset)
                .await?;
            total = response2.total;
            response.merge(&mut response2);
        }
        Ok(response)
    }

    async fn api_rate_wait(&self, res: &Response) -> Result<()> {
        let headers = res.headers();
        let sleep_time = headers
            .get("Retry-After")
            .context("No Retry-After header")?
            .to_str()
            .context("Invalid Retry-After header")?
            .parse::<u64>()
            .context("Invalid Retry-After header")?;
        warn!("API rate limit reached, sleeping for {} seconds", sleep_time);
        tokio::time::sleep(Duration::from_secs(sleep_time)).await;
        Ok(())
    }

    #[async_recursion]
    async fn make_request<T>(
        &self,
        path: &str,
        get_params: Option<&'async_recursion [(&'async_recursion str, &'async_recursion str)]>,
        body: Option<serde_json::Value>,
        limit: u32,
        offset: u32,
    ) -> Result<T>
    where
        T: DeserializeOwned,
    {
        let endpoint = self.build_endpoint(path);
        let mut request = match body.as_ref() {
            Some(b) => self.client.post(endpoint).json(b),
            None => self.client.get(endpoint),
        };
        request = request.query(&[("limit", limit), ("offset", offset)]);
        if let Some(p) = get_params {
            request = request.query(p);
        }
        let res = request.send().await?;
        if res.status() == StatusCode::TOO_MANY_REQUESTS {
            self.api_rate_wait(&res).await?;
            // Retry request
            return self
                .make_request(path, get_params, body, limit, offset)
                .await;
        }
        if res.status() != StatusCode::OK && res.status() != StatusCode::CREATED {
            return Err(anyhow!("Invalid response: {}", res.text().await?));
        }
        // TODO: change this
        let text = res.text().await?;
        std::fs::write("data.json", &text).unwrap();
        let obj = serde_json::from_str(&text)?;
        //let json = res.json().await?;
        Ok(obj)
    }

    #[async_recursion]
    async fn delete_request<T>(&self, path: &str, body: serde_json::Value) -> Result<()>
    where
        T: DeserializeOwned,
    {
        let endpoint = self.build_endpoint(path);
        let res = self.client.delete(endpoint).json(&body).send().await?;
        if res.status() == StatusCode::TOO_MANY_REQUESTS {
            self.api_rate_wait(&res).await?;
            // Retry request
            return self.delete_request::<T>(path, body).await;
        }
        if res.status() != StatusCode::OK && res.status() != StatusCode::CREATED {
            return Err(anyhow!("Invalid response: {}", res.text().await?));
        }
        let text = res.text().await?;
        if text.is_empty() {
            return Ok(());
        }
        let _: T = serde_json::from_str(&text)?;
        Ok(())
    }
}

pub fn push_query(queries: &mut Vec<String>, query: String, max_len: usize) {
    if query.len() > max_len {
        debug!("Query too long: {}, skipping", query);
        return;
    }
    queries.push(query);
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
        let res: SpotifyPlaylistResponse =
            self.make_request(&path, None, Some(body), 50, 0).await?;
        let playlist: Playlist = res.try_into()?;
        Ok(playlist)
    }

    async fn get_playlists_info(&self) -> Result<Vec<Playlist>> {
        let path = "/me/playlists";
        let res: SpotifyPageResponse<SpotifyPlaylistResponse> =
            self.paginated_request(&path, None, 50).await?;
        let playlists: Playlists = res.try_into()?;
        Ok(playlists.0)
    }

    async fn get_playlist_songs(&self, id: &str) -> Result<Vec<Song>> {
        let path = format!("/playlists/{}/tracks", id);
        let res: SpotifyPageResponse<SpotifySongItemResponse> =
            self.paginated_request(&path, None, 50).await?;
        let songs: Songs = res.try_into()?;
        Ok(songs.0)
    }

    async fn add_songs_to_playlist(&self, playlist_id: &str, songs_ids: &[String]) -> Result<()> {
        // TODO: add Song object to Playlist

        let uris: Vec<String> = songs_ids
            .iter()
            .map(|id| format!("spotify:track:{}", id))
            .collect();
        let path = format!("/playlists/{}/tracks", playlist_id);
        for u in uris.as_slice().chunks(100) {
            let body = json!({
                "uris": u,
            });
            let _: SpotifySnapshotResponse =
                self.make_request(&path, None, Some(body), 50, 0).await?;
        }
        Ok(())
    }

    async fn remove_songs_from_playlist(
        &self,
        playlist: &mut Playlist,
        songs_ids: &[String],
    ) -> Result<()> {
        // TODO: remove Song object from Playlist

        let uris: Vec<serde_json::Value> = songs_ids
            .iter()
            .map(|id| {
                let uri = format!("spotify:track:{}", id);
                json!({ "uri": uri })
            })
            .collect();
        let path = format!("/playlists/{}/tracks", playlist.id);
        let body = json!({
            "tracks": uris,
        });
        self.delete_request::<SpotifySnapshotResponse>(&path, body)
            .await?;
        Ok(())
    }

    async fn delete_playlist(&self, playlist_id: &str) -> Result<()> {
        let path = format!("/playlists/{}/followers", playlist_id);
        let body = json!({
            "playlist_id": playlist_id,
        });
        self.delete_request::<()>(&path, body).await?;
        Ok(())
    }

    async fn search_song(&self, song: &Song) -> Result<Option<Song>> {
        // Spotify doesn't support quotes in search

        let path = "/search";
        let max_len = 100;

        // TODO: It looks like single quotes have better results compared to double quotes, not
        // sure why

        let mut track_query = format!("track:\"{}\"", song.clean_name());

        // TODO: fix this
        if track_query.len() > max_len {
            debug!("Can't add track to query, skipping: {}", track_query);
            // Not the best solution, but it's worth a try
            track_query = track_query[..max_len].to_string();
        }

        let artist_queries: Vec<String> = song
            .artists
            .iter()
            .map(|a| format!("artist:\"{}\"", a.clean_name()))
            .collect();

        let mut album_query = None;
        if let Some(album) = &song.album {
            album_query = Some(format!("album:\"{}\"", album.clean_name()));
        }

        // Track + Artist + Album -> Track + Artist -> Track + Album
        let mut queries = vec![];
        if let Some(album_query) = album_query.as_ref() {
            let tr_al_query = format!("{} {}", track_query, album_query);
            push_query(&mut queries, tr_al_query, max_len);
        }
        for artist_query in artist_queries.iter().rev() {
            // It looks like spotify doesn't support multiple artists in search
            let tr_ar_query = format!("{} {}", track_query, artist_query);
            push_query(&mut queries, tr_ar_query, max_len);
        }
        if let Some(album_query) = album_query.as_ref() {
            for artist_query in artist_queries.iter().rev() {
                let tr_ar_al_query = format!("{} {} {}", track_query, artist_query, album_query);
                push_query(&mut queries, tr_ar_al_query, max_len);
            }
        }

        // TODO: remove this
        let queries_b = queries.clone();
        while let Some(query) = queries.pop() {
            let get_params = [("type", "track"), ("q", &query)];
            let res: SpotifySearchResponse = self
                .make_request(&path, Some(&get_params), None, 50, 0)
                .await?;
            let mut res_songs: Songs = res.try_into()?;
            if !res_songs.0.is_empty() {
                let res_song = res_songs.0.remove(0);
                if song.compare(&res_song) {
                    return Ok(Some(res_song));
                }
            }
        }
        debug!("Queries failed: {:?}", queries_b);
        return Ok(None);
    }
}

#[cfg(test)]
mod tests {
    use std::{env, path::PathBuf};

    use crate::yt_music::YtMusicApi;

    use super::*;

    #[tokio::test]
    async fn test_spotify_search_from_ytmusic() {
        let ytmusic_cookies = env::var("YT_MUSIC_COOKIES").unwrap();
        let ytmusic_cookies = PathBuf::from(ytmusic_cookies);
        let ytmusic_secret = std::env::var("YT_MUSIC_SECRET").unwrap();
        let ytmusic = YtMusicApi::new(&ytmusic_cookies, &ytmusic_secret).unwrap();

        let playlists = ytmusic.get_playlists_info().await.unwrap();
        let test_spotify = playlists.iter().find(|p| p.name == "TestSpotify").unwrap();
        let songs = ytmusic.get_playlist_songs(&test_spotify.id).await.unwrap();

        return;

        let spotify_client_id = env::var("SPOTIFY_CLIENT_ID").unwrap();
        let spotify_secret = env::var("SPOTIFY_CLIENT_SECRET").unwrap();
        let spotify = SpotifyApi::new(&spotify_client_id, &spotify_secret)
            .await
            .unwrap();

        let songs = spotify.search_songs(&songs).await.unwrap();
        let correct_ids = [
            "2x1GoZKREbFkQJ8FUaz3Lc",
            "088mi9DvOJCOKjMgqXJ03C",
            "5dayqPrW7a4b2Skq3EcxWK",
            "1vU4X8ffq8oNcvvqkgTEXm",
            "1YqUm734e5Yv5BJEDhLYxK",
            "0qG1teoBvooRo7Z5Z8edCk",
            "32dnKMni3I3gwUbWp4mi45",
        ];
        for song in songs {
            let song = song.unwrap();
            println!("Testing song: {}, id {}", song.name, song.id);
            assert!(correct_ids.contains(&song.id.as_str()));
        }
    }
}
