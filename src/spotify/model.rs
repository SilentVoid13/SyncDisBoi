use serde::Deserialize;

#[derive(Deserialize, Debug)]
pub struct SpotifyEmptyResponse {}

#[derive(Deserialize, Debug)]
#[allow(dead_code)]
pub struct SpotifyUserResponse {
    pub country: String,
    pub display_name: String,
    pub email: String,
}

#[derive(Deserialize, Debug)]
#[allow(dead_code)]
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
    pub next: Option<String>,
}
impl<T> SpotifyPageResponse<T> {
    pub fn merge(&mut self, other: Self) {
        self.items.extend(other.items);
        self.total += other.total;
        self.next = other.next;
    }
}

#[derive(Deserialize, Debug)]
pub struct SpotifyPlaylistResponse {
    pub id: String,
    pub name: String,
    #[allow(dead_code)]
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
    pub duration_ms: usize,
    pub artists: Vec<SpotifyArtistResponse>,
    pub album: SpotifyAlbumResponse,
    pub external_ids: SpotifyExternalIdsResponse,
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

#[derive(Deserialize, Debug)]
pub struct SpotifyExternalIdsResponse {
    pub isrc: String,
    #[allow(dead_code)]
    pub upc: Option<String>,
}
