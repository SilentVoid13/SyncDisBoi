pub mod model;
mod response;

use std::collections::HashMap;
use std::fmt::Write;
use std::io::Read;
use std::path::PathBuf;

use async_trait::async_trait;
use color_eyre::eyre::{eyre, Result};
use lazy_static::lazy_static;
use model::YtMusicOAuthDeviceRes;
use reqwest::header::HeaderMap;
use serde::de::DeserializeOwned;
use serde_json::json;
use tracing::info;

use self::model::{
    YtMusicContinuationResponse,
    YtMusicPlaylistEditResponse, YtMusicResponse,
};
use crate::music_api::{MusicApi, OAuthRefreshToken, OAuthToken, Playlist, Playlists, Song, Songs, PLAYLIST_DESC};
use crate::yt_music::model::{
    YtMusicPlaylistCreateResponse, YtMusicPlaylistDeleteResponse,
};
use crate::yt_music::response::SearchSongs;

lazy_static! {
    static ref CONTEXT: serde_json::Value = json!({
        "client": {
            "clientName": "WEB_REMIX",
            "clientVersion": "1.20230901.01.00",
            "hl": "en"
        },
        "user": {}
    });
}

pub struct YtMusicApi {
    client: reqwest::Client,
    debug: bool,
}

impl YtMusicApi {
    const BASE_API: &'static str = "https://music.youtube.com/youtubei/v1/";
    const BASE_PARAMS: &'static str =
        "?alt=json&prettyPrint=false&key=AIzaSyC9XL3ZjWddXya6X74dJoCTL-WEYFDNX30";

    const OAUTH_SCOPE: &'static str = "https://www.googleapis.com/auth/youtube";
    const OAUTH_CODE_URL: &'static str = "https://www.youtube.com/o/oauth2/device/code";
    const OAUTH_TOKEN_URL: &'static str = "https://oauth2.googleapis.com/token";
    const OAUTH_GRANT_TYPE: &'static str = "http://oauth.net/grant_type/device/1.0";
    const OAUTH_USER_AGENT: &'static str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64; rv:88.0) Gecko/20100101 Firefox/88.0 Cobalt/Version";

    pub async fn new(
        client_id: &str,
        client_secret: &str,
        oauth_token_path: PathBuf,
        clear_cache: bool,
        debug: bool,
        proxy: Option<&str>,
    ) -> Result<Self> {
        let mut headers = HeaderMap::new();
        headers.insert("User-Agent", Self::OAUTH_USER_AGENT.parse()?);
        let client = reqwest::Client::builder()
            .default_headers(headers)
            .build()?;

        let token = if !oauth_token_path.exists() || clear_cache {
            Self::request_token(&client, client_id, client_secret).await?
        } else {
            Self::refresh_token(&client, client_id, client_secret, &oauth_token_path).await?
        };
        // Write new token
        let mut file = std::fs::File::create(&oauth_token_path)?;
        serde_json::to_writer(&mut file, &token)?;

        let mut headers = HeaderMap::new();
        headers.insert("User-Agent", Self::OAUTH_USER_AGENT.parse()?);
        headers.insert("Cookie", "SOCS=CAI".parse()?);
        headers.insert(
            "Authorization",
            format!("Bearer {}", token.access_token).parse()?,
        );

        let mut client = reqwest::Client::builder()
            .cookie_store(true)
            .default_headers(headers);
        if let Some(proxy) = proxy {
            client = client
                .proxy(reqwest::Proxy::all(proxy)?)
                .danger_accept_invalid_certs(true)
        }
        let client = client.build()?;

        Ok(YtMusicApi { client, debug })
    }

    async fn refresh_token(
        client: &reqwest::Client,
        client_id: &str,
        client_secret: &str,
        oauth_token_path: &PathBuf,
    ) -> Result<OAuthToken> {
        let reader = std::fs::File::open(oauth_token_path)?;
        let mut oauth_token: OAuthToken = serde_json::from_reader(reader)?;

        let params = json!({
            "client_id": client_id,
            "client_secret": client_secret,
            "grant_type": "refresh_token",
            "refresh_token": &oauth_token.refresh_token,
        });
        let res = client
            .post(Self::OAUTH_TOKEN_URL)
            .form(&params)
            .send()
            .await?;
        let res = res.error_for_status()?;
        let refresh_token: OAuthRefreshToken = res.json().await?;
        oauth_token.access_token = refresh_token.access_token;
        oauth_token.expires_in = refresh_token.expires_in;
        Ok(oauth_token)
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
        let oauth_res: YtMusicOAuthDeviceRes = res.json().await?;

        let auth_url = format!(
            "{}?user_code={}",
            oauth_res.verification_url, oauth_res.user_code
        );
        webbrowser::open(&auth_url)?;
        info!("please authorize the app in your browser and press enter");
        std::io::stdin().read_exact(&mut [0]).unwrap();

        // 2. request the token
        let mut params = HashMap::new();
        params.insert("client_id", client_id);
        params.insert("code", &oauth_res.device_code);
        params.insert("client_secret", client_secret);
        params.insert("grant_type", Self::OAUTH_GRANT_TYPE);
        let res = client
            .post(Self::OAUTH_TOKEN_URL)
            .form(&params)
            .send()
            .await?;
        let res = res.error_for_status()?;
        let token: OAuthToken = res.json().await?;
        Ok(token)
    }

