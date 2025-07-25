use serde::Deserialize;

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
#[allow(dead_code)]
pub struct TidalOAuthDeviceRes {
    pub user_code: String,
    pub device_code: String,
    pub verification_uri: String,
    pub verification_uri_complete: String,
    pub expires_in: u32,
}

////////////
// V1 API //
////////////

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct TidalPageResponse<T> {
    pub items: Vec<T>,
    pub offset: usize,
    pub total_number_of_items: usize,
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
    pub isrc: String,
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

////////////
// V2 API //
////////////

#[derive(Deserialize, Debug)]
pub struct TidalPlaylistCreateResponse {
    #[allow(dead_code)]
    pub trn: String,
    pub data: TidalPlaylistDataResponse,
}

#[derive(Deserialize, Debug)]
pub struct TidalPlaylistDataResponse {
    pub uuid: String,
}

#[derive(Deserialize, Debug)]
pub struct TidalMediaResponseSingle {
    pub data: TidalMediaData,
    #[allow(dead_code)]
    pub included: Option<Vec<TidalMediaData>>,
}

#[derive(Deserialize, Debug)]
pub struct TidalMediaResponse {
    pub data: Vec<TidalMediaData>,
    pub included: Option<Vec<TidalMediaData>>,
}

#[derive(Deserialize, Debug)]
pub struct TidalMediaData {
    pub id: String,
    pub attributes: TidalMediaAttributes,
    pub relationships: Option<TidalMediaRelationships>,
    #[allow(dead_code)]
    #[serde(rename = "type")]
    pub typ: String,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
#[allow(dead_code)]
pub struct TidalMediaAttributes {
    pub title: Option<String>,
    pub isrc: Option<String>,
    pub name: Option<String>,
    pub duration: Option<String>,
    pub barcode_id: Option<String>,
    pub popularity: Option<f32>,

    // user attributes
    pub username: Option<String>,
    pub country: Option<String>,
    pub email: Option<String>,
}

#[derive(Deserialize, Debug)]
pub struct TidalMediaRelationships {
    pub artists: Option<TidalMediaRelationship>,
    pub albums: Option<TidalMediaRelationship>,
}

#[derive(Deserialize, Debug)]
pub struct TidalMediaRelationship {
    pub data: Option<Vec<TidalMediaRelationshipData>>,
}

#[derive(Deserialize, Debug)]
pub struct TidalMediaRelationshipData {
    pub id: String,
    #[allow(dead_code)]
    #[serde(rename = "type")]
    pub typ: String,
}
