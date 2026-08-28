use std::path::{Path, PathBuf};

use sync_dis_boi::merge::merge;
use sync_dis_boi::music_api::{MusicApiType, Playlist, Song};

use crate::common::util::temp_dir;

fn song(source: MusicApiType, id: &str, isrc: Option<&str>, name: &str) -> Song {
    Song {
        source,
        id: id.to_string(),
        sid: None,
        isrc: isrc.map(str::to_string),
        name: name.to_string(),
        album: None,
        artists: vec![],
        duration_ms: 200_000,
    }
}

fn playlist(name: &str, songs: Vec<Song>) -> Playlist {
    Playlist {
        id: format!("{name}-id"),
        name: name.to_string(),
        songs,
    }
}

fn write_json(dir: &Path, file: &str, playlists: &[Playlist]) -> PathBuf {
    let path = dir.join(file);
    serde_json::to_writer(std::fs::File::create(&path).unwrap(), playlists).unwrap();
    path
}

fn read_json(path: &Path) -> Vec<Playlist> {
    serde_json::from_reader(std::fs::File::open(path).unwrap()).unwrap()
}

#[test]
fn merge_unions_dedups_and_keeps_the_priority_copy() {
    let dir = temp_dir("merge");

    // Priority input (ISRC-rich, e.g. Tidal).
    let primary = write_json(
        &dir,
        "primary.json",
        &[
            playlist(
                "Rock",
                vec![
                    song(MusicApiType::Tidal, "t1", Some("AAAAA1111111"), "Song A"),
                    song(MusicApiType::Tidal, "t2", Some("BBBBB2222222"), "Song B"),
                ],
            ),
            // Auto playlist that must be skipped.
            playlist(
                "Discover Mix",
                vec![song(
                    MusicApiType::Tidal,
                    "t9",
                    Some("ZZZZZ9999999"),
                    "Noise",
                )],
            ),
        ],
    );

    // Secondary input (e.g. Spotify): re-supplies Song A under a different
    // id/source but same ISRC (must dedup, primary copy wins), plus a new
    // catalog-gap song C.
    let secondary = write_json(
        &dir,
        "secondary.json",
        &[playlist(
            "Rock",
            vec![
                song(MusicApiType::Spotify, "s1", Some("AAAAA1111111"), "Song A"),
                song(MusicApiType::Spotify, "s3", Some("CCCCC3333333"), "Song C"),
            ],
        )],
    );

    let out = dir.join("canon.json");
    merge(&[primary, secondary], &out, true).unwrap();
    let canon = read_json(&out);

    // "Discover Mix" skipped -> only the "Rock" playlist remains.
    assert_eq!(canon.len(), 1);
    let rock = &canon[0];
    assert_eq!(rock.name, "Rock");

    // A (deduped), B, C -> 3 unique songs.
    assert_eq!(rock.songs.len(), 3);

    // The kept copy of Song A is the higher-priority (Tidal) one.
    let a = rock
        .songs
        .iter()
        .find(|s| s.isrc.as_deref() == Some("AAAAA1111111"))
        .unwrap();
    assert_eq!(a.source, MusicApiType::Tidal);
    assert_eq!(a.id, "t1");

    let names: Vec<&str> = rock.songs.iter().map(|s| s.name.as_str()).collect();
    assert_eq!(names, ["Song A", "Song B", "Song C"]);

    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn merge_output_is_order_independent() {
    let dir = temp_dir("merge-order");

    // Same songs, shuffled order in the two inputs / playlists.
    let songs_a = vec![
        song(MusicApiType::Tidal, "t2", Some("BBBBB2222222"), "Beta"),
        song(MusicApiType::Tidal, "t1", Some("AAAAA1111111"), "Alpha"),
        song(MusicApiType::Tidal, "t3", Some("CCCCC3333333"), "Gamma"),
    ];
    let mut songs_b = songs_a.clone();
    songs_b.reverse();

    let in_a = write_json(&dir, "a.json", &[playlist("Mix", songs_a)]);
    let in_b = write_json(&dir, "b.json", &[playlist("Mix", songs_b)]);

    let out_a = dir.join("canon_a.json");
    let out_b = dir.join("canon_b.json");
    merge(&[in_a], &out_a, true).unwrap();
    merge(&[in_b], &out_b, true).unwrap();

    assert_eq!(
        std::fs::read_to_string(&out_a).unwrap(),
        std::fs::read_to_string(&out_b).unwrap(),
    );

    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn merge_of_a_missing_file_fails() {
    let dir = temp_dir("merge-missing");
    let res = merge(&[dir.join("nope.json")], &dir.join("out.json"), false);
    assert!(res.is_err());
    std::fs::remove_dir_all(&dir).ok();
}
