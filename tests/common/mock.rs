//! An in-memory [`MusicApi`] over the fixture catalogue, used to run the
//! contract suite and the sync logic offline.
//!
//! It mimics the platform quirks that sync has to cope with: `YtMusic` songs
//! carry no ISRC and are removed by `setVideoId`, and `ListenBrainz` songs
//! have no duration.

use std::collections::HashSet;
use std::future::Future;
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use color_eyre::eyre::{Result, bail, eyre};
use sync_dis_boi::music_api::{DynMusicApi, MusicApi, MusicApiType, Playlist, Song};

use super::fixtures::FIXTURES;

/// Clones share their state, so a test can hand one to code that takes
/// ownership of a `DynMusicApi` and inspect the account through another.
#[derive(Clone)]
pub struct MockApi {
    kind: MusicApiType,
    country: Option<String>,
    /// Each entry keeps the real ISRC even when the platform hides it.
    catalog: Vec<(Song, String)>,
    state: Arc<Mutex<State>>,
}

#[derive(Default)]
struct State {
    playlists: Vec<Playlist>,
    likes: Vec<Song>,
    /// Playlists whose songs can't be read, like a deleted followed playlist.
    unreadable: HashSet<String>,
    next_id: usize,
}

impl MockApi {
    pub fn new(kind: MusicApiType) -> Self {
        let catalog = FIXTURES
            .iter()
            .map(|f| {
                let mut song = f.song(kind.clone());
                song.id = format!("{}:{}", kind.short_name(), f.isrc);
                if kind == MusicApiType::YtMusic {
                    song.isrc = None;
                }
                if !kind.has_duration() {
                    song.duration_ms = 0;
                }
                (song, f.isrc.to_string())
            })
            .collect();
        // like the real platforms: only the region-locked ones report one
        let country = match kind {
            MusicApiType::Spotify | MusicApiType::Tidal => Some("FR".to_string()),
            MusicApiType::YtMusic | MusicApiType::ListenBrainz => None,
        };
        Self {
            kind,
            country,
            catalog,
            state: Arc::new(Mutex::new(State::default())),
        }
    }

    pub fn with_country(mut self, country: Option<&str>) -> Self {
        self.country = country.map(str::to_string);
        self
    }

    pub fn boxed(&self) -> DynMusicApi {
        Box::new(self.clone())
    }

    /// Seed a playlist directly, bypassing the API.
    pub fn seed_playlist(&self, name: &str, songs: Vec<Song>) -> String {
        let mut state = self.state.lock().unwrap();
        let id = Self::new_id(&mut state);
        state.playlists.push(Playlist {
            id: id.clone(),
            name: name.to_string(),
            songs,
        });
        id
    }

    pub fn seed_likes(&self, songs: Vec<Song>) {
        self.state.lock().unwrap().likes.extend(songs);
    }

    pub fn make_unreadable(&self, id: &str) {
        self.state.lock().unwrap().unreadable.insert(id.to_string());
    }

    /// The catalogue song for fixture `i`, as this platform returns it.
    pub fn catalog_song(&self, i: usize) -> Song {
        self.catalog[i].0.clone()
    }

    pub fn playlist(&self, name: &str) -> Option<Playlist> {
        let state = self.state.lock().unwrap();
        state
            .playlists
            .iter()
            .find(|p| p.name == name)
            .map(|p| Playlist {
                id: p.id.clone(),
                name: p.name.clone(),
                songs: p.songs.clone(),
            })
    }

    pub fn playlist_count(&self) -> usize {
        self.state.lock().unwrap().playlists.len()
    }

    pub fn likes(&self) -> Vec<Song> {
        self.state.lock().unwrap().likes.clone()
    }

    fn new_id(state: &mut State) -> String {
        state.next_id += 1;
        format!("playlist-{}", state.next_id)
    }
}

#[async_trait]
impl MusicApi for MockApi {
    fn api_type(&self) -> MusicApiType {
        self.kind.clone()
    }

    fn country_code(&self) -> Option<&str> {
        self.country.as_deref()
    }

    async fn create_playlist(&self, name: &str, _public: bool) -> Result<Playlist> {
        let id = self.seed_playlist(name, vec![]);
        Ok(Playlist {
            id,
            name: name.to_string(),
            songs: vec![],
        })
    }

    async fn get_playlists_info(&self) -> Result<Vec<Playlist>> {
        let state = self.state.lock().unwrap();
        Ok(state
            .playlists
            .iter()
            .map(|p| Playlist {
                id: p.id.clone(),
                name: p.name.clone(),
                songs: vec![],
            })
            .collect())
    }

