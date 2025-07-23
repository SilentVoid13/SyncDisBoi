pub mod model;
mod response;

use std::collections::HashMap;
use std::fmt::Write;
use std::io::Read;
use std::path::PathBuf;
use std::sync::LazyLock;

use async_trait::async_trait;
use color_eyre::eyre::Result;
use reqwest::header::HeaderMap;
use serde_json::json;
use tracing::{error, info, warn};
use ytmapi_rs::YtMusic;
use ytmapi_rs::auth::{OAuthToken, OAuthTokenGenerator};
use ytmapi_rs::common::{PlaylistID, VideoID, YoutubeID};
use ytmapi_rs::parse::PlaylistItem;
use ytmapi_rs::query::CreatePlaylistQuery;
use ytmapi_rs::query::playlist::{GetWatchPlaylistQueryID, PrivacyStatus};

use crate::ConfigArgs;
use crate::music_api::{MusicApi, MusicApiType, PLAYLIST_DESC, Playlist, Song};

pub struct YtMusicApi {
    client: reqwest::Client,
    client_yt: ytmapi_rs::YtMusic<ytmapi_rs::auth::OAuthToken>,
    config: ConfigArgs,
}

impl YtMusicApi {
    const BASE_API: &'static str = "https://music.youtube.com/youtubei/v1/";
    const BASE_PARAMS: &'static str = "?alt=json&key=AIzaSyC9XL3ZjWddXya6X74dJoCTL-WEYFDNX30";

    const OAUTH_SCOPE: &'static str = "https://www.googleapis.com/auth/youtube";
    const OAUTH_CODE_URL: &'static str = "https://www.youtube.com/o/oauth2/device/code";
    const OAUTH_TOKEN_URL: &'static str = "https://oauth2.googleapis.com/token";
    const OAUTH_GRANT_TYPE: &'static str = "http://oauth.net/grant_type/device/1.0";
    const OAUTH_USER_AGENT: &'static str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64; rv:88.0) Gecko/20100101 Firefox/88.0 Cobalt/Version";

    pub async fn new_oauth(
        client_id: &str,
        client_secret: &str,
        oauth_token_path: PathBuf,
        clear_cache: bool,
        config: ConfigArgs,
    ) -> Result<Self> {
        let mut headers = HeaderMap::new();
        headers.insert("User-Agent", Self::OAUTH_USER_AGENT.parse()?);
        let client = reqwest::Client::builder()
            .default_headers(headers)
            .build()?;

        let (yt, token) = if !oauth_token_path.exists() || clear_cache {
            let token = Self::request_token(&client, client_id, client_secret).await?;
            let yt = YtMusic::from_auth_token(token.clone());
            (yt, token)
        } else {
            info!("refreshing token");
            let reader = std::fs::File::open(&oauth_token_path)?;
            let oauth_token: ytmapi_rs::auth::OAuthToken = serde_json::from_reader(reader)?;
            let mut yt = YtMusic::from_auth_token(oauth_token);
            let new_token = yt.refresh_token().await?;
            (yt, new_token)
        };
        // Write new token
        let mut file = std::fs::File::create(&oauth_token_path)?;
        serde_json::to_writer(&mut file, &token)?;

        // TODO: add proxy
        // if let Some(proxy) = &config.proxy {
        //     client = client
        //         .proxy(reqwest::Proxy::all(proxy)?)
        //         .danger_accept_invalid_certs(true)
        // }

        Ok(YtMusicApi {
            client: reqwest::Client::new(),
            client_yt: yt,
            config,
        })
    }

    async fn request_token(
        client: &reqwest::Client,
        client_id: &str,
        client_secret: &str,
    ) -> Result<OAuthToken> {
        // 1. request access
        let params = json!({
            "client_id": client_id,
            "scope": Self::OAUTH_SCOPE,
        });
        let res = client
            .post(Self::OAUTH_CODE_URL)
            .form(&params)
            .send()
            .await?;
        let res = res.error_for_status()?;
        let oauth_res: OAuthTokenGenerator = res.json().await?;

        let auth_url = format!(
            "{}?user_code={}",
            oauth_res.verification_url, oauth_res.user_code
        );
        webbrowser::open(&auth_url)?;
        info!("please authorize the app in your browser and press enter");
        std::io::stdin().read_exact(&mut [0])?;

        // 2. request the token
        let mut params = HashMap::new();
        params.insert("client_id", client_id);
        params.insert("code", oauth_res.device_code.get_code());
        params.insert("client_secret", client_secret);
        params.insert("grant_type", Self::OAUTH_GRANT_TYPE);
        let res = client
            .post(Self::OAUTH_TOKEN_URL)
            .form(&params)
            .send()
            .await?;
        let res = res.error_for_status()?;
        let token: crate::music_api::OAuthToken = res.json().await?;

        // HACK: ytmapi_rs doesn't allow creating an OAuthToken manually
        let token = serde_json::from_value(json!({
            "token_type": token.token_type,
            "access_token": token.access_token,
            "refresh_token": token.refresh_token,
            "expires_in": token.expires_in * 2,
            "request_time": std::time::SystemTime::now(),
            "client_id": client_id,
            "client_secret": client_secret,
        }))?;

        Ok(token)
    }

    pub async fn new_headers(headers: &PathBuf, config: ConfigArgs) -> Result<Self> {
        todo!();

        /* let header_data = std::fs::read_to_string(headers)?;
        let header_json: serde_json::Map<String, serde_json::Value> =
            serde_json::from_str(&header_data)?;
        let mut headers = HeaderMap::new();
        for (key, val) in header_json.into_iter() {
            if let serde_json::Value::String(s) = val {
                headers.insert(
                    HeaderName::from_bytes(key.to_lowercase().as_bytes())?,
                    s.parse()?,
                );
            }
        }
        headers.remove("accept-encoding");
        headers.remove("content-encoding");

        let mut client = reqwest::ClientBuilder::new()
            .cookie_store(true)
            .default_headers(headers);

        // TODO: add proxy
        // if let Some(proxy) = &config.proxy {
        //     client = client
        //         .proxy(reqwest::Proxy::all(proxy)?)
        //         .danger_accept_invalid_certs(true)
        // }

        let client = client.build().unwrap();

        Ok(YtMusicApi { client, config }) */
    }

