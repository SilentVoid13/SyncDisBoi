//! The contract suite against the real platforms, plus a sync between every
//! pair of them. These create (and delete) playlists and like (then unlike)
//! songs on the configured accounts, so they are `#[ignore]`d by default:
//!
//! ```sh
//! cargo test --test live -- --ignored               # everything
//! cargo test --test live -- --ignored tidal::       # one platform
//! cargo test --test live -- --ignored cross::       # the sync matrix
//! ```
//!
//! See `tests/common/live.rs` for where credentials come from. Tests on the
//! same platform run one at a time; different platforms run in parallel.

#[path = "../common/mod.rs"]
mod common;

use sync_dis_boi::music_api::MusicApiType;

use crate::common::live;

mod spotify {
    use super::*;
    crate::contract_tests!(|t| live::run(&MusicApiType::Spotify, t); #[ignore = "live"]);
}

mod tidal {
    use super::*;
    crate::contract_tests!(|t| live::run(&MusicApiType::Tidal, t); #[ignore = "live"]);
}

mod yt_music {
    use super::*;
    crate::contract_tests!(|t| live::run(&MusicApiType::YtMusic, t); #[ignore = "live"]);
}

mod listenbrainz {
    use super::*;
    crate::contract_tests!(|t| live::run(&MusicApiType::ListenBrainz, t); #[ignore = "live"]);
}

mod cross {
    use super::*;
    crate::cross_tests!(#[ignore = "live"] live::run_pair;
        spotify_to_tidal: Spotify => Tidal,
        spotify_to_yt_music: Spotify => YtMusic,
        spotify_to_listenbrainz: Spotify => ListenBrainz,
        tidal_to_spotify: Tidal => Spotify,
        tidal_to_yt_music: Tidal => YtMusic,
        tidal_to_listenbrainz: Tidal => ListenBrainz,
        yt_music_to_spotify: YtMusic => Spotify,
        yt_music_to_tidal: YtMusic => Tidal,
        yt_music_to_listenbrainz: YtMusic => ListenBrainz,
        listenbrainz_to_spotify: ListenBrainz => Spotify,
        listenbrainz_to_tidal: ListenBrainz => Tidal,
        listenbrainz_to_yt_music: ListenBrainz => YtMusic,
    );
}
