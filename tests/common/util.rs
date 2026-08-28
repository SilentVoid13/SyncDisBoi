use std::future::Future;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use color_eyre::eyre::{Result, WrapErr, eyre};
use sync_dis_boi::music_api::{DynMusicApi, Playlist, Song};

/// Every playlist a test creates starts with this, and nothing else is ever
/// deleted by the suite.
pub const TEST_PREFIX: &str = "SyncDisBoi-Test-";

pub fn unique_name(label: &str) -> String {
    static COUNTER: AtomicUsize = AtomicUsize::new(0);
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .subsec_nanos();
    let n = COUNTER.fetch_add(1, Ordering::Relaxed);
    format!("{TEST_PREFIX}{label}-{nanos:08x}{n}")
}

/// Platforms are eventually consistent: a write is not always visible to the
/// next read. Polls `probe` until it yields a value, returning the last
/// error (or a timeout) if it never does.
pub async fn eventually<T, F, Fut>(what: &str, mut probe: F) -> Result<T>
where
    F: FnMut() -> Fut,
    Fut: Future<Output = Result<Option<T>>>,
{
    const TIMEOUT: Duration = Duration::from_secs(45);
    const INTERVAL: Duration = Duration::from_secs(3);
    let start = Instant::now();
    loop {
        let last = match probe().await {
            Ok(Some(v)) => return Ok(v),
            Ok(None) => eyre!("condition not met"),
            Err(e) => e,
        };
        if start.elapsed() > TIMEOUT {
            return Err(last).wrap_err(format!("timed out waiting for: {what}"));
        }
        tokio::time::sleep(INTERVAL).await;
    }
}

/// A fresh, empty directory under the system temp dir.
pub fn temp_dir(label: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(unique_name(label));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

pub async fn find_playlist(api: &DynMusicApi, name: &str) -> Result<Option<Playlist>> {
    Ok(api
        .get_playlists_info()
        .await?
        .into_iter()
        .find(|p| p.name == name))
}

/// Poll until playlist `id` holds exactly `len` songs.
pub async fn wait_for_playlist_len(api: &DynMusicApi, id: &str, len: usize) -> Result<Vec<Song>> {
    eventually(&format!("playlist {id} to hold {len} songs"), || async {
        let songs = api.get_playlist_songs(id).await?;
        Ok((songs.len() == len).then_some(songs))
    })
    .await
}

pub async fn sweep_test_playlists(api: &DynMusicApi) -> Result<()> {
    for playlist in api.get_playlists_info().await? {
        if playlist.name.starts_with(TEST_PREFIX) {
            api.delete_playlist(playlist).await?;
        }
    }
    Ok(())
}

pub async fn delete_by_name(api: &DynMusicApi, name: &str) -> Result<()> {
    if let Some(playlist) = find_playlist(api, name).await? {
        api.delete_playlist(playlist).await?;
    }
    Ok(())
}

/// Returns `body`'s error first, then `cleanup`'s. Contract tests use this
/// (and `ensure!` rather than `assert!`) so a failure never skips cleanup.
pub fn first_error<T>(body: Result<T>, cleanup: Result<()>) -> Result<T> {
    match (body, cleanup) {
        (Err(e), _) => Err(e),
        (Ok(_), Err(e)) => Err(e.wrap_err("test passed but cleanup failed")),
        (Ok(v), Ok(())) => Ok(v),
    }
}

/// Create a test playlist, hand it to `body`, and always delete it.
pub async fn with_playlist<T, F, Fut>(api: &'static DynMusicApi, label: &str, body: F) -> Result<T>
where
    F: FnOnce(Playlist) -> Fut,
    Fut: Future<Output = Result<T>>,
{
    let name = unique_name(label);
    let playlist = api.create_playlist(&name, false).await?;
    let handle = Playlist {
        id: playlist.id.clone(),
        name: playlist.name.clone(),
        songs: vec![],
    };
    let res = body(playlist).await;
    first_error(res, api.delete_playlist(handle).await)
}

pub fn describe(songs: &[Song]) -> String {
    songs
        .iter()
        .map(|s| format!("  - [{}] {}", s.id, s))
        .collect::<Vec<_>>()
        .join("\n")
}

pub fn contains_match(songs: &[Song], song: &Song) -> bool {
    songs.iter().any(|s| s.compare(song))
}
