mod model;
mod response;

use std::io::Read;
use std::path::PathBuf;

use async_trait::async_trait;
use color_eyre::Result;
use color_eyre::eyre::eyre;
use model::{TidalMediaResponse, TidalMediaResponseSingle, TidalOAuthDeviceRes};
use reqwest::Response;
use reqwest::header::HeaderMap;
use serde::de::DeserializeOwned;
use serde_json::json;
use tracing::info;

use self::model::{TidalPageResponse, TidalPlaylistResponse, TidalSongItemResponse};
use crate::ConfigArgs;
use crate::music_api::{
    MusicApi, MusicApiType, OAuthRefreshToken, OAuthReqToken, OAuthToken, PLAYLIST_DESC, Playlist,
    Playlists, Song, Songs,
};
use crate::tidal::model::{TidalPlaylistCreateResponse, TidalSearchResponse};

pub struct TidalApi {
    client: reqwest::Client,
    config: ConfigArgs,
    user_id: String,
    country_code: String,
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
    const SCOPE: &'static str = "r_usr w_usr w_sub";
    const RES_DEBUG_FILE: &'static str = "debug/tidal_last_res.json";

    pub async fn new(
        client_id: &str,
        client_secret: &str,
        oauth_token_path: PathBuf,
        clear_cache: bool,
        config: ConfigArgs,
    ) -> Result<Self> {
        let token = if !oauth_token_path.exists() || clear_cache {
            info!("requesting new token");
            Self::request_token(client_id, client_secret, &config).await?
        } else {
            info!("refreshing token");
            Self::refresh_token(client_id, client_secret, &oauth_token_path, &config).await?
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
        if let Some(proxy) = &config.proxy {
            client = client
                .proxy(reqwest::Proxy::all(proxy)?)
                .danger_accept_invalid_certs(true)
        }
        let client = client.build()?;

        let url = format!("{}/users/me", Self::API_V2_URL);
        let res = client.get(&url).send().await?;
        let status = res.status();
        let me_res: TidalMediaResponseSingle = Self::debug_response_json(&config, res).await?;
        if !status.is_success() {
            return Err(eyre!("Invalid HTTP status: {}", status));
        }
        let country_code = me_res.data.attributes.country.unwrap_or("US".into());

        Ok(Self {
            client,
            config,
            user_id: me_res.data.id,
            country_code,
        })
    }

    async fn request_token(
        client_id: &str,
        client_secret: &str,
        config: &ConfigArgs,
    ) -> Result<OAuthToken> {
        let client = reqwest::Client::new();
        let params = json!({
            "client_id": client_id,
            "scope": Self::SCOPE,
        });
        let res = client.post(Self::AUTH_URL).form(&params).send().await?;
        let status = res.status();
        let device_res: TidalOAuthDeviceRes = Self::debug_response_json(config, res).await?;
        if !status.is_success() {
            return Err(eyre!("Invalid HTTP status: {}", status));
        }

        let url = format!("https://{}", device_res.verification_uri_complete);

        webbrowser::open(&url)?;
        info!("please authorize the app in your browser and press enter");
        std::io::stdin().read_exact(&mut [0])?;

        let auth_token = OAuthReqToken {
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
        let status = res.status();
        let token: OAuthToken = Self::debug_response_json(config, res).await?;
        if !status.is_success() {
            return Err(eyre!("Invalid HTTP status: {}", status));
        }

        Ok(token)
    }

    async fn refresh_token(
        client_id: &str,
        client_secret: &str,
        oauth_token_path: &PathBuf,
        config: &ConfigArgs,
    ) -> Result<OAuthToken> {
        let client = reqwest::Client::new();
        let reader = std::fs::File::open(oauth_token_path)?;
        let mut oauth_token: OAuthToken = serde_json::from_reader(reader)?;

        let params = json!({
            "client_id": client_id,
            "client_secret": client_secret,
            "grant_type": "refresh_token",
            "refresh_token": &oauth_token.refresh_token,
        });

        let res = client.post(Self::TOKEN_URL).form(&params).send().await?;
        let status = res.status();
        let refresh_token: OAuthRefreshToken = Self::debug_response_json(config, res).await?;
        if !status.is_success() {
            return Err(eyre!("Invalid HTTP status: {}", status));
        }

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
        let mut res: TidalPageResponse<T> = self
            .make_request_json(url, method, Some((limit, 0)))
            .await?;
        if res.items.is_empty() {
            return Ok(res);
        }

        let mut offset = limit;
        while offset < res.total_number_of_items {
            let res2: TidalPageResponse<T> = self
                .make_request_json(url, method, Some((limit, offset)))
                .await?;
            if res2.items.is_empty() {
                break;
            }
            res.items.extend(res2.items);
            offset += limit;
        }
        Ok(res)
    }

    async fn make_request_json<T>(
        &self,
        url: &str,
        method: &HttpMethod<'_>,
        lim_off: Option<(usize, usize)>,
    ) -> Result<T>
    where
        T: DeserializeOwned,
    {
        let mut request = match method {
            HttpMethod::Get(p) => self.client.get(url).query(p),
            HttpMethod::Post(b) => self.client.post(url).form(b),
            HttpMethod::Put(b) => self.client.put(url).form(b),
        };
        if let Some((limit, offset)) = lim_off {
            request = request.query(&[("limit", limit), ("offset", offset)]);
        }

        let res = request.send().await?;
        let status = res.status();
        let obj = Self::debug_response_json(&self.config, res).await?;
        if !status.is_success() {
            return Err(eyre!("Invalid HTTP status: {}", status));
        }
        Ok(obj)
    }

    async fn debug_response_json<T>(config: &ConfigArgs, res: Response) -> Result<T>
    where
        T: DeserializeOwned,
    {
        Ok(if config.debug {
            let text = res.text().await?;
            std::fs::write(Self::RES_DEBUG_FILE, &text)?;
            serde_json::from_str(&text)?
        } else {
            res.json().await?
        })
    }
}

#[async_trait]
impl MusicApi for TidalApi {
    fn api_type(&self) -> MusicApiType {
        MusicApiType::Tidal
    }

    fn country_code(&self) -> &str {
        &self.country_code
    }

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
        let res: TidalPlaylistCreateResponse = self
            .make_request_json(&url, &HttpMethod::Put(&params), Some((5, 0)))
            .await?;

        Ok(Playlist {
            id: res.data.uuid,
            name: name.to_string(),
            songs: vec![],
        })
    }

    async fn get_playlists_info(&self) -> Result<Vec<Playlist>> {
        let url = format!("{}/v1/users/{}/playlists", Self::API_URL, self.user_id);
        let params = json!({
            "countryCode": self.country_code,
        });
        let res: TidalPageResponse<TidalPlaylistResponse> = self
            .paginated_request(&url, &HttpMethod::Get(&params), 100)
            .await?;
        let playlists: Playlists = res.try_into()?;
        Ok(playlists.0)
    }

    async fn get_playlist_songs(&self, id: &str) -> Result<Vec<Song>> {
        let url = format!("{}/v1/playlists/{}/items", Self::API_URL, id);
        let params = json!({
            "countryCode": self.country_code,
        });
        // NOTE: a limit > 100 triggers a 400 error
        let res: TidalPageResponse<TidalSongItemResponse> = self
            .paginated_request(&url, &HttpMethod::Get(&params), 100)
            .await?;
        let songs: Songs = res.try_into()?;
        Ok(songs.0)
    }

    async fn add_songs_to_playlist(&self, playlist: &mut Playlist, songs: &[Song]) -> Result<()> {
        if songs.is_empty() {
            return Ok(());
        }

        // 1. query playlist ETag
        let url = format!("{}/v1/playlists/{}", Self::API_URL, playlist.id);
        let params = json!({
            "countryCode": self.country_code,
        });
        let res = self.client.get(&url).query(&params).send().await?;
        let status = res.status();
        let etag = res.headers().get("ETag").cloned();
        let () = Self::debug_response_json(&self.config, res).await?;
        let etag = etag.ok_or(eyre!("No ETag in Tidal Response"))?;
        if !status.is_success() {
            return Err(eyre!("Invalid HTTP status: {}", status));
        }

        // 2. add songs to playlist
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
        let status = res.status();
        let () = Self::debug_response_json(&self.config, res).await?;
        if !status.is_success() {
            return Err(eyre!("Invalid HTTP status: {}", status));
        }

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
        let url = format!(
            "{}/v2/my-collection/playlists/folders/remove",
            Self::API_URL
        );
        let params = json!({
            "trns": format!("trn:playlist:{}", playlist.id),
        });
        let () = self
            .make_request_json(&url, &HttpMethod::Put(&params), None)
            .await?;
        Ok(())
    }

    async fn search_song(&self, song: &Song) -> Result<Option<Song>> {
        if let Some(isrc) = &song.isrc {
            let url = format!("{}/tracks", Self::API_V2_URL);
            let params = json!({
                "countryCode": self.country_code,
                "include": "albums,artists",
                "filter[isrc]": isrc.to_uppercase(),
            });
            let res: TidalMediaResponse = self
                .make_request_json(&url, &HttpMethod::Get(&params), Some((1, 0)))
                .await?;
            if res.data.is_empty() {
                return Ok(None);
            }
            let mut res_songs: Songs = res.try_into()?;
            if res_songs.0.is_empty() {
                return Ok(None);
            }
            return Ok(Some(res_songs.0.remove(0)));
        }

        let url = format!("{}/v1/search", Self::API_URL);
        let mut queries = song.build_queries();

        while let Some(query) = queries.pop() {
            let params = json!({
                "countryCode": self.country_code,
                "query": query,
                "type": "TRACKS",
            });
            let res: TidalSearchResponse = self
                .make_request_json(&url, &HttpMethod::Get(&params), Some((3, 0)))
                .await?;
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

    async fn add_likes(&self, songs: &[Song]) -> Result<()> {
        if songs.is_empty() {
            return Ok(());
        }

        let url = format!(
            "{}/v1/users/{}/favorites/tracks",
            Self::API_URL,
            self.user_id
        );
        let tracks = songs.iter().map(|s| s.id.as_str()).collect::<Vec<_>>();

        // NOTE: we get error 500 if we like too much songs at once
        for tracks_chunk in tracks.chunks(100) {
            let params = json!({
                "countryCode": self.country_code,
                "trackIds": tracks_chunk.join(","),
                "onArtifactNotFound": "FAIL",
            });
            let () = self
                .make_request_json(&url, &HttpMethod::Post(&params), None)
                .await?;
        }
        Ok(())
    }

    async fn get_likes(&self) -> Result<Vec<Song>> {
        let url = format!(
            "{}/v1/users/{}/favorites/tracks",
            Self::API_URL,
            self.user_id
        );
        let params = json!({
            "countryCode": self.country_code,
        });
        let res: TidalPageResponse<TidalSongItemResponse> = self
            .paginated_request(&url, &HttpMethod::Get(&params), 1000)
            .await?;
        let songs: Songs = res.try_into()?;
        Ok(songs.0)
    }
}
