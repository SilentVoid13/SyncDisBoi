use sync_dis_boi::music_api::{MusicApiType, Song};
use sync_dis_boi::utils::{clean_enclosure, clean_isrc, dedup_songs, generic_name_clean};

#[test]
fn clean_enclosure_removes_nested_groups() {
    let name =
        "POP/STARS (feat. (G)I-DLE, Madison Beer, Jaira Burns & League ((A)) of Legends) test";
    assert_eq!(clean_enclosure(name, '(', ')'), "POP/STARS  test");

    let name = "test (feat. test) test (feat. test2)";
    assert_eq!(clean_enclosure(name, '(', ')'), "test  test");
}

#[test]
fn clean_enclosure_leaves_names_without_the_tag() {
    assert_eq!(clean_enclosure("plain name", '(', ')'), "plain name");
}

#[test]
fn generic_name_clean_normalises() {
    let cases = [
        ("Don't Stop", "dont stop"),
        ("Title: Subtitle", "title  subtitle"),
        ("Café Résumé à", "cafe resume a"),
        ("100% Pure", "100 pure"),
        ("Song (Live) [2011]", "song"),
        // "part" markers are meaningful: keep them, without the brackets
        ("Echoes (Part 2)", "echoes part 2"),
        ("Echoes (Part Two)", "echoes part two"),
    ];
    for (name, clean) in cases {
        assert_eq!(generic_name_clean(name), clean, "cleaning {name:?}");
    }
}

#[test]
fn clean_isrc_normalises_valid_codes() {
    assert_eq!(
        clean_isrc(Some(" us-aaa-00-00001 ".to_string())),
        Some("USAAA0000001".to_string())
    );
    assert_eq!(
        clean_isrc(Some("USAAA0000001".to_string())),
        Some("USAAA0000001".to_string())
    );
}

#[test]
fn clean_isrc_rejects_invalid_codes() {
    assert_eq!(clean_isrc(Some("TOOSHORT".to_string())), None);
    assert_eq!(clean_isrc(Some("USAAA00000011".to_string())), None);
    assert_eq!(clean_isrc(None), None);
}

fn song(id: &str) -> Song {
    Song {
        source: MusicApiType::Spotify,
        id: id.to_string(),
        sid: None,
        isrc: None,
        name: id.to_string(),
        album: None,
        artists: vec![],
        duration_ms: 0,
    }
}

#[test]
fn dedup_songs_keeps_first_occurrence_in_order() {
    let mut songs = vec![song("a"), song("b"), song("a"), song("c"), song("b")];
    assert!(dedup_songs(&mut songs));
    let ids: Vec<&str> = songs.iter().map(|s| s.id.as_str()).collect();
    assert_eq!(ids, ["a", "b", "c"]);
}

#[test]
fn dedup_songs_reports_when_nothing_changed() {
    let mut songs = vec![song("a"), song("b")];
    assert!(!dedup_songs(&mut songs));
    assert_eq!(songs.len(), 2);
    assert!(!dedup_songs(&mut vec![]));
}
