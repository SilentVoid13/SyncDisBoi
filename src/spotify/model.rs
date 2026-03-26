use serde::{Deserialize, Deserializer};
use serde_json::Value;

#[derive(Deserialize, Debug)]
pub struct SpotifyEmptyResponse {}

#[derive(Deserialize, Debug)]
#[allow(dead_code)]
pub struct SpotifyUserResponse {
    /// Omitted on Spotify Web API Development Mode (Feb 2026+); see changelog.
    #[serde(default)]
    pub country: Option<String>,
    pub display_name: Option<String>,
    /// Omitted on Spotify Web API Development Mode (Feb 2026+); see changelog.
    #[serde(default)]
    pub email: Option<String>,
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
pub struct SpotifyPageResponse<T: for<'d> Deserialize<'d>> {
    #[serde(deserialize_with = "deserialize_non_null_vec")]
    pub items: Vec<T>,
    pub total: u32,
    pub next: Option<String>,
}

impl<T: for<'d> Deserialize<'d>> SpotifyPageResponse<T> {
    pub fn merge(&mut self, other: Self) {
        self.items.extend(other.items);
        self.total += other.total;
        self.next = other.next;
    }
}

fn deserialize_non_null_vec<'de, D, T>(deserializer: D) -> Result<Vec<T>, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de>,
{
    let items: Vec<Option<T>> = Vec::deserialize(deserializer)?;
    Ok(items.into_iter().flatten().collect())
}

#[derive(Deserialize, Debug)]
pub struct SpotifyPlaylistResponse {
    pub id: String,
    pub name: String,
    #[allow(dead_code)]
    pub public: Option<bool>,
}

/// One row from `GET /playlists/{id}/items` or `GET /me/tracks`.
/// Feb 2026+: playlist rows use `item` (track or episode); legacy `track` is still accepted.
#[derive(Deserialize, Debug)]
pub struct SpotifySongItemResponse {
    #[serde(default)]
    pub item: Option<Value>,
    #[serde(default)]
    pub track: Option<Value>,
}

#[derive(Deserialize, Debug)]
pub struct SpotifySongResponse {
    // id can be null if the song is a local import
    pub id: Option<String>,
    pub name: String,
    pub duration_ms: usize,
    pub artists: Vec<SpotifyArtistResponse>,
    pub album: SpotifyAlbumResponse,
    /// May be omitted in some API modes (see Spotify changelog).
    #[serde(default)]
    pub external_ids: Option<SpotifyExternalIdsResponse>,
}

#[derive(Deserialize, Debug)]
pub struct SpotifyArtistResponse {
    // id can be null if the song is a local import
    pub id: Option<String>,
    pub name: Option<String>,
}

#[derive(Deserialize, Debug)]
pub struct SpotifyAlbumResponse {
    // id can be null if the song is a local import
    pub id: Option<String>,
    pub name: String,
}

#[derive(Deserialize, Debug)]
pub struct SpotifyExternalIdsResponse {
    // isrc can be null if the song is now deleted/unavailable
    pub isrc: Option<String>,
    #[allow(dead_code)]
    pub upc: Option<String>,
}
