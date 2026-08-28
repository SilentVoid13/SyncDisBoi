use serde::Deserialize;

/// `GET /1/validate-token`
#[derive(Deserialize, Debug)]
#[allow(dead_code)]
pub struct ListenBrainzTokenResponse {
    pub valid: bool,
    pub user_name: Option<String>,
    pub message: String,
}

#[derive(Deserialize, Debug)]
#[allow(dead_code)]
pub struct ListenBrainzStatusResponse {
    pub status: Option<String>,
}

/// `POST /1/playlist/create`
#[derive(Deserialize, Debug)]
#[allow(dead_code)]
pub struct ListenBrainzCreateResponse {
    pub status: Option<String>,
    pub playlist_mbid: String,
}

/// `GET /1/user/{user_name}/playlists`
#[derive(Deserialize, Debug)]
#[allow(dead_code)]
pub struct ListenBrainzPlaylistsResponse {
    #[serde(default)]
    pub playlists: Vec<ListenBrainzPlaylistWrapper>,
    #[serde(default)]
    pub playlist_count: usize,
    #[serde(default)]
    pub offset: usize,
    #[serde(default)]
    pub count: usize,
}

/// Every playlist comes double-wrapped: `serialize_playlists` calls
/// `serialize_jspf()`, which itself returns a `{"playlist": ...}` object.
#[derive(Deserialize, Debug)]
pub struct ListenBrainzPlaylistWrapper {
    pub playlist: ListenBrainzPlaylistResponse,
}

#[derive(Deserialize, Debug)]
#[allow(dead_code)]
pub struct ListenBrainzPlaylistResponse {
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub identifier: String,
    #[serde(default)]
    pub track: Vec<ListenBrainzTrackResponse>,
    pub annotation: Option<String>,
    pub creator: Option<String>,
}

#[derive(Deserialize, Debug)]
#[allow(dead_code)]
pub struct ListenBrainzTrackResponse {
    #[serde(default)]
    pub identifier: ListenBrainzIdentifier,
    /// The combined artist credit, e.g. `Daft Punk feat. Pharrell Williams`.
    pub creator: Option<String>,
    pub album: Option<String>,
    pub title: Option<String>,
    pub duration: Option<usize>,
    pub extension: Option<ListenBrainzTrackExtension>,
}

/// A track `identifier` is an array of URIs, but `ListenBrainz` still accepts
/// (and older playlists still carry) a bare string, so both must parse.
#[derive(Deserialize, Debug)]
#[serde(untagged)]
pub enum ListenBrainzIdentifier {
    One(String),
    Many(Vec<String>),
}

impl Default for ListenBrainzIdentifier {
    fn default() -> Self {
        ListenBrainzIdentifier::Many(Vec::new())
    }
}

impl ListenBrainzIdentifier {
    /// The first identifier with the given prefix, stripped of it.
    pub fn first_stripped(&self, prefix: &str) -> Option<&str> {
        match self {
            ListenBrainzIdentifier::One(s) => s.strip_prefix(prefix),
            ListenBrainzIdentifier::Many(v) => {
                v.iter().find_map(|s| s.strip_prefix(prefix))
            }
        }
    }
}

#[derive(Deserialize, Debug)]
pub struct ListenBrainzTrackExtension {
    #[serde(rename = "https://musicbrainz.org/doc/jspf#track")]
    pub track: Option<ListenBrainzTrackExtensionInner>,
}

#[derive(Deserialize, Debug)]
#[allow(dead_code)]
pub struct ListenBrainzTrackExtensionInner {
    #[serde(default)]
    pub artist_identifiers: Vec<String>,
    pub release_identifier: Option<String>,
    pub added_by: Option<String>,
    pub added_at: Option<String>,
}

/// One entry of the `POST /1/metadata/lookup/` response array. Only matched
/// entries carry the `MusicBrainz` fields.
#[derive(Deserialize, Debug)]
#[allow(dead_code)]
pub struct ListenBrainzLookupResponse {
    #[serde(default)]
    pub index: usize,
    pub recording_mbid: Option<String>,
    pub recording_name: Option<String>,
    pub release_mbid: Option<String>,
    pub release_name: Option<String>,
    pub artist_credit_name: Option<String>,
    #[serde(default)]
    pub artist_mbids: Vec<String>,
    pub artist_name_arg: Option<String>,
    pub recording_name_arg: Option<String>,
}

