use sync_dis_boi::music_api::{Album, Artist, MusicApiType, Song};

/// A real song, available on every platform, that tests search for and put in
/// playlists. Metadata is how Tidal names it (the album release, not a
/// compilation), so name-based searches have an unambiguous target.
pub struct Fixture {
    pub isrc: &'static str,
    pub name: &'static str,
    pub artists: &'static [&'static str],
    pub album: &'static str,
    pub duration_ms: usize,
}

pub const FIXTURES: [Fixture; 5] = [
    Fixture {
        isrc: "GBAYE9200070",
        name: "Creep",
        artists: &["Radiohead"],
        album: "Pablo Honey",
        duration_ms: 239_000,
    },
    Fixture {
        isrc: "GBAHT0500600",
        name: "Knights of Cydonia",
        artists: &["Muse"],
        album: "Black Holes and Revelations",
        duration_ms: 367_000,
    },
    Fixture {
        isrc: "USVT10300001",
        name: "Seven Nation Army",
        artists: &["The White Stripes"],
        album: "Elephant",
        duration_ms: 232_000,
    },
    Fixture {
        isrc: "USGF19942501",
        name: "Smells Like Teen Spirit",
        artists: &["Nirvana"],
        album: "Nevermind",
        duration_ms: 301_000,
    },
    Fixture {
        isrc: "USQX91300108",
        name: "Get Lucky",
        artists: &["Daft Punk", "Pharrell Williams", "Nile Rodgers"],
        album: "Random Access Memories",
        duration_ms: 370_000,
    },
];

impl Fixture {
    /// The song as a `source` platform would describe it.
    pub fn song(&self, source: MusicApiType) -> Song {
        Song {
            source,
            id: format!("fixture-{}", self.isrc),
            sid: None,
            isrc: Some(self.isrc.to_string()),
            name: self.name.to_string(),
            album: Some(Album {
                id: None,
                name: self.album.to_string(),
                upc: None,
            }),
            artists: self
                .artists
                .iter()
                .map(|a| Artist {
                    id: None,
                    name: (*a).to_string(),
                })
                .collect(),
            duration_ms: self.duration_ms,
        }
    }

    pub fn song_without_isrc(&self, source: MusicApiType) -> Song {
        Song {
            isrc: None,
            ..self.song(source)
        }
    }
}

/// The platform that search probes pretend to come from. It must differ from
/// the platform under test (same-platform songs compare by id only), and have
/// durations, so matching is checked as strictly as possible.
pub fn probe_source(target: &MusicApiType) -> MusicApiType {
    if *target == MusicApiType::Tidal {
        MusicApiType::Spotify
    } else {
        MusicApiType::Tidal
    }
}

/// Probe songs for the first `n` fixtures, as seen from outside `target`.
pub fn probes(target: &MusicApiType, n: usize) -> Vec<Song> {
    FIXTURES[..n]
        .iter()
        .map(|f| f.song(probe_source(target)))
        .collect()
}

/// A song that exists nowhere, with a syntactically valid ISRC.
pub fn unknown_song(source: MusicApiType, with_isrc: bool) -> Song {
    Song {
        source,
        id: "fixture-unknown".to_string(),
        sid: None,
        isrc: with_isrc.then(|| "ZZZ999999999".to_string()),
        name: "Qvxzq Wjxkp Plorth".to_string(),
        album: Some(Album {
            id: None,
            name: "Zvbnq Oxtrqw Yyqk".to_string(),
            upc: None,
        }),
        artists: vec![Artist {
            id: None,
            name: "Xqzvw Jrrbtk".to_string(),
        }],
        duration_ms: 123_000,
    }
}

/// A song with neither album nor ISRC, which sync must skip (it is how
/// `YouTube` videos look).
pub fn albumless_song(source: MusicApiType) -> Song {
    Song {
        album: None,
        ..FIXTURES[4].song_without_isrc(source)
    }
}

/// Platforms that always return ISRCs when reading playlists and likes.
/// `ListenBrainz` returns one only when `MusicBrainz` knows exactly one.
pub fn exposes_isrc(kind: &MusicApiType) -> bool {
    matches!(kind, MusicApiType::Spotify | MusicApiType::Tidal)
}