    fn build_endpoint(&self, path: &str, ctoken: Option<&str>) -> String {
        let mut endpoint = format!("{}{}{}", Self::BASE_API, path, Self::BASE_PARAMS,);
        if let Some(c) = ctoken {
            std::write!(&mut endpoint, "&ctoken={c}&continuation={c}", c = c).unwrap();
        }
        endpoint
    }

    fn add_context(&self, body: &serde_json::Value) -> serde_json::Value {
        let mut body = body.clone();
        match body.as_object_mut() {
            Some(o) => o.insert("context".to_string(), CONTEXT.clone()),
            _ => unreachable!(),
        };
        body
    }

    async fn paginated_request(
        &self,
        path: &str,
        body: &serde_json::Value,
    ) -> Result<YtMusicResponse> {
        let mut response: YtMusicResponse = self.make_request(path, body, None).await?;
        let mut continuation = response.get_continuation();

        while let Some(cont) = continuation {
            let mut response2: YtMusicContinuationResponse =
                self.make_request(path, body, Some(&cont)).await?;
            response.merge(&mut response2);
            continuation = response2.get_continuation();
        }
        Ok(response)
    }

    async fn make_request<T>(
        &self,
        path: &str,
        body: &serde_json::Value,
        ctoken: Option<&str>,
    ) -> Result<T>
    where
        T: DeserializeOwned + std::fmt::Debug,
    {
        let body = self.add_context(body);
        let endpoint = self.build_endpoint(path, ctoken);
        let res = self.client.post(endpoint).json(&body).send().await?;
        let res = res.error_for_status()?;
        let obj = if self.debug {
            let text = res.text().await?;
            std::fs::write("debug/yt_music_last_res.json", &text).unwrap();
            serde_json::from_str(&text)?
        } else {
            res.json().await?
        };
        Ok(obj)
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
    async fn create_playlist(&self, name: &str, public: bool) -> Result<Playlist> {
        let privacy_status = match public {
            true => "PUBLIC",
            false => "PRIVATE",
        };
        let body = json!({
            "title": name,
            "description": PLAYLIST_DESC,
            "privacyStatus": privacy_status,
        });
        let response: YtMusicPlaylistCreateResponse =
            self.make_request("playlist/create", &body, None).await?;
        let id = Self::clean_playlist_id(&response.playlist_id);
        Ok(Playlist {
            id,
            name: name.to_string(),
            songs: vec![],
        })
    }

    async fn get_playlists_info(&self) -> Result<Vec<Playlist>> {
        let browse_id = "FEmusic_liked_playlists";
        let body = json!({ "browseId": browse_id });
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
        let body = json!({ "browseId": browse_id });
        let response = self.paginated_request("browse", &body).await?;
        let songs: Songs = response.try_into()?;

        /*
        let body = json!({
            "playbackContext": {"contentPlaybackContext": {"signatureTimestamp": "20008"}},
            "video_id": "qV4rU7fzyx8",
        });
        let res: () = self.make_request("player", &body, None).await?;
        todo!();
        */

        Ok(songs.0)
    }

    async fn add_songs_to_playlist(&self, playlist: &mut Playlist, songs: &[Song]) -> Result<()> {
        for song in songs {
            playlist.songs.push(song.clone());
        }

        let mut actions = vec![];
        for song in songs.iter() {
            let action = json!({
                "action": "ACTION_ADD_VIDEO",
                "addedVideoId": song.id,
            });
            actions.push(action);
        }
        let body = json!({
            "playlistId": playlist.id,
            "actions": actions,
        });
        let response: YtMusicPlaylistEditResponse = self
            .make_request("browse/edit_playlist", &body, None)
            .await?;
        if !response.success() {
            return Err(eyre!("Error adding song to playlist"));
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
        let mut actions = vec![];
        for song in songs.iter() {
            let action = json!({
                "setVideoId": song.sid.as_ref().ok_or(eyre!("Song setVideoId not found"))?,
                "removedVideoId": song.id,
                "action": "ACTION_REMOVE_VIDEO",
            });
            actions.push(action);
        }
        let body = json!({
            "playlistId": playlist.id,
            "actions": actions,
        });
        let response = self
            .make_request::<YtMusicPlaylistEditResponse>("browse/edit_playlist", &body, None)
            .await?;
        if response.success() {
            Ok(())
        } else {
            Err(eyre!("Error removing song from playlist"))
        }
    }

    async fn delete_playlist(&self, playlist: Playlist) -> Result<()> {
        let body = json!({
            "playlistId": playlist.id,
        });
        self.make_request::<YtMusicPlaylistDeleteResponse>("playlist/delete", &body, None)
            .await?;
        Ok(())
    }

    async fn search_song(&self, song: &Song) -> Result<Option<Song>> {
        let mut queries = song.build_queries();

        let ignore_spelling = "AUICCAFqDBAOEAoQAxAEEAkQBQ%3D%3D";
        let params = format!("EgWKAQI{}{}", "I", ignore_spelling);

        while let Some(query) = queries.pop() {
            let body = json!({
                "query": query,
                "params": params,
            });
            let response = self
                .make_request::<YtMusicResponse>("search", &body, None)
                .await?;
            let res_songs: SearchSongs = response.try_into()?;
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

#[cfg(test)]
mod tests {
    #[tokio::test]
    async fn test_ytmusic_to_spotify() {}
}
