# Spotify Multi-Market ISRC Enrichment Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** When a Spotify "like" fails to find a destination match, retry once with additional ISRCs discovered by re-querying the track under a small set of alternate Spotify markets, opt-in via `--isrc-enrich`.

**Architecture:** New default-no-op `MusicApi::enrich_isrc` trait method, overridden by `SpotifyApi` to call `GET /tracks/{id}?market=X` per configured market and merge any newly-found ISRC into the song. `synchronize_likes()` calls it (gated by `config.isrc_enrich`) inside its existing bounded-concurrency search closure when the first search comes back empty, then retries the destination search once with the enriched song.

**Tech Stack:** Rust, clap (derive), reqwest, existing `SpotifyApi`/`MusicApi` trait infrastructure.

---

### Task 1: `--isrc-enrich` / `--isrc-markets` CLI flags

**Files:**
- Modify: `src/lib.rs:65-67` (end of `ConfigArgs`, right after the `strip_qualifiers` field)
- Modify: `src/spotify/mod.rs:542-553` (`test_config()`, the only place that constructs `ConfigArgs` by struct literal with every field named)

Rust's whole-crate compilation means adding a struct field and fixing every literal-construction call site must land together, or nothing compiles. `test_config()` is the only such call site in the crate (confirmed via `grep -rn "ConfigArgs {" src/`).

- [ ] **Step 1: Add the two fields to `ConfigArgs`**

In `src/lib.rs`, after the `strip_qualifiers` field (the last field in the struct), add:

```rust
    /// Attempt to discover additional ISRCs for a Spotify like that fails
    /// to find a destination match, by re-querying the track under the
    /// markets in --isrc-markets (Spotify's track relinking can serve a
    /// different regional release -- and ISRC -- per market). Adds up to
    /// `isrc_markets.len()` extra Spotify requests per unmatched like.
    #[arg(long, default_value = "false")]
    pub isrc_enrich: bool,

    /// Markets to probe when --isrc-enrich is set, as a comma-separated
    /// list of ISO 3166-1 alpha-2 country codes.
    #[arg(long, value_delimiter = ',', default_value = "US,GB,DE,JP,BR")]
    pub isrc_markets: Vec<String>,
```

- [ ] **Step 2: Update `test_config()` in `src/spotify/mod.rs`**

Add the two new fields to the struct literal so it keeps compiling:

```rust
    fn test_config() -> ConfigArgs {
        ConfigArgs {
            debug: false,
            like_all: false,
            sync_likes: false,
            diff_country: false,
            proxy: None,
            search_concurrency: 8,
            map_singles: false,
            strip_qualifiers: true,
            isrc_enrich: false,
            isrc_markets: vec![],
        }
    }
```

- [ ] **Step 3: Confirm the crate compiles and the flags surface correctly**

