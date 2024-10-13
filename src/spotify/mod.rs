mod model;
mod response;

use std::collections::HashMap;
use std::time::Duration;

use async_recursion::async_recursion;
use async_trait::async_trait;
use color_eyre::eyre::{eyre, Result};
use model::SpotifyUserResponse;
use reqwest::header::HeaderMap;
use reqwest::{Response, StatusCode};
use serde::de::DeserializeOwned;
use serde_json::json;
use tokio::net::TcpListener;
use tracing::{debug, info, warn};

use self::model::{
    SpotifyPageResponse, SpotifyPlaylistResponse, SpotifySnapshotResponse, SpotifySongItemResponse,
};
use crate::music_api::{MusicApi, MusicApiType, OAuthToken, Playlist, Playlists, Song, Songs, PLAYLIST_DESC};
use crate::spotify::model::SpotifySearchResponse;

pub struct SpotifyApi {
    client: reqwest::Client,
    debug: bool,
    country_code: String,
}

#[derive(Debug)]
enum HttpMethod<'a> {
    Get(&'a [(&'a str, &'a str)]),
    Post(&'a serde_json::Value),
    Delete(&'a serde_json::Value),
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

    pub async fn new(
        client_id: &str,
        client_secret: &str,
        debug: bool,
        proxy: Option<&str>,
    ) -> Result<Self> {
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
        let res = res.error_for_status()?;
        let token: OAuthToken = res.json().await?;

        let bearer = format!("Bearer {}", token.access_token);
        let mut headers = HeaderMap::new();
        headers.insert("authorization", bearer.parse()?);
        headers.insert("content-type", "application/json".parse()?);

        let mut client = reqwest::ClientBuilder::new()
            .default_headers(headers)
            .cookie_store(true);

        if let Some(proxy) = proxy {
            client = client
                .proxy(reqwest::Proxy::all(proxy)?)
                .danger_accept_invalid_certs(true)
        }

        let client = client.build()?;

        let url = format!("{}/me", Self::BASE_API);
        let res = client.get(&url).send().await?;
        let res = res.error_for_status()?;
        let me_res: SpotifyUserResponse = res.json().await?;
        let country_code = me_res.country;

        Ok(Self { client, debug, country_code })
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

        info!("Please authorize the app in your browser");
        let (socket, _) = listener.accept().await?;

        socket.readable().await?;
        let mut buffer = [0; 1024];
        let _ = socket.try_read(&mut buffer);

        let data = String::from_utf8(buffer.to_vec())?;
        let splits: Vec<&str> = data.split_whitespace().collect();
        if splits.len() <= 1 {
            return Err(eyre!("Invalid spotify server callback"));
        }
        let url = format!("{}{}", SpotifyApi::REDIRECT_URI_HOST, splits[1]);
        let auth_code = reqwest::Url::parse(&url)?
            .query_pairs()
            .find(|pair| pair.0 == "code")
            .ok_or(eyre!("Spotify server returned no autorization code"))?
            .1
            .to_string();

        Ok(auth_code)
    }

    fn build_endpoint(&self, path: &str) -> String {
        format!("{}{}", SpotifyApi::BASE_API, path)
    }

    async fn paginated_request<T>(
        &self,
        path: &str,
        method: HttpMethod<'_>,
        limit: u32,
    ) -> Result<SpotifyPageResponse<T>>
    where
        T: DeserializeOwned,
    {
        let mut offset = 0;
        let mut response: SpotifyPageResponse<T> =
            self.make_request_json(path, &method, limit, offset).await?;
        let mut total = response.total;

        while offset < total {
            offset += limit;
            let mut response2: SpotifyPageResponse<T> =
                self.make_request_json(path, &method, limit, offset).await?;
            total = response2.total;
            response.merge(&mut response2);
        }
        Ok(response)
    }

    async fn api_rate_wait(&self, res: &Response) -> Result<()> {
        let headers = res.headers();
        let sleep_time = headers
            .get("Retry-After")
            .ok_or(eyre!("Invalid Retry-After header"))?
            .to_str()?
            .parse::<u64>()?;
        warn!(
            "API rate limit reached, sleeping for {} seconds",
            sleep_time
        );
        tokio::time::sleep(Duration::from_secs(sleep_time)).await;
        Ok(())
    }

    #[async_recursion]
    async fn make_request(
        &self,
        path: &str,
        method: &HttpMethod<'_>,
        limit: u32,
        offset: u32,
    ) -> Result<Response> {
        let endpoint = self.build_endpoint(path);

        let mut request = match method {
            HttpMethod::Get(p) => self.client.get(endpoint).query(p),
            HttpMethod::Post(b) => self.client.post(endpoint).json(b),
            HttpMethod::Delete(b) => self.client.delete(endpoint).json(b),
        };
        request = request.query(&[("limit", limit), ("offset", offset)]);
        let res = request.send().await?;
        if res.status() == StatusCode::TOO_MANY_REQUESTS {
            self.api_rate_wait(&res).await?;
            // Retry request
            return self.make_request(path, method, limit, offset).await;
        }
        let res = res.error_for_status()?;
        if res.status() != StatusCode::OK && res.status() != StatusCode::CREATED {
            return Err(eyre!("Invalid response: {}", res.text().await?));
        }
        Ok(res)
    }

    async fn make_request_json<T>(
        &self,
        path: &str,
        method: &HttpMethod<'_>,
        limit: u32,
        offset: u32,
    ) -> Result<T>
    where
        T: DeserializeOwned,
    {
        let res = self.make_request(path, method, limit, offset).await?;
        let obj = if self.debug {
            let text = res.text().await?;
            std::fs::write("debug/spotify_last_res.json", &text).unwrap();
            serde_json::from_str(&text)?
        } else {
            res.json().await?
        };
        Ok(obj)
    }
}

pub fn push_query(queries: &mut Vec<String>, query: String, max_len: usize) {
    if query.len() > max_len {
        debug!("hit query limit: {}, skipping", query);
        return;
    }
    queries.push(query);
}

#[async_trait]
impl MusicApi for SpotifyApi {
    fn api_type(&self) -> MusicApiType {
        MusicApiType::Spotify
    }

    fn country_code(&self) -> &str {
        &self.country_code
    }

    async fn create_playlist(&self, name: &str, public: bool) -> Result<Playlist> {
        let path = "/me/playlists";
        let body = json!({
            "name": name,
            "public": public,
            "description": PLAYLIST_DESC,
        });
        let res: SpotifyPlaylistResponse = self
            .make_request_json(path, &HttpMethod::Post(&body), 50, 0)
            .await?;
        let playlist: Playlist = res.try_into()?;
        Ok(playlist)
    }

    async fn get_playlists_info(&self) -> Result<Vec<Playlist>> {
        let path = "/me/playlists";
        let res: SpotifyPageResponse<SpotifyPlaylistResponse> = self
            .paginated_request(path, HttpMethod::Get(&[]), 50)
            .await?;
        let playlists: Playlists = res.try_into()?;
        Ok(playlists.0)
    }

    async fn get_playlist_songs(&self, id: &str) -> Result<Vec<Song>> {
        let path = format!("/playlists/{}/tracks", id);
        let res: SpotifyPageResponse<SpotifySongItemResponse> = self
            .paginated_request(&path, HttpMethod::Get(&[]), 50)
            .await?;
        let songs: Songs = res.try_into()?;
        Ok(songs.0)
    }

    async fn add_songs_to_playlist(&self, playlist: &mut Playlist, songs: &[Song]) -> Result<()> {
        for song in songs {
            playlist.songs.push(song.clone());
        }

        let uris: Vec<String> = songs
            .iter()
            .map(|song| format!("spotify:track:{}", song.id))
            .collect();

        let path = format!("/playlists/{}/tracks", playlist.id);
        for u in uris.as_slice().chunks(100) {
            let body = json!({
                "uris": u,
            });
            let _: SpotifySnapshotResponse = self
                .make_request_json(&path, &HttpMethod::Post(&body), 50, 0)
                .await?;
        }
        Ok(())
    }

    async fn remove_songs_from_playlist(
        &self,
        playlist: &mut Playlist,
        songs: &[Song],
    ) -> Result<()> {
        for song in songs {
            playlist.songs.retain(|s| s != song);
        }

        let uris: Vec<serde_json::Value> = songs
            .iter()
            .map(|song| {
                let uri = format!("spotify:track:{}", song.id);
                json!({ "uri": uri })
            })
            .collect();
        let path = format!("/playlists/{}/tracks", playlist.id);
        let body = json!({
            "tracks": uris,
        });
        self.make_request_json::<SpotifySnapshotResponse>(&path, &HttpMethod::Delete(&body), 50, 0)
            .await?;
        Ok(())
    }

    async fn delete_playlist(&self, playlist: Playlist) -> Result<()> {
        let path = format!("/playlists/{}/followers", playlist.id);
        let body = json!({
            "playlist_id": playlist.id,
        });
        self.make_request(&path, &HttpMethod::Delete(&body), 50, 0)
            .await?;
        Ok(())
    }

    async fn search_song(&self, song: &Song) -> Result<Option<Song>> {
        let path = "/search";
        let max_len = 100;
        let mut queries = vec![];

        if let Some(isrc) = &song.isrc {
            queries.push(format!("isrc:{}", isrc));
        } else {
            let mut track_query = format!("track:\"{}\"", song.clean_name());
            if track_query.len() > max_len {
                warn!(
                    "song name is bigger than spotify max search: \"{}\", truncating",
                    track_query
                );
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

            // Query: Track + Album
            if let Some(album_query) = album_query.as_ref() {
                let tr_al_query = format!("{} {}", track_query, album_query);
                push_query(&mut queries, tr_al_query, max_len);
            }
            // Query: Track + Artist
            for artist_query in artist_queries.iter().rev() {
                // INFO: spotify doesn't support multiple artists in search
                // we have to create one query per artist
                let tr_ar_query = format!("{} {}", track_query, artist_query);
                push_query(&mut queries, tr_ar_query, max_len);
            }
            // Query: Track + Artist + Album
            if let Some(album_query) = album_query.as_ref() {
                for artist_query in artist_queries.iter().rev() {
                    let tr_ar_al_query =
                        format!("{} {} {}", track_query, artist_query, album_query);
                    push_query(&mut queries, tr_ar_al_query, max_len);
                }
            }
        }

        while let Some(query) = queries.pop() {
            let get_params = [("type", "track"), ("q", &query)];
            let res: SpotifySearchResponse = self
                .make_request_json(path, &HttpMethod::Get(&get_params), 3, 0)
                .await?;
            let res_songs: Songs = res.try_into()?;
            // iterate over top 3 results
            for res_song in res_songs.0.into_iter().take(3) {
                if song.compare(&res_song) {
                    return Ok(Some(res_song));
                }
            }
        }
        return Ok(None);
    }
}

#[cfg(test)]
mod tests {
    use std::env;

    use super::*;
    use crate::yt_music::YtMusicApi;

    #[tokio::test]
    async fn test_spotify_search_from_ytmusic() {
        let yt_client_id = env::var("YTMUSIC_CLIENT_ID").unwrap();
        let yt_client_secret = env::var("YTMUSIC_CLIENT_SECRET").unwrap();
        let config_dir = dirs::config_dir().unwrap();
        let oauth_token_path = config_dir.join("SyncDisBoi").join("ytmusic_oauth.json");
        let ytmusic = YtMusicApi::new(
            &yt_client_id,
            &yt_client_secret,
            oauth_token_path,
            false,
            false,
            None,
        )
        .await
        .unwrap();

        let playlists = ytmusic.get_playlists_info().await.unwrap();
        let test_spotify = playlists.iter().find(|p| p.name == "TestSpotify").unwrap();
        let songs = ytmusic.get_playlist_songs(&test_spotify.id).await.unwrap();

        let spotify_client_id = env::var("SPOTIFY_CLIENT_ID").unwrap();
        let spotify_secret = env::var("SPOTIFY_CLIENT_SECRET").unwrap();
        let spotify = SpotifyApi::new(&spotify_client_id, &spotify_secret, false, None)
            .await
            .unwrap();

        let songs = spotify.search_songs(&songs).await.unwrap();
        let correct_ids = [
            "2x1GoZKREbFkQJ8FUaz3Lc",
            // error on spotify side, album is "justice" instead of "cross"
            "none",
            "5dayqPrW7a4b2Skq3EcxWK",
            "1vU4X8ffq8oNcvvqkgTEXm",
            "1YqUm734e5Yv5BJEDhLYxK",
            "0qG1teoBvooRo7Z5Z8edCk",
            "32dnKMni3I3gwUbWp4mi45",
            "5HLdSJ0lsTulL0Lk7yTiYr",
            "3Eq7BJV1hGAiL8ctKoCrbD",
            "3F9ByoUqu31xU0I3G5xfVg",
        ];
        dbg!(&songs);
        for (i, song) in songs.into_iter().enumerate() {
            if let Some(song) = song {
                println!("Testing song: {}, id {}", song.name, song.id);
                assert_eq!(song.id, correct_ids[i]);
            } else {
                assert_eq!(correct_ids[i], "none");
            }
        }
    }
}
