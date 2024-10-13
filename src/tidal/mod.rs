mod model;
mod response;

use std::io::Read;
use std::path::PathBuf;

use async_trait::async_trait;
use color_eyre::eyre::eyre;
use color_eyre::Result;
use model::TidalMediaResponse;
use reqwest::header::HeaderMap;
use reqwest::Response;
use serde::de::DeserializeOwned;
use serde_json::json;
use tracing::info;

use self::model::{
    TidalDeviceRes, TidalMeResponse, TidalOAuthRefresh, TidalPageResponse, TidalPlaylistResponse,
    TidalSongItemResponse
};
use crate::music_api::{MusicApi, Playlist, Playlists, Song, Songs, PLAYLIST_DESC};
use crate::tidal::model::{
    TidalOAuthToken, TidalPlaylistCreateResponse, TidalReqToken, TidalSearchResponse,
};
use crate::yt_music::model::ItemSectionRendererContent;

pub struct TidalApi {
    client: reqwest::Client,
    debug: bool,
    user_id: usize,
}

#[derive(Debug)]
enum HttpMethod<'a> {
    Get(&'a serde_json::Value),
    Post(&'a serde_json::Value),
    Put(&'a serde_json::Value),
}

impl TidalApi {
    const API_URL: &'static str = "https://api.tidal.com";
    const API_V2_URL: &'static str = "https://openapi.tidal.com/v2";

    const AUTH_URL: &'static str = "https://auth.tidal.com/v1/oauth2/device_authorization";
    const TOKEN_URL: &'static str = "https://auth.tidal.com/v1/oauth2/token";
    const ME_URL: &'static str = "https://login.tidal.com/oauth2/me";
    const SCOPE: &'static str = "r_usr w_usr w_sub";

    pub async fn new(
        client_id: &str,
        client_secret: &str,
        oauth_token_path: PathBuf,
        clear_cache: bool,
        debug: bool,
        proxy: Option<&str>,
    ) -> Result<Self> {
        let token = if !oauth_token_path.exists() || clear_cache {
            info!("requesting new token");
            Self::request_token(client_id, client_secret).await?
        } else {
            info!("refreshing token");
            Self::refresh_token(client_id, client_secret, &oauth_token_path).await?
        };
        // Write new token
        let mut file = std::fs::File::create(&oauth_token_path)?;
        serde_json::to_writer(&mut file, &token)?;

        let mut headers = HeaderMap::new();
        headers.insert(
            "Authorization",
            format!("Bearer {}", token.access_token).parse()?,
        );
        headers.insert("Content-Type", "application/vnd.tidal.v1+json".parse()?);

        let mut client = reqwest::Client::builder()
            .cookie_store(true)
            .default_headers(headers);
        if let Some(proxy) = proxy {
            client = client
                .proxy(reqwest::Proxy::all(proxy)?)
                .danger_accept_invalid_certs(true)
        }
        let client = client.build()?;

        let res = client.get(Self::ME_URL).send().await?;
        let res = res.error_for_status()?;
        let me_res: TidalMeResponse = res.json().await?;

        Ok(Self {
            client,
            debug,
            user_id: me_res.user_id,
        })
    }

    async fn request_token(client_id: &str, client_secret: &str) -> Result<TidalOAuthToken> {
        let client = reqwest::Client::new();
        let params = json!({
            "client_id": client_id,
            "scope": Self::SCOPE,
        });
        let res = client.post(Self::AUTH_URL).form(&params).send().await?;
        let res = res.error_for_status()?;
        let device_res: TidalDeviceRes = res.json().await?;
        let url = format!("https://{}", device_res.verification_uri_complete);

        webbrowser::open(&url)?;
        info!("please authorize the app in your browser and press enter");
        std::io::stdin().read_exact(&mut [0]).unwrap();

        let auth_token = TidalReqToken {
            client_id: client_id.to_string(),
            device_code: device_res.device_code.clone(),
            grant_type: "urn:ietf:params:oauth:grant-type:device_code".to_string(),
            scope: Self::SCOPE.to_string(),
        };
        let res = client
            .post(Self::TOKEN_URL)
            .basic_auth(client_id, Some(client_secret))
            .form(&auth_token)
            .send()
            .await?;
        let token: TidalOAuthToken = res.json().await?;

        Ok(token)
    }

    async fn refresh_token(
        client_id: &str,
        client_secret: &str,
        oauth_token_path: &PathBuf,
    ) -> Result<TidalOAuthToken> {
        let client = reqwest::Client::new();
        let reader = std::fs::File::open(oauth_token_path)?;
        let mut oauth_token: TidalOAuthToken = serde_json::from_reader(reader)?;

        let params = json!({
            "client_id": client_id,
            "client_secret": client_secret,
            "grant_type": "refresh_token",
            "refresh_token": &oauth_token.refresh_token,
        });
        let res = client.post(Self::TOKEN_URL).form(&params).send().await?;
        let res = res.error_for_status()?;
        let refresh_token: TidalOAuthRefresh = res.json().await?;
        oauth_token.access_token = refresh_token.access_token;
        oauth_token.expires_in = refresh_token.expires_in;
        oauth_token.scope = refresh_token.scope;
        Ok(oauth_token)
    }

    async fn paginated_request<T>(
        &self,
        url: &str,
        method: &HttpMethod<'_>,
        limit: usize,
    ) -> Result<TidalPageResponse<T>>
    where
        T: DeserializeOwned + std::fmt::Debug,
    {
        let mut res: TidalPageResponse<T> = self.make_request_json(url, &method, limit, 0).await?;
        if res.items.is_empty() {
            return Ok(res);
        }

        while res.items.len() % limit == 0 {
            let offset = res.items.len();
            let res2: TidalPageResponse<T> = self.make_request_json(url, &method, limit, offset).await?;
            if res2.items.is_empty() {
                break;
            }
            res.items.extend(res2.items);
        }
        Ok(res)
    }

    async fn make_request(&self, url: &str, method: &HttpMethod<'_>, limit: usize, offset: usize) -> Result<Response> 
    {
        let mut request = match method {
            HttpMethod::Get(p) => {
                self.client.get(url).query(p)
            }
            HttpMethod::Post(b) => {
                self.client.post(url).json(b)
            },
            HttpMethod::Put(b) => {
                self.client.put(url).form(b)
            }
        };
        request = request.query(&[("limit", limit), ("offset", offset)]);
        let res = request.send().await?;
        let res = res.error_for_status()?;
        Ok(res)
    }

    async fn make_request_json<T>(&self, url: &str, method: &HttpMethod<'_>, limit: usize, offset: usize) -> Result<T> 
    where
        T: DeserializeOwned,
    {
        let res = self.make_request(url, method, limit, offset).await?;
        let obj = if self.debug {
            let text = res.text().await?;
            std::fs::write("debug/tidal_last_res.json", &text).unwrap();
            serde_json::from_str(&text)?
        } else {
            res.json().await?
        };
        Ok(obj)
    }
}

#[async_trait]
impl MusicApi for TidalApi {
    async fn create_playlist(&self, name: &str, public: bool) -> Result<Playlist> {
        let url = format!(
            "{}/v2/my-collection/playlists/folders/create-playlist",
            Self::API_URL
        );
        let params = json!({
            "name": name,
            "description": PLAYLIST_DESC,
            "public": public,
            "folderId": "root"
        });
        let res: TidalPlaylistCreateResponse = self.make_request_json(&url, &HttpMethod::Put(&params), 0, 5).await?;

        Ok(Playlist {
            id: res.data.uuid,
            name: name.to_string(),
            songs: vec![],
        })
    }

    async fn get_playlists_info(&self) -> Result<Vec<Playlist>> {
        let url = format!("{}/v1/users/{}/playlists", Self::API_URL, self.user_id);
        let params = json!({
            "countryCode": "US",
        });
        let res: TidalPageResponse<TidalPlaylistResponse> =
            self.paginated_request(&url, &HttpMethod::Get(&params), 100).await?;
        let playlists: Playlists = res.try_into()?;
        Ok(playlists.0)
    }

    async fn get_playlist_songs(&self, id: &str) -> Result<Vec<Song>> {
        dbg!("get_playlist_songs");
        let url = format!("{}/v1/playlists/{}/items", Self::API_URL, id);
        let params = json!({
            "countryCode": "US",
        });
        let res: TidalPageResponse<TidalSongItemResponse> =
            self.paginated_request(&url, &HttpMethod::Get(&params), 100).await?;
        let songs: Songs = res.try_into()?;
        Ok(songs.0)
    }

    async fn add_songs_to_playlist(&self, playlist: &mut Playlist, songs: &[Song]) -> Result<()> {
        dbg!("add_songs_to_playlist");
        if songs.is_empty() {
            return Ok(());
        }

        // TODO: accomodate make_request to access response headers + body

        let url = format!("{}/v1/playlists/{}", Self::API_URL, playlist.id);
        let params = json!({
            "countryCode": "US",
        });
        let res = self.client.get(url).query(&params).send().await?;
        let res = res.error_for_status()?;
        let etag = res
            .headers()
            .get("ETag")
            .ok_or(eyre!("No ETag in Tidal Response"))?;

        let url = format!("{}/v1/playlists/{}/items", Self::API_URL, playlist.id);
        let params = json!({
            "trackIds": songs.iter().map(|s| s.id.as_str()).collect::<Vec<_>>().join(","),
            "onDuplicate": "FAIL",
            "onArtifactNotFound": "FAIL",
        });
        let res = self
            .client
            .post(url)
            .header("If-None-Match", etag)
            .form(&params)
            .send()
            .await?;
        res.error_for_status()?;

        Ok(())
    }

    async fn remove_songs_from_playlist(
        &self,
        _playlist: &mut Playlist,
        _songs_ids: &[Song],
    ) -> Result<()> {
        todo!()
    }

    async fn delete_playlist(&self, playlist: Playlist) -> Result<()> {
        let url = format!("{}/v2/my-collection/playlists/folders/remove", Self::API_URL);
        let params = json!({
            "trns": format!("trn:playlist:{}", playlist.id),
        });
        let res = self.make_request(&url, &HttpMethod::Put(&params), 0, 5).await?;
        Ok(())
    }

    async fn search_song(&self, song: &Song) -> Result<Option<Song>> {
        if let Some(isrc) = &song.isrc {
            let url = format!("{}/tracks", Self::API_V2_URL);
            let params = json!({
                "countryCode": "US",
                "include": "albums,artists",
                "filter[isrc]": isrc.to_uppercase(),
            });
            let res: TidalMediaResponse = self.make_request_json(&url, &HttpMethod::Get(&params), 0, 3).await?;
            if res.data.is_empty() {
                return Ok(None);
            }
            let res_song: Song = res.try_into()?;
            return Ok(Some(res_song));
        }

        let url = format!("{}/v1/search", Self::API_URL);
        let mut queries = song.build_queries();

        while let Some(query) = queries.pop() {
            let params = json!({
                "countryCode": "US",
                "query": query,
                "type": "TRACKS",
            });
            let res: TidalSearchResponse = self.make_request_json(&url, &HttpMethod::Get(&params), 0, 3).await?;
            let res_songs: Songs = res.try_into()?;
            // iterate over top 3 results
            for res_song in res_songs.0.into_iter().take(3) {
                if song.compare(&res_song) {
                    return Ok(Some(res_song));
                }
            }
        }
        Ok(None)
    }
}
