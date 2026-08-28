//! Real platform clients for the live suite.
//!
//! Credentials come from the environment (`.env` is loaded) and from the
//! OAuth token caches the CLI writes to `~/.config/SyncDisBoi`. Tests never
//! start an interactive login: a missing cache is reported as an error telling
//! you which CLI command to run first.

use std::future::Future;
use std::path::PathBuf;
use std::sync::LazyLock;

use async_trait::async_trait;
use color_eyre::eyre::{Result, WrapErr, bail, eyre};
use sync_dis_boi::ConfigArgs;
use sync_dis_boi::listenbrainz::ListenBrainzApi;
use sync_dis_boi::music_api::{DynMusicApi, MusicApi, MusicApiType, Playlist, Song};
use sync_dis_boi::spotify::SpotifyApi;
use sync_dis_boi::tidal::TidalApi;
use sync_dis_boi::yt_music::YtMusicApi;
use tokio::runtime::Runtime;
use tokio::sync::{Mutex, OnceCell};

use super::util::{TEST_PREFIX, sweep_test_playlists};

/// Every test shares one runtime, so clients built by one test (and their
/// connection pools) stay usable from the others.
static RUNTIME: LazyLock<Runtime> = LazyLock::new(|| {
    let _ = dotenvy::dotenv();
    color_eyre::install().ok();
    // `TEST_LOG=debug` to see what the clients are doing.
    let level = std::env::var("TEST_LOG")
        .ok()
        .and_then(|l| l.parse().ok())
        .unwrap_or(tracing::Level::WARN);
    tracing_subscriber::fmt()
        .with_max_level(level)
        .with_test_writer()
        .try_init()
        .ok();
    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .expect("failed to build the test runtime")
});

struct Slot {
    /// Leaked: `DynMusicApi` is `Sync` but not `Send`, and a reference to it
    /// is what can live in a static.
    api: OnceCell<&'static DynMusicApi>,
    /// Tests on one platform run one at a time: they share the account's
    /// likes, and running them concurrently only trips rate limiters.
    lock: Mutex<()>,
}

const PLATFORMS: [MusicApiType; 4] = [
    MusicApiType::Spotify,
    MusicApiType::Tidal,
    MusicApiType::YtMusic,
    MusicApiType::ListenBrainz,
];

static SLOTS: LazyLock<[Slot; 4]> = LazyLock::new(|| {
    std::array::from_fn(|_| Slot {
        api: OnceCell::new(),
        lock: Mutex::new(()),
    })
});

fn slot_index(kind: &MusicApiType) -> usize {
    PLATFORMS.iter().position(|p| p == kind).unwrap()
}

async fn client(kind: &MusicApiType) -> Result<&'static DynMusicApi> {
    let slot = &SLOTS[slot_index(kind)];
    slot.api
        .get_or_try_init(|| async {
            let api = build(kind)
                .await
                .wrap_err_with(|| format!("could not build the {kind:?} client"))?;
            if api.api_type() != *kind {
                bail!("built a {:?} client for {kind:?}", api.api_type());
            }
            let api: DynMusicApi = Box::new(TestPlaylistsOnly(api));
            // Leftovers of an earlier run that died before its cleanup.
            sweep_test_playlists(&api).await?;
            Ok(&*Box::leak(Box::new(api)))
        })
        .await
        .copied()
}

/// Hides every playlist that isn't a test playlist. Sync and
/// `get_playlists_full` read the songs of every listed playlist, which on a
/// real account means thousands of songs and minutes per call; the tests only
/// care about the playlists they create. It also guarantees the suite never
/// reads or touches the account's own playlists.
struct TestPlaylistsOnly(DynMusicApi);

#[async_trait]
impl MusicApi for TestPlaylistsOnly {
    fn api_type(&self) -> MusicApiType {
        self.0.api_type()
    }

    fn country_code(&self) -> Option<&str> {
        self.0.country_code()
    }

    async fn create_playlist(&self, name: &str, public: bool) -> Result<Playlist> {
        self.0.create_playlist(name, public).await
    }

    async fn get_playlists_info(&self) -> Result<Vec<Playlist>> {
        let mut playlists = self.0.get_playlists_info().await?;
        playlists.retain(|p| p.name.starts_with(TEST_PREFIX));
        Ok(playlists)
    }

    async fn get_playlist_songs(&self, id: &str) -> Result<Vec<Song>> {
        self.0.get_playlist_songs(id).await
    }

    async fn add_songs_to_playlist(&self, playlist: &mut Playlist, songs: &[Song]) -> Result<()> {
        self.0.add_songs_to_playlist(playlist, songs).await
    }

    async fn remove_songs_from_playlist(
        &self,
        playlist: &mut Playlist,
        songs: &[Song],
    ) -> Result<()> {
        self.0.remove_songs_from_playlist(playlist, songs).await
    }

    async fn delete_playlist(&self, playlist: Playlist) -> Result<()> {
        self.0.delete_playlist(playlist).await
    }

    async fn search_song(&self, song: &Song) -> Result<Option<Song>> {
        self.0.search_song(song).await
    }