    fn build_endpoint(&self, path: &str, ctoken: Option<&str>) -> String {
        let mut endpoint = format!("{}{}{}", Self::BASE_API, path, Self::BASE_PARAMS,);
        if let Some(c) = ctoken {
            std::write!(&mut endpoint, "&ctoken={c}&continuation={c}", c = c).unwrap();
        }
        endpoint
    }

    pub fn clean_playlist_id(id: &str) -> String {
        if let Some(id) = id.strip_prefix("VL") {
            return id.to_string();
        }
        id.to_string()
    }
}

#[async_trait]
impl MusicApi for YtMusicApi {
    fn api_type(&self) -> MusicApiType {
        MusicApiType::YtMusic
    }

    fn country_code(&self) -> &str {
        // TODO: it seems impossible to get the country code from YtMusic
        "UNKNOWN"
    }

    async fn create_playlist(&self, name: &str, public: bool) -> Result<Playlist> {
        let privacy_status = match public {
            true => PrivacyStatus::Public,
            false => PrivacyStatus::Private,
        };
        let query = CreatePlaylistQuery::new(name, Some(PLAYLIST_DESC), privacy_status);
        let id = self
            .client_yt
            .create_playlist(query)
            .await?
            .get_playlist_id()
            .to_string();

        Ok(Playlist {
            id,
            name: name.to_string(),
            songs: vec![],
        })
    }

    async fn get_playlists_info(&self) -> Result<Vec<Playlist>> {
        let playlists = self.client_yt.get_library_playlists().await?;
        let mut res = Vec::new();
        for playlist in playlists {
            let id = playlist.playlist_id.get_raw().to_string();
            let name = playlist.title;
            let playlist = Playlist {
                id,
                name,
                songs: vec![],
            };
            res.push(playlist);
        }
        Ok(res)
    }

    async fn get_playlist_songs(&self, id: &str) -> Result<Vec<Song>> {
        use ytmapi_rs::common::PlaylistID;
        let Ok(tracks) = self
            .client_yt
            .get_playlist_tracks(PlaylistID::from_raw(id))
            .await
        else {
            error!("Failed to get playlist tracks for id: {}", id);
            return Ok(Vec::new());
        };
        let mut res = Vec::new();
        for track in tracks {
            match track {
                PlaylistItem::Song(song) => match song.try_into() {
                    Ok(s) => {
                        res.push(s);
                    }
                    Err(e) => {
                        error!("Error in playlist response: {}", e);
                    }
                },
                PlaylistItem::Video(_) => {
                    // warn!(
                    //     "found video in playlist '{}': {}, skipping it",
                    //     id, video.title,
                    // );
                }
                _ => {}
            }
        }
        Ok(res)
    }

    async fn add_songs_to_playlist(&self, playlist: &mut Playlist, songs: &[Song]) -> Result<()> {
        if songs.is_empty() {
            return Ok(());
        }
        for song in songs {
            playlist.songs.push(song.clone());
        }
        info!("adding songss: {:#?}", songs);

        let id = Self::clean_playlist_id(&playlist.id);
        let playlist_id = PlaylistID::from_raw(&id);
        let video_ids = songs
            .iter()
            .map(|s| VideoID::from_raw(&s.id))
            .collect::<Vec<_>>();
        dbg!(&video_ids);
        dbg!(&playlist_id);
        self.client_yt
            .add_video_items_to_playlist(playlist_id, video_ids)
            .await?;

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

        // TODO: add remove_songs_to_playlist
        todo!();
    }

    async fn delete_playlist(&self, playlist: Playlist) -> Result<()> {
        let playlist_id = PlaylistID::from_raw(&playlist.id);
        self.client_yt.delete_playlist(playlist_id).await?;
        Ok(())
    }

    async fn search_song(&self, song: &Song) -> Result<Option<Song>> {
        if let Some(isrc) = &song.isrc {
            let q = format!("\"{}\"", isrc);
            let res = match self.client_yt.search_songs(q).await {
                Ok(s) => s,
                Err(e) => {
                    error!("Error converting search result: {}", e);
                    return Ok(None);
                }
            };
            if let Some(res_song) = res.into_iter().next() {
                let mut res_song: Song = res_song.try_into()?;
                res_song.isrc = Some(isrc.to_string());
                return Ok(Some(res_song));
            }
        } else {
            let mut queries = song.build_queries();
            while let Some(query) = queries.pop() {
                let res = self.client_yt.search_songs(query).await?;
                // iterate over top 3 results
                for res_song in res.into_iter().take(3) {
                    let res_song = res_song.try_into()?;
                    if song.compare(&res_song) {
                        return Ok(Some(res_song));
                    }
                }
            }
        }
        Ok(None)
    }

    async fn add_like(&self, songs: &[Song]) -> Result<()> {
        // TODO: find a way to bulk-like
        for song in songs {
            self.client_yt
                .rate_song(
                    VideoID::from_raw(&song.id),
                    ytmapi_rs::common::LikeStatus::Liked,
                )
                .await?;
        }
        Ok(())
    }

    async fn get_likes(&self) -> Result<Vec<Song>> {
        let songs = self.get_playlist_songs("LM").await?;
        Ok(songs)
    }
}

#[cfg(test)]
mod tests {}
