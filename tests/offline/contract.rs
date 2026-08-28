//! The contract suite against a mock of each platform. This checks the suite
//! itself and the platform-agnostic code it drives; the same tests run against
//! the real platforms in `tests/live`.

use sync_dis_boi::music_api::MusicApiType;

use crate::common::mock;

mod spotify {
    use super::*;
    crate::contract_tests!(|t| mock::run(&MusicApiType::Spotify, t));
}

mod tidal {
    use super::*;
    crate::contract_tests!(|t| mock::run(&MusicApiType::Tidal, t));
}

mod yt_music {
    use super::*;
    crate::contract_tests!(|t| mock::run(&MusicApiType::YtMusic, t));
}

mod listenbrainz {
    use super::*;
    crate::contract_tests!(|t| mock::run(&MusicApiType::ListenBrainz, t));
}

mod cross {
    use super::*;
    crate::cross_tests!(mock::run_pair;
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
