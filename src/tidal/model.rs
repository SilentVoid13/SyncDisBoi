use serde::{Deserialize, Serialize};

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct TidalDeviceRes {
    pub user_code: String,
    pub device_code: String,
    pub verification_uri: String,
    pub verification_uri_complete: String,
    pub expires_in: u32,
}

#[derive(Serialize, Debug)]
pub struct TidalReqToken {
    pub client_id: String,
    pub device_code: String,
    pub grant_type: String,
    pub scope: String,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct TidalOAuthToken {
    pub scope: String,
    pub token_type: String,
    pub access_token: String,
    pub refresh_token: String,
    pub expires_in: u64,
}

#[derive(Deserialize, Debug)]
pub struct TidalOAuthRefresh {
    pub access_token: String,
    pub expires_in: u64,
    pub scope: String,
    pub token_type: String,
}

#[derive(Deserialize, Debug)]
pub struct TidalPlaylistCreateResponse {
    pub trn: String,
    pub data: TidalDataResponse,
}

#[derive(Deserialize, Debug)]
pub struct TidalDataResponse {
    pub uuid: String,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct TidalMeResponse {
    pub user_id: usize,
}

#[derive(Deserialize, Debug)]
pub struct TidalPageResponse<T> {
    pub items: Vec<T>,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct TidalPlaylistResponse {
    pub uuid: String,
    pub title: String,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct TidalSongItemResponse {
    pub item: TidalSongResponse,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct TidalSongResponse {
    pub id: usize,
    pub title: String,
    pub duration: usize,
    pub artists: Vec<TidalArtistResponse>,
    pub album: TidalAlbumResponse,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct TidalAlbumResponse {
    pub id: usize,
    pub title: String,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct TidalArtistResponse {
    pub id: usize,
    pub name: String,
}

#[derive(Deserialize, Debug)]
pub struct TidalSearchResponse {
    pub tracks: TidalPageResponse<TidalSongResponse>,
}
