use serde::Deserialize;

#[derive(Deserialize, Debug)]
pub struct SpotifyToken {
    pub access_token: String,
    pub token_type: String,
    pub refresh_token: String,
    pub expires_in: u32,
}

#[derive(Deserialize, Debug)]
pub struct SpotifyUser {
    pub id: String,
    pub display_name: String,
    pub email: String,
}

#[derive(Deserialize, Debug)]
pub struct SpotifySnapshotResponse {
    pub snapshot_id: String,
}

#[derive(Deserialize, Debug)]
pub struct SpotifySearchResponse {
    pub tracks: SpotifyPageResponse<SpotifySongResponse>,
}

#[derive(Deserialize, Debug)]
pub struct SpotifyPageResponse<T> {
    pub items: Vec<T>,
    pub total: u32,
}
impl<T> SpotifyPageResponse<T> {
    pub fn merge(&mut self, other: &mut Self) {
        self.items.append(&mut other.items);
    }
}

#[derive(Deserialize, Debug)]
pub struct SpotifyPlaylistResponse {
    pub id: String,
    pub name: String,
    pub public: bool,
}

#[derive(Deserialize, Debug)]
pub struct SpotifySongItemResponse {
    pub track: SpotifySongResponse,
}

#[derive(Deserialize, Debug)]
pub struct SpotifySongResponse {
    pub id: String,
    pub name: String,
    pub artists: Vec<SpotifyArtistResponse>,
    pub album: SpotifyAlbumResponse,
}

#[derive(Deserialize, Debug)]
pub struct SpotifyArtistResponse {
    pub id: String,
    pub name: String,
}

#[derive(Deserialize, Debug)]
pub struct SpotifyAlbumResponse {
    pub id: String,
    pub name: String,
}