/// `GET /1/feedback/user/{user_name}/get-feedback`
#[derive(Deserialize, Debug)]
#[allow(dead_code)]
pub struct ListenBrainzFeedbackResponse {
    #[serde(default)]
    pub feedback: Vec<ListenBrainzFeedbackItem>,
    #[serde(default)]
    pub count: usize,
    #[serde(default)]
    pub total_count: usize,
    #[serde(default)]
    pub offset: usize,
}

#[derive(Deserialize, Debug)]
#[allow(dead_code)]
pub struct ListenBrainzFeedbackItem {
    /// `null` for feedback recorded against an MSID only.
    pub recording_mbid: Option<String>,
    #[serde(default)]
    pub score: i32,
    pub track_metadata: Option<ListenBrainzTrackMetadata>,
}

#[derive(Deserialize, Debug)]
#[allow(dead_code)]
pub struct ListenBrainzTrackMetadata {
    #[serde(default)]
    pub track_name: String,
    #[serde(default)]
    pub artist_name: String,
    pub release_name: Option<String>,
    pub mbid_mapping: Option<ListenBrainzMbidMapping>,
}

#[derive(Deserialize, Debug)]
#[allow(dead_code)]
pub struct ListenBrainzMbidMapping {
    pub recording_mbid: Option<String>,
    pub release_mbid: Option<String>,
    #[serde(default)]
    pub artist_mbids: Vec<String>,
}

// ListenBrainz exposes no ISRC lookup, so ISRC resolution goes to MusicBrainz.

/// `GET /ws/2/recording?query=isrc:...`
#[derive(Deserialize, Debug)]
#[allow(dead_code)]
pub struct MusicBrainzSearchResponse {
    #[serde(default)]
    pub count: usize,
    #[serde(default)]
    pub recordings: Vec<MusicBrainzRecording>,
}

#[derive(Deserialize, Debug)]
#[allow(dead_code)]
pub struct MusicBrainzRecording {
    pub id: String,
    #[serde(default)]
    pub score: u32,
    pub title: Option<String>,
    /// Milliseconds.
    pub length: Option<usize>,
    #[serde(default)]
    pub isrcs: Vec<String>,
    #[serde(rename = "artist-credit", default)]
    pub artist_credit: Vec<MusicBrainzArtistCredit>,
    #[serde(default)]
    pub releases: Vec<MusicBrainzRelease>,
}

#[derive(Deserialize, Debug)]
#[allow(dead_code)]
pub struct MusicBrainzArtistCredit {
    #[serde(default)]
    pub name: String,
    /// Text joining this artist to the next one, e.g. `" feat. "`.
    #[serde(default)]
    pub joinphrase: String,
    pub artist: Option<MusicBrainzArtist>,
}

#[derive(Deserialize, Debug)]
#[allow(dead_code)]
pub struct MusicBrainzArtist {
    pub id: String,
    #[serde(default)]
    pub name: String,
}

#[derive(Deserialize, Debug)]
#[allow(dead_code)]
pub struct MusicBrainzRelease {
    pub id: String,
    #[serde(default)]
    pub title: String,
    /// UPC/EAN. Absent when the release is embedded in a recording result.
    pub barcode: Option<String>,
}

/// `GET /ws/2/release?query=barcode:...`
#[derive(Deserialize, Debug)]
#[allow(dead_code)]
pub struct MusicBrainzReleaseSearchResponse {
    #[serde(default)]
    pub count: usize,
    #[serde(default)]
    pub releases: Vec<MusicBrainzRelease>,
}

impl MusicBrainzRecording {
    /// The full artist credit including join phrases (`"A feat. B"`).
    pub fn credit_string(&self) -> String {
        let mut out = String::new();
        for (i, credit) in self.artist_credit.iter().enumerate() {
            out.push_str(&credit.name);
            if i + 1 < self.artist_credit.len() {
                out.push_str(&credit.joinphrase);
            }
        }
        out
    }
}