    async fn get_playlist_songs(&self, id: &str) -> Result<Vec<Song>> {
        let state = self.state.lock().unwrap();
        if state.unreadable.contains(id) {
            bail!("playlist {id} is not accessible");
        }
        let playlist = state
            .playlists
            .iter()
            .find(|p| p.id == id)
            .ok_or(eyre!("no playlist {id}"))?;
        Ok(playlist.songs.clone())
    }

    async fn add_songs_to_playlist(&self, playlist: &mut Playlist, songs: &[Song]) -> Result<()> {
        let mut state = self.state.lock().unwrap();
        let stored = state
            .playlists
            .iter_mut()
            .find(|p| p.id == playlist.id)
            .ok_or(eyre!("no playlist {}", playlist.id))?;
        for song in songs {
            if song.source != self.kind {
                bail!("cannot add a {:?} song to {:?}", song.source, self.kind);
            }
            let mut song = song.clone();
            if self.kind == MusicApiType::YtMusic {
                song.sid = Some(format!("set-{}-{}", stored.songs.len(), song.id));
            }
            stored.songs.push(song.clone());
            playlist.songs.push(song);
        }
        Ok(())
    }

    async fn remove_songs_from_playlist(
        &self,
        playlist: &mut Playlist,
        songs: &[Song],
    ) -> Result<()> {
        let mut state = self.state.lock().unwrap();
        let stored = state
            .playlists
            .iter_mut()
            .find(|p| p.id == playlist.id)
            .ok_or(eyre!("no playlist {}", playlist.id))?;
        for song in songs {
            if self.kind == MusicApiType::YtMusic {
                let sid = song
                    .sid
                    .as_ref()
                    .ok_or(eyre!("Song setVideoId not found"))?;
                stored.songs.retain(|s| s.sid.as_ref() != Some(sid));
            } else {
                stored.songs.retain(|s| s.id != song.id);
            }
            playlist.songs.retain(|s| s.id != song.id);
        }
        Ok(())
    }

    async fn delete_playlist(&self, playlist: Playlist) -> Result<()> {
        let mut state = self.state.lock().unwrap();
        let before = state.playlists.len();
        state.playlists.retain(|p| p.id != playlist.id);
        if state.playlists.len() == before {
            bail!("no playlist {}", playlist.id);
        }
        Ok(())
    }

    async fn search_song(&self, song: &Song) -> Result<Option<Song>> {
        if let Some(isrc) = &song.isrc {
            return Ok(self
                .catalog
                .iter()
                .find(|(_, real)| real == isrc)
                .map(|(s, _)| {
                    let mut s = s.clone();
                    // like YtMusic, which echoes the ISRC it was searched with
                    s.isrc = Some(isrc.clone());
                    s
                }));
        }
        Ok(self
            .catalog
            .iter()
            .map(|(s, _)| s)
            .find(|s| song.compare(s))
            .cloned())
    }

    async fn add_likes(&self, songs: &[Song]) -> Result<()> {
        let mut state = self.state.lock().unwrap();
        for song in songs {
            if !state.likes.iter().any(|l| l.id == song.id) {
                state.likes.push(song.clone());
            }
        }
        Ok(())
    }

    async fn remove_likes(&self, songs: &[Song]) -> Result<()> {
        let mut state = self.state.lock().unwrap();
        state.likes.retain(|l| !songs.iter().any(|s| s.id == l.id));
        Ok(())
    }

    async fn get_likes(&self) -> Result<Vec<Song>> {
        Ok(self.state.lock().unwrap().likes.clone())
    }
}

fn runtime() -> tokio::runtime::Runtime {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
}

fn leak(api: MockApi) -> &'static DynMusicApi {
    let api: DynMusicApi = Box::new(api);
    Box::leak(Box::new(api))
}

/// Run a contract test against a fresh mock of `kind`.
pub fn run<Fut>(kind: &MusicApiType, test: fn(&'static DynMusicApi) -> Fut)
where
    Fut: Future<Output = Result<()>>,
{
    let api = leak(MockApi::new(kind.clone()));
    if let Err(e) = runtime().block_on(test(api)) {
        panic!("{e:?}");
    }
}

pub fn run_pair<Fut>(
    src: &MusicApiType,
    dst: &MusicApiType,
    test: fn(&'static DynMusicApi, &'static DynMusicApi) -> Fut,
) where
    Fut: Future<Output = Result<()>>,
{
    let src = leak(MockApi::new(src.clone()));
    let dst = leak(MockApi::new(dst.clone()));
    if let Err(e) = runtime().block_on(test(src, dst)) {
        panic!("{e:?}");
    }
}

/// Run an async block on a throwaway runtime (for the offline sync tests).
pub fn block_on<F: Future>(fut: F) -> F::Output {
    runtime().block_on(fut)
}
