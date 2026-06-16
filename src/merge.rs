use std::path::{Path, PathBuf};

use color_eyre::eyre::{Result, eyre};
use tracing::{info, warn};

use crate::music_api::{Playlist, Song};
use crate::sync::SKIPPED_PLAYLISTS;

/// Deterministic, content-based ordering key for a song.
fn song_sort_key(song: &Song) -> (String, String, String, String) {
    let artist = song
        .artists
        .first()
        .map(|a| a.name.to_lowercase())
        .unwrap_or_default();
    (
        artist,
        song.name.to_lowercase(),
        song.isrc.clone().unwrap_or_default(),
        song.id.clone(),
    )
}

/// Merge several canonical playlist exports into a single platform-agnostic
/// canonical file.
///
/// Inputs are processed in the given order, which acts as a **priority order**:
/// when the same song appears in more than one input (matched via
/// [`crate::music_api::Song::compare`], i.e. ISRC first then fuzzy), the copy
/// from the *earlier* input is kept. Listing ISRC-rich platforms first (e.g.
/// Tidal, Spotify) therefore guarantees the canonical copy keeps its ISRC and
/// cleaner metadata.
///
/// Playlists are unioned by name; auto-generated / personal playlists listed in
/// [`SKIPPED_PLAYLISTS`] are ignored. Deletions are not propagated: the merge is
/// purely additive, consistent with the rest of `SyncDisBoi`.
pub fn merge(inputs: &[PathBuf], output: &Path, minify: bool) -> Result<()> {
    if inputs.is_empty() {
        return Err(eyre!("no input files provided to merge"));
    }

    let mut merged: Vec<Playlist> = Vec::new();

    for input in inputs {
        info!("ingesting {:?} ...", input);
        let playlists: Vec<Playlist> = serde_json::from_reader(std::fs::File::open(input)?)?;

        let mut added = 0;
        let mut deduped = 0;
        for playlist in playlists {
            if SKIPPED_PLAYLISTS.contains(&playlist.name.as_str()) {
                continue;
            }

            // Find the matching canonical playlist, or create it.
            let idx = if let Some(i) = merged.iter().position(|p| p.name == playlist.name) {
                i
            } else {
                merged.push(Playlist {
                    id: playlist.id,
                    name: playlist.name,
                    songs: Vec::new(),
                });
                merged.len() - 1
            };

            for song in playlist.songs {
                // `==` uses `Song::compare` (ISRC first, then fuzzy). The earlier
                // input already present wins, so we just skip anything matched.
                if merged[idx].songs.iter().any(|s| s == &song) {
                    deduped += 1;
                    continue;
                }
                merged[idx].songs.push(song);
                added += 1;
            }
        }
        info!(
            "  -> {} new songs, {} already covered by a higher-priority input",
            added, deduped
        );
    }

    // canonicalize ordering so the file is stable across runs
    merged.sort_by(|a, b| a.name.cmp(&b.name));
    for playlist in &mut merged {
        playlist.songs.sort_by_key(song_sort_key);
    }

    let total: usize = merged.iter().map(|p| p.songs.len()).sum();
    if total == 0 {
        warn!("merged canonical file contains no songs");
    }
    info!(
        "merged into {} playlists, {} unique songs",
        merged.len(),
        total
    );

    if minify {
        serde_json::to_writer(std::fs::File::create(output)?, &merged)?;
    } else {
        serde_json::to_writer_pretty(std::fs::File::create(output)?, &merged)?;
    }
    info!("successfully wrote canonical playlists to: {:?}", output);

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::music_api::{MusicApiType, Song};

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

    #[test]
    fn merge_unions_dedups_and_keeps_isrc_copy() {
        let dir = std::env::temp_dir().join(format!("sdb_merge_test_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();

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

        let canon: Vec<Playlist> =
            serde_json::from_reader(std::fs::File::open(&out).unwrap()).unwrap();

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
        let dir = std::env::temp_dir().join(format!("sdb_merge_order_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();

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
}
