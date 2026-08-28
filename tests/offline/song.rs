use sync_dis_boi::music_api::{Album, Artist, MusicApiType, Song};

fn song(source: MusicApiType, name: &str, duration_ms: usize) -> Song {
    Song {
        source,
        id: format!("{name}-{duration_ms}"),
        sid: None,
        isrc: None,
        name: name.to_string(),
        album: None,
        artists: vec![],
        duration_ms,
    }
}

fn with_album(mut song: Song, album: &str) -> Song {
    song.album = Some(Album {
        id: None,
        name: album.to_string(),
        upc: None,
    });
    song
}

fn with_isrc(mut song: Song, isrc: &str) -> Song {
    song.isrc = Some(isrc.to_string());
    song
}

#[test]
fn compare_same_source_uses_id_only() {
    let mut a = song(MusicApiType::ListenBrainz, "a", 0);
    let mut b = song(MusicApiType::ListenBrainz, "b", 0);
    assert!(!a.compare(&b));
    b.id.clone_from(&a.id);
    assert!(a.compare(&b));
    a.name = "totally different".to_string();
    assert!(a.compare(&b));
}

#[test]
fn compare_uses_isrc_when_both_songs_have_one() {
    let a = with_isrc(song(MusicApiType::Spotify, "Song", 200_000), "USAAA0000001");
    let same_meta = with_isrc(song(MusicApiType::Tidal, "Song", 200_000), "USAAA0000002");
    let other_meta = with_isrc(song(MusicApiType::Tidal, "Other", 1_000), "USAAA0000001");
    assert!(!a.compare(&same_meta));
    assert!(a.compare(&other_meta));
}

#[test]
fn compare_falls_back_to_metadata_when_an_isrc_is_missing() {
    let a = with_isrc(song(MusicApiType::Spotify, "Song", 200_000), "USAAA0000001");
    let b = song(MusicApiType::YtMusic, "Song", 200_000);
    assert!(a.compare(&b));
    assert!(b.compare(&a));
}

#[test]
fn compare_rejects_different_names() {
    let a = song(MusicApiType::Spotify, "Creep", 239_000);
    let b = song(MusicApiType::Tidal, "Karma Police", 239_000);
    assert!(!a.compare(&b));
}

#[test]
fn compare_ignores_name_suffixes() {
    let a = song(MusicApiType::Spotify, "Song - Remastered 2011", 200_000);
    let b = song(MusicApiType::Tidal, "Song (Remastered 2011)", 200_000);
    let c = song(MusicApiType::YtMusic, "Song feat. Someone", 200_000);
    assert!(a.compare(&b));
    assert!(a.compare(&c));
}

#[test]
fn compare_allows_two_seconds_of_duration_difference() {
    let a = song(MusicApiType::Spotify, "Song", 200_000);
    assert!(a.compare(&song(MusicApiType::Tidal, "Song", 202_999)));
    assert!(a.compare(&song(MusicApiType::Tidal, "Song", 198_000)));
    assert!(!a.compare(&song(MusicApiType::Tidal, "Song", 203_000)));
    assert!(!a.compare(&song(MusicApiType::Tidal, "Song", 197_000)));
}

#[test]
fn compare_zero_duration_does_not_panic() {
    // a sub-second duration used to underflow `usize`
    let zero = song(MusicApiType::YtMusic, "test", 0);
    let normal = song(MusicApiType::Spotify, "test", 200_000);
    assert!(!zero.compare(&normal));
    assert!(!normal.compare(&zero));
}

#[test]
fn compare_skips_duration_for_platforms_without_it() {
    let lb = song(MusicApiType::ListenBrainz, "test song", 0);
    let spotify = song(MusicApiType::Spotify, "test song", 200_000);
    assert!(lb.compare(&spotify));
    assert!(spotify.compare(&lb));
}

#[test]
fn compare_checks_album_names() {
    let a = with_album(song(MusicApiType::Spotify, "Song", 200_000), "First Album");
    let b = with_album(song(MusicApiType::Tidal, "Song", 200_000), "Other Record");
    let c = with_album(
        song(MusicApiType::Tidal, "Song", 200_000),
        "First Album (Deluxe)",
    );
    assert!(!a.compare(&b));
    assert!(a.compare(&c));
}

#[test]
fn compare_skips_album_check_for_singles() {
    // YtMusic often maps an album track to its single
    let album = with_album(song(MusicApiType::Spotify, "Song", 200_000), "Some Album");
    let single = with_album(song(MusicApiType::YtMusic, "Song", 200_000), "Song");
    assert!(single.is_single());
    assert!(!album.is_single());
    assert!(album.compare(&single));
}

#[test]
fn clean_name_strips_decorations() {
    let cases = [
        ("Bohemian Rhapsody - Remastered 2011", "bohemian rhapsody"),
        ("Get Lucky (feat. Pharrell Williams)", "get lucky"),
        ("Get Lucky feat. Pharrell Williams", "get lucky"),
        ("Song [Live]", "song"),
        ("Don't Stop Me Now", "dont stop me now"),
    ];
    for (name, clean) in cases {
        assert_eq!(song(MusicApiType::Spotify, name, 0).clean_name(), clean);
    }
}

#[test]
fn build_queries_go_from_broad_to_specific() {
    let mut s = with_album(song(MusicApiType::Spotify, "Song", 0), "Album");
    s.artists = vec![
        Artist {
            id: None,
            name: "First".to_string(),
        },
        Artist {
            id: None,
            name: "Second".to_string(),
        },
    ];
    // searches pop from the back, so the most specific query runs first
    assert_eq!(
        s.build_queries(),
        [
            "song album",
            "song second",
            "song first",
            "song second album",
            "song first album",
        ]
    );
}

#[test]
fn build_queries_without_album_use_artists_only() {
    let mut s = song(MusicApiType::Spotify, "Song", 0);
    s.artists = vec![Artist {
        id: None,
        name: "Artist".to_string(),
    }];
    assert_eq!(s.build_queries(), ["song artist"]);
}

#[test]
fn song_json_without_album_upc_still_parses() {
    // exports written before `upc` existed must keep importing
    let json = r#"{
        "source": "Tidal", "id": "1", "sid": null, "isrc": "USAAA0000001",
        "name": "Song", "album": {"id": null, "name": "Album"},
        "artists": [{"id": null, "name": "Artist"}], "duration_ms": 1000
    }"#;
    let song: Song = serde_json::from_str(json).unwrap();
    assert_eq!(song.album.unwrap().upc, None);
}

#[test]
fn display_shows_name_album_and_artists() {
    let mut s = with_album(song(MusicApiType::Spotify, "Song", 0), "Album");
    s.artists = vec![Artist {
        id: None,
        name: "Artist".to_string(),
    }];
    assert_eq!(s.to_string(), "Song (Album) - Artist");
}