Run: `cargo build 2>&1 | tail -30`
Expected: clean build, no errors (this is the first task, so there's no prior test to run yet -- just confirm compilation).

Run: `cargo run -- --help 2>&1 | grep -A3 isrc`
Expected: both `--isrc-enrich` (default `false`) and `--isrc-markets` (default `US,GB,DE,JP,BR`) listed with their doc comments.

- [ ] **Step 4: Commit**

```bash
git add src/lib.rs src/spotify/mod.rs
git commit -m "feat(lib): add --isrc-enrich and --isrc-markets flags"
```

---

### Task 2: `MusicApi::enrich_isrc` default trait method

**Files:**
- Modify: `src/music_api.rs:114` (end of the `MusicApi` trait, right after `async fn get_likes(&self) -> Result<Vec<Song>>;` and before the trait's closing `}`)

No test for this task: the default body is an unconditional `Ok(())` that never touches `song`, true by inspection. Exercising it through the trait would mean stubbing every other `MusicApi` method (13 of them) on a mock implementor for one assertion that compiling already proves -- the design doc explicitly calls this out as disproportionate. Task 3 (the real `SpotifyApi` override) is where the actual behavior lives and gets covered by manual verification per its own steps.

- [ ] **Step 1: Add the trait method**

In `src/music_api.rs`, inside `pub trait MusicApi { ... }`, right after `async fn get_likes(&self) -> Result<Vec<Song>>;` and before the trait's closing `}`:

```rust

    /// Attempts to discover additional ISRCs for `song` by re-querying
    /// under alternate markets/storefronts. Populates `song.isrc` with any
    /// newly found codes (existing codes are preserved; duplicates are
    /// skipped). Default: no-op. Only Spotify currently has documented
    /// per-market ISRC variance (track relinking); other platforms don't
    /// need this.
    async fn enrich_isrc(&self, _song: &mut Song, _markets: &[String]) -> Result<()> {
        Ok(())
    }
```

- [ ] **Step 2: Run tests, confirm the crate still compiles**

Run: `cargo test --lib 2>&1 | tail -20`
Expected: all existing tests still pass (adding a default trait method doesn't require any implementor to change).

- [ ] **Step 3: Commit**

```bash
git add src/music_api.rs
git commit -m "feat(music_api): add enrich_isrc trait method with a no-op default"
```

---

### Task 3: `SpotifyApi::enrich_isrc` implementation

**Files:**
- Modify: `src/spotify/mod.rs` (imports, and the `impl MusicApi for SpotifyApi` block)

- [ ] **Step 1: Add the missing imports**

In `src/spotify/mod.rs`, `SpotifySongResponse` and `clean_isrc` aren't imported yet (confirmed via `grep -n "clean_isrc\|SpotifySongResponse" src/spotify/mod.rs` -- neither appears outside `response.rs`). Update the two relevant `use` lines:

Replace:
```rust
use self::model::{
    SpotifyPageResponse, SpotifyPlaylistResponse, SpotifySnapshotResponse, SpotifySongItemResponse,
};
```
with:
```rust
use self::model::{
    SpotifyPageResponse, SpotifyPlaylistResponse, SpotifySnapshotResponse, SpotifySongItemResponse,
    SpotifySongResponse,
};
```

Replace:
```rust
use crate::utils::debug_response_json;
```
with:
```rust
use crate::utils::{clean_isrc, debug_response_json};
```

- [ ] **Step 2: Run cargo check to confirm the imports compile (both currently unused -- expect warnings, not errors)**

Run: `cargo build 2>&1 | tail -20`
Expected: builds with `unused import` warnings for `SpotifySongResponse` and `clean_isrc` (not yet used) -- confirms the imports themselves are valid before wiring in the method that uses them.

- [ ] **Step 3: Implement `enrich_isrc`**

In `src/spotify/mod.rs`, inside `impl MusicApi for SpotifyApi { ... }`, add the method (placement doesn't matter functionally; put it after `search_song` and before `add_likes` to keep search-related methods grouped):

```rust
    async fn enrich_isrc(&self, song: &mut Song, markets: &[String]) -> Result<()> {
        let path = format!("/tracks/{}", song.id);
        for market in markets {
            // Already implicitly tried via the account's own market.
            if market.eq_ignore_ascii_case(&self.country_code) {
                continue;
            }
            let res: SpotifySongResponse = self
                .make_request_json(&path, &HttpMethod::Get(&[("market", market.as_str())]), 50, 0)
                .await?;
            if let Some(isrc) = clean_isrc(res.external_ids.isrc) {
                if !song.isrc.contains(&isrc) {
                    song.isrc.push(isrc);
                }
            }
        }
        Ok(())
    }
```

- [ ] **Step 4: Run tests and clippy**

Run: `cargo test --lib 2>&1 | tail -20`
Expected: all tests pass (this method isn't called by anything yet, so nothing existing should change behavior).

Run: `PATH=/usr/lib/rust-1.85/bin:$PATH cargo clippy --all-targets -- -D warnings 2>&1 | tail -60`
Expected: no new errors beyond the 6 pre-existing, unrelated ones (`src/music_api.rs` duration-bucket cast and ISRC-mapping redundant-closure/inefficient-to-string, `src/sync.rs`'s two `attempts += ... as i32` casts). The unused-import warnings from Step 2 should be gone now that `SpotifySongResponse` and `clean_isrc` are both used.

- [ ] **Step 5: Commit**

```bash
git add src/spotify/mod.rs
git commit -m "feat(spotify): implement enrich_isrc via per-market track lookups"
```

---

### Task 4: Wire enrichment into `synchronize_likes()`

**Files:**
- Modify: `src/sync.rs:314-321` (the `stream::iter(to_search).map(...)` closure inside `synchronize_likes()`)

- [ ] **Step 1: Replace the search closure**

Replace:
```rust
    let search_results: Vec<(Song, Result<Option<Song>>)> = stream::iter(to_search)
        .map(|src_like| async move {
            let result = dst_api.search_song(&src_like).await;
            (src_like, result)
        })
        .buffered(config.search_concurrency.max(1))
        .collect()
        .await;
```
with:
```rust
    let search_results: Vec<(Song, Result<Option<Song>>)> = stream::iter(to_search)
        .map(|src_like| async move {
            let mut result = dst_api.search_song(&src_like).await;
            if config.isrc_enrich && matches!(result, Ok(None)) {
                let mut enriched = src_like.clone();
                if src_api
                    .enrich_isrc(&mut enriched, &config.isrc_markets)
                    .await
                    .is_ok()
                    && enriched.isrc.len() > src_like.isrc.len()
                {
                    result = dst_api.search_song(&enriched).await;
                }
            }
            (src_like, result)
        })
        .buffered(config.search_concurrency.max(1))
        .collect()
        .await;
```

This closure now captures `src_api` and `config` in addition to the already-captured `dst_api` -- both are `&`-references already in scope in `synchronize_likes()` (its own `src_api: &DynMusicApi` and `config: &ConfigArgs` parameters), so no signature changes are needed elsewhere.

- [ ] **Step 2: Run tests and clippy**

Run: `cargo build 2>&1 | tail -30`
Expected: clean build. If this fails with a borrow/lifetime error about `src_api`/`config` not living long enough inside the `async move` block, it's because `stream::iter(...).buffered(...)` requires each future to be independently `'static`-ish across the whole stream's lifetime -- `src_api`/`config`/`dst_api` are `&` references from the enclosing function, and `async move` moves the *references themselves* (which are `Copy`) into each future, so this should compile the same way the pre-existing `dst_api` capture already does. If it doesn't, capture `src_api`/`config` the same way `dst_api` already is and re-check for a typo rather than restructuring.

Run: `cargo test --lib 2>&1 | tail -20`
Expected: all tests still pass (no unit tests exercise `synchronize_likes()` directly -- it's network-dependent and untested at the unit level, consistent with the rest of `src/sync.rs`).

Run: `PATH=/usr/lib/rust-1.85/bin:$PATH cargo clippy --all-targets -- -D warnings 2>&1 | tail -60`
Expected: no new errors beyond the same 6 pre-existing ones.

- [ ] **Step 3: Commit**

```bash
git add src/sync.rs
git commit -m "feat(sync): retry unmatched likes with market-enriched ISRCs"
```

---

### Task 5: Full verification pass

**Files:** none (verification only)

- [ ] **Step 1: Run the full test suite**

Run: `cargo test --lib 2>&1 | tail -60`
Expected: `test result: ok.` for the whole suite, same pass count as before this plan started (this plan adds no new tests -- see Task 2's rationale).

- [ ] **Step 2: Run clippy**

Run: `PATH=/usr/lib/rust-1.85/bin:$PATH cargo clippy --all-targets -- -D warnings 2>&1 | tail -60`
Expected: only the 6 pre-existing, unrelated errors -- confirm none of this plan's new code (`enrich_isrc` in either file, the `synchronize_likes` closure) appears in the output.

- [ ] **Step 3: Confirm both new flags end-to-end**

Run: `cargo run --release -- --help 2>&1 | grep -B1 -A4 isrc-enrich`
Expected: flag listed, default `false`.

Run: `cargo run --release -- --help 2>&1 | grep -B1 -A4 isrc-markets`
Expected: flag listed, default `US,GB,DE,JP,BR`.

Run: `cargo run --release -- --isrc-markets US,CA --help 2>&1 | head -5`
Expected: no clap parsing error (confirms the comma-delimited override parses).

- [ ] **Step 4: Final commit (only if Steps 1-3 required fixes)**

```bash
git add -A
git commit -m "chore: fix test/clippy fallout from isrc-enrich work"
```

If no fixes were needed, skip this commit -- Tasks 1-4 already cover the full change.