    // Forwarded explicitly: platforms override these (`get_playlists_full` is
    // left to the default so that it goes through the filter above).
    async fn search_songs(&self, songs: &[Song]) -> Result<Vec<Option<Song>>> {
        self.0.search_songs(songs).await
    }

    async fn prefetch_searches(&self, songs: &[Song]) -> Result<()> {
        self.0.prefetch_searches(songs).await
    }

    async fn add_likes(&self, songs: &[Song]) -> Result<()> {
        self.0.add_likes(songs).await
    }

    async fn remove_likes(&self, songs: &[Song]) -> Result<()> {
        self.0.remove_likes(songs).await
    }

    async fn get_likes(&self) -> Result<Vec<Song>> {
        self.0.get_likes().await
    }
}

fn config() -> ConfigArgs {
    ConfigArgs {
        debug: false,
        like_all: false,
        sync_likes: false,
        diff_country: true,
        proxy: None,
    }
}

fn env(name: &str) -> Result<String> {
    std::env::var(name).map_err(|_| eyre!("{name} is not set (in the environment or .env)"))
}

fn token_cache(file: &str, login_cmd: &str) -> Result<PathBuf> {
    let path = dirs::config_dir()
        .ok_or(eyre!("no system config dir"))?
        .join("SyncDisBoi")
        .join(file);
    if !path.exists() {
        bail!(
            "no cached token at {}; log in once with `{login_cmd}` then rerun the tests",
            path.display()
        );
    }
    Ok(path)
}

async fn build(kind: &MusicApiType) -> Result<DynMusicApi> {
    let config = config();
    Ok(match kind {
        MusicApiType::Spotify => {
            let token = token_cache(
                "spotify_oauth.json",
                "cargo run -- spotify export -o /dev/null",
            )?;
            Box::new(
                SpotifyApi::new(
                    &env("SPOTIFY_CLIENT_ID")?,
                    &env("SPOTIFY_CLIENT_SECRET")?,
                    token,
                    SpotifyApi::REDIRECT_URI_URL,
                    false,
                    config,
                )
                .await?,
            )
        }
        MusicApiType::Tidal => {
            let token = token_cache("tidal_oauth.json", "cargo run -- tidal export -o /dev/null")?;
            let id = std::env::var("TIDAL_CLIENT_ID")
                .unwrap_or_else(|_| TidalApi::DEFAULT_CLIENT_ID.to_string());
            let secret = std::env::var("TIDAL_CLIENT_SECRET")
                .unwrap_or_else(|_| TidalApi::DEFAULT_CLIENT_SECRET.to_string());
            Box::new(TidalApi::new(&id, &secret, token, false, config).await?)
        }
        MusicApiType::YtMusic => {
            // Browser headers (the CLI's `--headers`) win when present,
            // falling back to the OAuth token cache.
            let headers = PathBuf::from(
                std::env::var("YTMUSIC_HEADERS").unwrap_or_else(|_| "browser.json".to_string()),
            );
            if headers.exists() {
                Box::new(YtMusicApi::new_headers(&headers, config)?)
            } else {
                let token = token_cache(
                    "ytmusic_oauth.json",
                    "cargo run -- yt-music export -o /dev/null",
                )?;
                Box::new(
                    YtMusicApi::new_oauth(
                        &env("YTMUSIC_CLIENT_ID")?,
                        &env("YTMUSIC_CLIENT_SECRET")?,
                        token,
                        false,
                        config,
                    )
                    .await?,
                )
            }
        }
        MusicApiType::ListenBrainz => {
            let url = std::env::var("LISTENBRAINZ_API_URL")
                .unwrap_or_else(|_| ListenBrainzApi::BASE_API.to_string());
            Box::new(ListenBrainzApi::new(&env("LISTENBRAINZ_TOKEN")?, &url, config).await?)
        }
    })
}

/// Run a single-platform test against the real `kind` client.
pub fn run<Fut>(kind: &MusicApiType, test: fn(&'static DynMusicApi) -> Fut)
where
    Fut: Future<Output = Result<()>>,
{
    let res: Result<()> = RUNTIME.block_on(async {
        let api = client(kind).await?;
        let _guard = SLOTS[slot_index(kind)].lock.lock().await;
        test(api).await
    });
    if let Err(e) = res {
        panic!("{e:?}");
    }
}

/// Run a two-platform test. Locks are taken in a fixed order so that
/// `a -> b` and `b -> a` can't deadlock each other.
pub fn run_pair<Fut>(
    src: &MusicApiType,
    dst: &MusicApiType,
    test: fn(&'static DynMusicApi, &'static DynMusicApi) -> Fut,
) where
    Fut: Future<Output = Result<()>>,
{
    let res: Result<()> = RUNTIME.block_on(async {
        let src_api = client(src).await?;
        let dst_api = client(dst).await?;
        let (first, second) = {
            let (a, b) = (slot_index(src), slot_index(dst));
            (a.min(b), a.max(b))
        };
        let _g1 = SLOTS[first].lock.lock().await;
        let _g2 = SLOTS[second].lock.lock().await;
        test(src_api, dst_api).await
    });
    if let Err(e) = res {
        panic!("{e:?}");
    }
}
