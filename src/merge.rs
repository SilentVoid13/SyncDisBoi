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
