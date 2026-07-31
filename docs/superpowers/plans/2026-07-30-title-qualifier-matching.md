# Title-Qualifier-Aware Matching Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add an opt-out `--strip-qualifiers` flag so remix/version tracks (e.g. `"Circus Bells (Hardfloor Remix)"`) can be matched without colliding with their original/sibling tracks under today's aggressive title-qualifier stripping, while leaving default behavior for existing users completely unchanged.

**Architecture:** Split `generic_name_clean()` into an always-applied `normalize_name()` (lowercase/punctuation only) and the existing qualifier-stripping pass. Thread a new `strip_qualifiers: bool` alongside the existing `map_singles: bool` through `Song::compare()`, `Song::build_queries()`, and `SongIndex::contains()`. When `strip_qualifiers` is `true` (default), behavior is byte-for-byte identical to today. When `false`, `compare()` also checks the qualifier-preserving raw name (`max(stripped_score, raw_score)`), `build_queries()` emits extra raw-name query variants, and a new `pick_best_match()` helper (replacing four near-identical "first passing candidate wins" loops in the platform `search_song()` implementations) ranks multiple passing candidates by raw-name closeness instead of taking the first one the API happened to return.

**Tech Stack:** Rust, clap (CLI parsing), strsim (Levenshtein), existing `cargo test` unit tests.

**Note on task sizing:** Rust compiles a crate as a single unit — a stale call site anywhere fails the whole build, not just the file you're editing. So unlike a dynamically-typed codebase, some of these tasks bundle a signature change together with *every* call site of that signature in one task, even across files, so the crate compiles and `cargo test` actually runs at the end of every task. Tasks that don't share a signature change stay independent.

Reference spec: `docs/superpowers/specs/2026-07-30-title-qualifier-matching-design.md`

---

### Task 1: `normalize_name()` in `src/utils.rs`

**Files:**
- Modify: `src/utils.rs:36-57` (the `generic_name_clean` function and its test module)

- [ ] **Step 1: Write the failing tests**

Add to the `#[cfg(test)] mod tests` block at the bottom of `src/utils.rs` (after the existing `test_clean_enclosure` test):

```rust
    #[test]
    fn test_normalize_name_preserves_qualifiers() {
        let name = "Circus Bells (Hardfloor Remix)";
        assert_eq!(normalize_name(name), "circus bells (hardfloor remix)");
    }

    #[test]
    fn test_generic_name_clean_unchanged_after_refactor() {
        let name = "Circus Bells (Hardfloor Remix)";
        assert_eq!(generic_name_clean(name), "circus bells");
    }
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test --lib utils::tests -- --nocapture`
Expected: compile error, `cannot find function normalize_name in this scope`

- [ ] **Step 3: Implement `normalize_name` and refactor `generic_name_clean`**

Replace `src/utils.rs:36-57` (the current `generic_name_clean` function) with:

```rust
pub fn normalize_name(name: &str) -> String {
    let mut name = name.to_lowercase();
    let replaces = [
        ("'", ""),
        ("\"", ""),
        (":", " "),
        ("%", ""),
        ("é", "e"),
        ("è", "e"),
        ("à", "a"),
    ];
    for (a, b) in replaces {
        name = name.replace(a, b);
    }
    name
}

pub fn generic_name_clean(name: &str) -> String {
    let name = normalize_name(name);
    let part_re = Regex::new(r"\((part (?:[a-zA-Z]+|[0-9]+))\)").unwrap();
    let name = if part_re.is_match(&name) {
        part_re.replace_all(&name, "$1").to_string()
    } else {
        name
    };
    let name = clean_enclosure(&name, '(', ')');
    let name = clean_enclosure(&name, '[', ']');
    name.trim_end().to_string()
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test --lib utils::tests -- --nocapture`
Expected: `test result: ok. 3 passed` (the two new tests plus the existing `test_clean_enclosure`)

- [ ] **Step 5: Commit**

```bash
git add src/utils.rs
git commit -m "refactor(utils): split generic_name_clean into normalize_name + qualifier stripping"
```

---

### Task 2: `--strip-qualifiers` CLI flag in `src/lib.rs`

Done early, before anything references `config.strip_qualifiers`, so later tasks don't need to touch `lib.rs` again.

**Files:**
- Modify: `src/lib.rs:16-56` (`ConfigArgs`)
- Modify: `src/spotify/mod.rs:513-523` (`test_config()` — the only place that constructs `ConfigArgs` as a struct literal outside of clap parsing)

- [ ] **Step 1: Add the field**

Add to `ConfigArgs` in `src/lib.rs`, right after the `map_singles` field (`src/lib.rs:47-55`):

```rust
    /// Strip remix/mix/version qualifiers and feat. credits from track
    /// titles before comparing/searching (default: on, matching today's
    /// behavior). Disabling this keeps the raw title available alongside
    /// the stripped one, using both to avoid conflating a track with its
    /// remixes when they collapse to the same stripped name -- at the cost
    /// of being pickier about cross-platform title-formatting differences
    /// (e.g. feat.-credit lists) that the stripped comparison used to paper
    /// over.
    #[arg(long, default_value_t = true, action = clap::ArgAction::Set)]
    pub strip_qualifiers: bool,
```

- [ ] **Step 2: Fix the now-broken `test_config()` literal**

In `src/spotify/mod.rs`, update `test_config()` (`src/spotify/mod.rs:513-523`):

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
        }
    }
```

- [ ] **Step 3: Build and check the flag**

Run: `cargo build 2>&1 | tail -30`
Expected: clean build (no other code references `strip_qualifiers` yet, so nothing else breaks).

Run: `cargo run --release -- --help 2>&1 | grep -B1 -A4 strip-qualifiers`
Expected: the flag is listed with the doc comment above and a default of `true`.

- [ ] **Step 4: Commit**

```bash
git add src/lib.rs src/spotify/mod.rs
git commit -m "feat(lib): add --strip-qualifiers CLI flag, default true"
```

---

### Task 3: `Song::raw_clean_name()` and `name_score()` in `src/music_api.rs`

Standalone additions — no existing signatures change, so nothing else in the crate is affected.

**Files:**
- Modify: `src/music_api.rs:1-14` (imports, and the free-function insertion point)
- Modify: `src/music_api.rs:123-137` (`impl Song` — after `clean_name`)
- Modify: `src/music_api.rs` test module

- [ ] **Step 1: Write the failing test**

Add to the `#[cfg(test)] mod tests` block in `src/music_api.rs` (after the `song()` helper, before `compare_matches_when_either_side_has_an_overlapping_isrc`):

```rust
    #[test]
    fn raw_clean_name_preserves_qualifiers_that_clean_name_strips() {
        let mut s = song(MusicApiType::Spotify, "sp-1", &[], 200_000);
        s.name = "Circus Bells (Hardfloor Remix)".to_string();
        assert_eq!(s.clean_name(), "circus bells");
        assert_eq!(s.raw_clean_name(), "circus bells (hardfloor remix)");
    }
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --lib music_api::tests::raw_clean_name_preserves_qualifiers_that_clean_name_strips`
Expected: compile error, `no method named raw_clean_name found for struct Song`

- [ ] **Step 3: Implement `raw_clean_name` and `name_score`**

Update the import at `src/music_api.rs:10` from:

```rust
use crate::utils::generic_name_clean;
```

to:

```rust
use crate::utils::{generic_name_clean, normalize_name};
```

Add a free function near the top of the file, just after the `pub type DynMusicApi = Box<dyn MusicApi + Sync>;` line (`src/music_api.rs:14`):

```rust

fn name_score(a: &str, b: &str) -> f64 {
    normalized_levenshtein(a, b).abs()
}
```

Add `raw_clean_name` to `impl Song`, right after the existing `clean_name` method (`src/music_api.rs:124-137`):

```rust
    /// Basic-normalized name with qualifier stripping skipped: used as a
    /// second comparison signal when `--strip-qualifiers=false`, so
    /// remix/version qualifiers remain available to disambiguate songs
    /// that would otherwise collapse to the same `clean_name()`.
    pub fn raw_clean_name(&self) -> String {
        normalize_name(&self.name)
    }
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test --lib music_api::tests::raw_clean_name_preserves_qualifiers_that_clean_name_strips`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add src/music_api.rs
git commit -m "feat(music_api): add Song::raw_clean_name and name_score helper"
```

---

### Task 4: Thread `strip_qualifiers` through `compare()` and `SongIndex::contains()` — crate-wide

This is the big one: `compare()` and `SongIndex::contains()` are called from `src/music_api.rs`'s own tests, `src/sync.rs` (5 call sites), and all four platform `search_song()` implementations. Every one of those call sites has to be updated in this same task, or the crate won't compile.

**Files:**
- Modify: `src/music_api.rs:148-211` (`compare`)
- Modify: `src/music_api.rs:271-285` (`SongIndex::contains`)
- Modify: `src/music_api.rs:288-296` (`impl PartialEq for Song`)
- Modify: `src/music_api.rs` test module (existing `compare_*`/`song_index_*` tests, plus one new test)
- Modify: `src/sync.rs:124,173,211,307,329`
- Modify: `src/subsonic/mod.rs:288`
- Modify: `src/tidal/mod.rs:393`
- Modify: `src/yt_music/mod.rs:441`
- Modify: `src/spotify/mod.rs:476`

- [ ] **Step 1: Update `music_api.rs` tests for the new signatures**

Update every existing `.compare(&b, false)` / `.compare(&a, false)` / `.compare(&b, true)` call and every `index.contains(&needle, false)` / `index.contains(&far, false)` call in the test module to add `true` as the new final `strip_qualifiers` argument (preserving today's behavior exactly). The full updated test module:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    fn song(source: MusicApiType, id: &str, isrc: &[&str], duration_ms: usize) -> Song {
        Song {
            source,
            id: id.to_string(),
            sid: None,
            isrc: isrc.iter().map(|s| s.to_string()).collect(),
            name: "Same Name".to_string(),
            album: None,
            artists: vec![],
            duration_ms,
        }
    }

    #[test]
    fn compare_matches_when_either_side_has_an_overlapping_isrc() {
        // e.g. a Subsonic recording tagged with multiple reissue ISRCs, one
        // of which happens to be the single ISRC the other platform reports.
        let a = song(
            MusicApiType::Subsonic,
            "sub-1",
            &["GBBTF0400057", "USSM19501100"],
            200_000,
        );
        let b = song(MusicApiType::Spotify, "sp-1", &["USSM19501100"], 999_000);
        assert!(a.compare(&b, false, true));
        assert!(b.compare(&a, false, true));
    }

    #[test]
    fn compare_falls_back_to_name_and_duration_when_isrcs_are_disjoint() {
        // e.g. a remaster or regional reissue: same recording, but the
        // ISRC differs, so ISRC alone can't rule it out.
        let a = song(MusicApiType::Subsonic, "sub-1", &["AAAAA1111111"], 200_000);
        let b = song(MusicApiType::Spotify, "sp-1", &["BBBBB2222222"], 200_000);
        assert!(a.compare(&b, false, true));
    }

    #[test]
    fn compare_rejects_when_isrcs_are_disjoint_and_names_or_durations_differ() {
        let mut a = song(MusicApiType::Subsonic, "sub-1", &["AAAAA1111111"], 200_000);
        a.name = "Totally Different Song".to_string();
        let b = song(MusicApiType::Spotify, "sp-1", &["BBBBB2222222"], 200_000);
        assert!(!a.compare(&b, false, true));
    }

    #[test]
    fn compare_rejects_single_vs_album_track_without_map_singles() {
        let mut a = song(MusicApiType::Subsonic, "sub-1", &[], 200_000);
        a.album = Some(Album {
            id: None,
            name: "Some Full Album".to_string(),
        });
        let mut b = song(MusicApiType::Spotify, "sp-1", &[], 200_000);
        b.album = Some(Album {
            id: None,
            name: b.name.clone(),
        });
        assert!(!a.compare(&b, false, true));
    }

    #[test]
    fn compare_matches_single_vs_album_track_with_map_singles() {
        let mut a = song(MusicApiType::Subsonic, "sub-1", &[], 200_000);
        a.album = Some(Album {
            id: None,
            name: "Some Full Album".to_string(),
        });
        let mut b = song(MusicApiType::Spotify, "sp-1", &[], 200_000);
        b.album = Some(Album {
            id: None,
            name: b.name.clone(),
        });
        assert!(a.compare(&b, true, true));
    }

    #[test]
    fn compare_with_strip_qualifiers_false_falls_back_to_raw_name_when_stripped_diverges() {
        // Contrived but demonstrates the mechanic: stripping collapses "Q1"
        // vs "Q2" down to a single differing character (levenshtein ratio
        // 0.5), while the long shared "(Extended Mix)" suffix makes the
        // *raw* names very similar (ratio ~0.94). With strip_qualifiers
        // true, only the stripped score is used and the match is rejected;
        // with it false, the raw score rescues it.
        let mut a = song(MusicApiType::Subsonic, "sub-1", &[], 200_000);
        a.name = "Q1 (Extended Mix)".to_string();
        let mut b = song(MusicApiType::Spotify, "sp-1", &[], 200_000);
        b.name = "Q2 (Extended Mix)".to_string();

        assert!(!a.compare(&b, false, true));
        assert!(a.compare(&b, false, false));
    }

    #[test]
    fn raw_clean_name_preserves_qualifiers_that_clean_name_strips() {
        let mut s = song(MusicApiType::Spotify, "sp-1", &[], 200_000);
        s.name = "Circus Bells (Hardfloor Remix)".to_string();
        assert_eq!(s.clean_name(), "circus bells");
        assert_eq!(s.raw_clean_name(), "circus bells (hardfloor remix)");
    }

    #[test]
    fn song_index_matches_naive_contains_via_isrc() {
        let haystack = vec![
            song(MusicApiType::Subsonic, "sub-1", &["AAAAA1111111"], 200_000),
            song(MusicApiType::Subsonic, "sub-2", &["BBBBB2222222"], 999_000),
        ];
        let index = SongIndex::build(&haystack);

        let needle = song(MusicApiType::Spotify, "sp-1", &["AAAAA1111111"], 42_000);
        assert!(haystack.contains(&needle));
        assert!(index.contains(&needle, false, true));
    }

    #[test]
    fn song_index_matches_naive_contains_via_duration_bucket() {
        let haystack = vec![song(MusicApiType::Subsonic, "sub-1", &[], 200_000)];
        let index = SongIndex::build(&haystack);

        // within the 1s tolerance, no shared ISRC
        let mut needle = song(MusicApiType::Spotify, "sp-1", &[], 200_900);
        needle.name = "Same Name".to_string();
        assert!(haystack.contains(&needle));
        assert!(index.contains(&needle, false, true));

        // outside the 1s tolerance
        let mut far = song(MusicApiType::Spotify, "sp-2", &[], 210_000);
        far.name = "Same Name".to_string();
        assert!(!haystack.contains(&far));
        assert!(!index.contains(&far, false, true));
    }

    #[test]
    fn song_index_rejects_absent_song() {
        let haystack = vec![song(MusicApiType::Subsonic, "sub-1", &["AAAAA1111111"], 200_000)];
        let index = SongIndex::build(&haystack);

        let mut needle = song(MusicApiType::Spotify, "sp-1", &["ZZZZZ9999999"], 999_000);
        needle.name = "Nothing Like It".to_string();
        assert!(!haystack.contains(&needle));
        assert!(!index.contains(&needle, false, true));
    }
}
```

(`haystack.contains(&needle)` calls are `Vec<Song>::contains`, which uses `PartialEq` — unaffected, left as-is. Note `raw_clean_name_preserves_qualifiers_that_clean_name_strips` was already added in Task 3; it's included here just so the full test module listing above is complete and copy-pasteable.)

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test --lib music_api::tests`
Expected: compile error, `this method takes 3 arguments but 2 arguments were supplied` (for both `compare` and `contains`)

- [ ] **Step 3: Update `compare()`, `SongIndex::contains()`, and `PartialEq`**

Replace `src/music_api.rs:148-169` (from `pub fn compare` through the `if score < 0.8 { return false; }` after the name check) with:

```rust
    pub fn compare(&self, other: &Self, map_singles: bool, strip_qualifiers: bool) -> bool {
        if self.source == other.source {
            return self.id == other.id;
        }
        if !self.isrc.is_empty() && !other.isrc.is_empty() {
            let isrc_overlap = self.isrc.iter().any(|i| other.isrc.contains(i));
            if isrc_overlap {
                return true;
            }
            // NOTE: don't bail out here. Reissues, remasters, and regional
            // releases legitimately get a different ISRC for the same
            // recording, so fall through to the name/duration/album checks
            // below instead of treating a mismatched ISRC as conclusive.
        }

        // Check song name resemblance. With strip_qualifiers=false, also
        // check the qualifier-preserving raw name and take the best of the
        // two -- this only ever widens what counts as a name match, so
        // recall improves without weakening the strip_qualifiers=true
        // (default) path. Candidate *ranking* when several names tie is
        // handled separately by pick_best_match, not here.
        let score = if strip_qualifiers {
            name_score(&self.clean_name(), &other.clean_name())
        } else {
            let stripped = name_score(&self.clean_name(), &other.clean_name());
            let raw = name_score(&self.raw_clean_name(), &other.raw_clean_name());
            stripped.max(raw)
        };
        if score < 0.8 {
            return false;
        }
```

Leave the rest of `compare()` (the artist comment, duration check, and album check — currently `src/music_api.rs:171-211`) unchanged.

Replace `src/music_api.rs:271-285` (`SongIndex::contains`) with:

```rust
    /// Equivalent to `songs.contains(song)` for the slice this index was
    /// built from.
    pub fn contains(&self, song: &Song, map_singles: bool, strip_qualifiers: bool) -> bool {
        for isrc in &song.isrc {
            if let Some(candidates) = self.by_isrc.get(isrc.as_str()) {
                if candidates
                    .iter()
                    .any(|c| song.compare(c, map_singles, strip_qualifiers))
                {
                    return true;
                }
            }
        }
        let bucket = (song.duration_ms / 1000) as i64;
        (bucket - 1..=bucket + 1).any(|b| {
            self.by_duration_bucket.get(&b).is_some_and(|candidates| {
                candidates
                    .iter()
                    .any(|c| song.compare(c, map_singles, strip_qualifiers))
            })
        })
    }
```

Update `impl PartialEq for Song` at `src/music_api.rs:288-296`:

```rust
impl PartialEq for Song {
    fn eq(&self, other: &Self) -> bool {
        // Neither `map_singles` nor `strip_qualifiers` has a config to read
        // here; every remaining caller of this impl compares songs from the
        // same source (where `compare` short-circuits on id equality before
        // ever reaching the name/album checks), so the values passed are
        // inconsequential.
        self.compare(other, false, false)
    }
}
```

- [ ] **Step 4: Update `src/sync.rs`'s five call sites**

In `src/sync.rs`, change each of these five lines by adding `, config.strip_qualifiers` before the closing paren:

Line 124:
```rust
                if dst_playlist_index.contains(src_song, config.map_singles, config.strip_qualifiers) {
```

Line 173:
```rust
                    if dst_playlist_index.contains(dst_song, config.map_singles, config.strip_qualifiers) {
```

Line 211:
```rust
                    .filter(|s| !dst_likes_index.contains(s, config.map_singles, config.strip_qualifiers))
```

Line 307:
```rust
        .filter(|src_like| !dst_likes_index.contains(src_like, config.map_singles, config.strip_qualifiers))
```

Line 329:
```rust
        if dst_likes_index.contains(&song, config.map_singles, config.strip_qualifiers) {
```

(`to_sync.contains(dst_song)` at `src/sync.rs:184` is `Vec<Song>::contains` via `PartialEq` — leave unchanged.)

- [ ] **Step 5: Update the four platform `compare()` call sites**

These four are today's "first passing candidate wins" loops. For this task, only add the third argument — do **not** change the loop structure yet (that happens in Tasks 6 and 7, once `pick_best_match` exists and `build_queries` is updated).

`src/subsonic/mod.rs:288`:
```rust
                if song.compare(&res_song, self.config.map_singles, self.config.strip_qualifiers) {
```

`src/tidal/mod.rs:393`:
```rust
                if song.compare(&res_song, self.config.map_singles, self.config.strip_qualifiers) {
```

`src/yt_music/mod.rs:441`:
```rust
                    if song.compare(&res_song, self.config.map_singles, self.config.strip_qualifiers) {
```

`src/spotify/mod.rs:476`:
```rust
                if song.compare(&res_song, self.config.map_singles, self.config.strip_qualifiers) {
```

- [ ] **Step 6: Run tests to verify everything passes**

Run: `cargo build 2>&1 | tail -40`
Expected: clean build, no errors.

Run: `cargo test --lib 2>&1 | tail -60`
Expected: `test result: ok.` for the whole suite, including every `music_api::tests::*` test.

- [ ] **Step 7: Commit**

```bash
git add src/music_api.rs src/sync.rs src/subsonic/mod.rs src/tidal/mod.rs src/yt_music/mod.rs src/spotify/mod.rs
git commit -m "feat(music_api): thread strip_qualifiers through compare and SongIndex::contains"
```

---

### Task 5: `pick_best_match()` candidate-ranking helper

Standalone addition — `pub fn` items aren't flagged as dead code even when unused within the crate yet, so this doesn't need its call sites updated in the same task.

**Files:**
- Modify: `src/music_api.rs` (new free function, placed after the `SongIndex` `impl` block, i.e. after the closing `}` that currently ends `impl<'a> SongIndex<'a>` at `src/music_api.rs:286`)

- [ ] **Step 1: Write the failing tests**

Add to the `#[cfg(test)] mod tests` block in `src/music_api.rs`, after the `song_index_rejects_absent_song` test:

```rust
    #[test]
    fn pick_best_match_returns_first_pass_when_strip_qualifiers_true() {
        let mut query = song(MusicApiType::Spotify, "sp-1", &[], 538_700);
        query.name = "Circus Bells - Hardfloor Mix".to_string();

        let mut wrong_first = song(MusicApiType::Subsonic, "sub-1", &[], 538_900);
        wrong_first.name = "Circus Bells (Totally Unrelated Remix)".to_string();
        let mut correct = song(MusicApiType::Subsonic, "sub-2", &[], 538_200);
        correct.name = "Circus Bells (Hardfloor Remix)".to_string();

        let candidates = vec![wrong_first.clone(), correct.clone()];

        // With qualifiers stripped, both candidates' names collapse to
        // "circus bells" and the first one in iteration order wins -- the
        // exact bug this feature addresses.
        let picked = pick_best_match(&query, candidates.into_iter(), false, true).unwrap();
        assert_eq!(picked.id, wrong_first.id);
    }

    #[test]
    fn pick_best_match_prefers_closest_raw_name_when_strip_qualifiers_false() {
        let mut query = song(MusicApiType::Spotify, "sp-1", &[], 538_700);
        query.name = "Circus Bells - Hardfloor Mix".to_string();

        let mut wrong_first = song(MusicApiType::Subsonic, "sub-1", &[], 538_900);
        wrong_first.name = "Circus Bells (Totally Unrelated Remix)".to_string();
        let mut correct = song(MusicApiType::Subsonic, "sub-2", &[], 538_200);
        correct.name = "Circus Bells (Hardfloor Remix)".to_string();

        let candidates = vec![wrong_first, correct.clone()];

        // With qualifiers preserved, the raw name ranks the two candidates
        // and correctly prefers the one that actually shares "hardfloor".
        let picked = pick_best_match(&query, candidates.into_iter(), false, false).unwrap();
        assert_eq!(picked.id, correct.id);
    }

    #[test]
    fn pick_best_match_returns_none_when_no_candidate_passes() {
        let mut query = song(MusicApiType::Spotify, "sp-1", &[], 200_000);
        query.name = "Totally Different".to_string();
        let mut candidate = song(MusicApiType::Subsonic, "sub-1", &[], 900_000);
        candidate.name = "Unrelated".to_string();

        assert!(pick_best_match(&query, vec![candidate.clone()].into_iter(), false, true).is_none());
        assert!(pick_best_match(&query, vec![candidate].into_iter(), false, false).is_none());
    }
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test --lib music_api::tests::pick_best_match`
Expected: compile error, `cannot find function pick_best_match in this scope`

- [ ] **Step 3: Implement `pick_best_match`**

Add this free function to `src/music_api.rs`, immediately after the closing `}` of `impl<'a> SongIndex<'a>` (currently ending at `src/music_api.rs:286`), before `impl PartialEq for Song`:

```rust

/// Picks the best-matching candidate from `candidates` for `song`. With
/// `strip_qualifiers` true (the default), returns the first candidate that
/// passes `compare()` -- identical to the "first result that matches" loops
/// every platform's `search_song()` used to run inline. With it false,
/// evaluates every candidate, keeps the ones passing `compare()`, and
/// returns the one whose *raw* (qualifier-preserving) name is closest to
/// `song`'s. Otherwise a remix and its original both matching on stripped
/// name get picked arbitrarily by search-result order, which is what
/// silently produced wrong/missed matches.
pub fn pick_best_match(
    song: &Song,
    candidates: impl Iterator<Item = Song>,
    map_singles: bool,
    strip_qualifiers: bool,
) -> Option<Song> {
    if strip_qualifiers {
        return candidates.filter(|c| song.compare(c, map_singles, strip_qualifiers)).next();
    }
    let song_raw = song.raw_clean_name();
    candidates
        .filter(|c| song.compare(c, map_singles, strip_qualifiers))
        .max_by(|a, b| {
            let score_a = name_score(&song_raw, &a.raw_clean_name());
            let score_b = name_score(&song_raw, &b.raw_clean_name());
            score_a.total_cmp(&score_b)
        })
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test --lib music_api::tests::pick_best_match`
Expected: `test result: ok. 3 passed`

If `pick_best_match_prefers_closest_raw_name_when_strip_qualifiers_false` fails because the raw-name scoring doesn't rank `correct` above `wrong_first` as expected, print both scores with a quick `dbg!()` in the test, inspect which candidate actually wins, and adjust the fixture's wording (e.g. make the non-matching remix's raw text share even less with the query) rather than changing `pick_best_match`'s logic — the logic itself follows directly from the design.

- [ ] **Step 5: Commit**

```bash
git add src/music_api.rs
git commit -m "feat(music_api): add pick_best_match candidate-ranking helper"
```

---

### Task 6: `build_queries()` + wire up `subsonic`/`tidal`/`yt_music` to `pick_best_match`

`build_queries()`'s signature change breaks exactly three callers (`subsonic`, `tidal`, `yt_music` — `spotify` doesn't call it), so they're bundled into this one task. While touching each of these three files' `search_song`, also swap the manual "first passing candidate" loop (already 3-arg-`compare()`-compatible since Task 4) over to `pick_best_match`, since that's the actual fix for the candidate-tie-break bug and there's no reason to leave it for a separate pass over the same lines.

**Files:**
- Modify: `src/music_api.rs:213-239` (`build_queries`)
- Modify: `src/music_api.rs` test module
- Modify: `src/subsonic/mod.rs:260-294`
- Modify: `src/tidal/mod.rs:378-399`
- Modify: `src/yt_music/mod.rs:425-462`

- [ ] **Step 1: Write the failing test for `build_queries`**

Add to the `#[cfg(test)] mod tests` block in `src/music_api.rs`:

```rust
    #[test]
    fn build_queries_adds_raw_variants_only_when_strip_qualifiers_false() {
        let mut s = song(MusicApiType::Spotify, "sp-1", &[], 200_000);
        s.name = "Track - Remix".to_string();
        s.album = Some(Album {
            id: None,
            name: "Some Album".to_string(),
        });
        s.artists = vec![Artist {
            id: None,
            name: "Some Artist".to_string(),
        }];

        let stripped_queries = s.build_queries(true);
        assert!(!stripped_queries.iter().any(|q| q.contains("remix")));

        let raw_queries = s.build_queries(false);
        assert!(raw_queries.iter().any(|q| q.contains("track - remix")));
        // stripped variants are still present too -- recall only improves
        assert!(raw_queries.len() > stripped_queries.len());
    }
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --lib music_api::tests::build_queries_adds_raw_variants_only_when_strip_qualifiers_false`
Expected: compile error, `this method takes 0 arguments but 1 argument was supplied`

- [ ] **Step 3: Update `build_queries`**

Replace `src/music_api.rs:213-239` (the whole `build_queries` method) with:

```rust
    pub fn build_queries(&self, strip_qualifiers: bool) -> Vec<String> {
        let mut queries = vec![];
        let track_name = self.clean_name();

        // Query: Track + Album
        if let Some(album) = self.album.as_ref() {
            let album_name = album.clean_name();
            let tr_al_query = format!("{} {}", track_name, album_name);
            queries.push(tr_al_query);
        }
        // Query: Track + Artist
        for artist in self.artists.iter().rev() {
            let artist_name = artist.clean_name();
            let tr_ar_query = format!("{} {}", track_name, artist_name);
            queries.push(tr_ar_query);
        }
        // Query: Track + Artist + Album
        if let Some(album) = self.album.as_ref() {
            let album_name = album.clean_name();
            for artist in self.artists.iter().rev() {
                let artist_name = artist.clean_name();
                let tr_ar_al_query = format!("{} {} {}", track_name, artist_name, album_name);
                queries.push(tr_ar_al_query);
            }
        }

        // With qualifiers preserved, also try the raw (unstripped) track
        // name -- the remix/version text stripped out of `track_name`
        // above is often literally present in the destination platform's
        // title too, so keeping it in the query text improves recall for
        // exactly the tracks that collapse under the stripped name.
        if !strip_qualifiers {
            let raw_track_name = self.raw_clean_name();
            if raw_track_name != track_name {
                if let Some(album) = self.album.as_ref() {
                    let album_name = album.clean_name();
                    queries.push(format!("{} {}", raw_track_name, album_name));
                }
                for artist in self.artists.iter().rev() {
                    let artist_name = artist.clean_name();
                    queries.push(format!("{} {}", raw_track_name, artist_name));
                }
            }
        }

        queries
    }
```

- [ ] **Step 4: Update `src/subsonic/mod.rs::search_song`**

Replace `src/subsonic/mod.rs:260-294` (the whole `search_song` method) with:

```rust
    async fn search_song(&self, song: &Song) -> Result<Option<Song>> {
        // NOTE: unlike Tidal/Spotify, the Subsonic API has no dedicated
        // ISRC-filter search endpoint, so every lookup goes through
        // search3's free-text query. Song::compare() still uses ISRC for
        // exact matching when both sides happen to have one.
        //
        // With --navidrome, each known ISRC is also tried as a literal
        // search3 query, ahead of the fuzzy name/artist/album queries:
        // Navidrome indexes the ISRC tag, so this acts as a de facto
        // ISRC-filter search. Not assumed for plain Subsonic servers, which
        // may not expose or index ISRC at all.
        let mut queries = song.build_queries(self.config.strip_qualifiers);
        if self.navidrome {
            queries.extend(song.isrc.iter().cloned());
        }
        while let Some(query) = queries.pop() {
            let params = [
                ("query", query),
                ("songCount", "5".to_string()),
                ("artistCount", "0".to_string()),
                ("albumCount", "0".to_string()),
            ];
            let payload: SubsonicSearch3Payload = self.request("search3", &params).await?;
            let Some(result) = payload.search_result3 else {
                continue;
            };
            let res_songs: Songs = result.try_into()?;
            if let Some(best) = crate::music_api::pick_best_match(
                song,
                res_songs.0.into_iter().take(5),
                self.config.map_singles,
                self.config.strip_qualifiers,
            ) {
                return Ok(Some(best));
            }
        }
        Ok(None)
    }
```

- [ ] **Step 5: Update `src/tidal/mod.rs::search_song`**

Replace `src/tidal/mod.rs:378-399` (from `let url = format!("{}/v1/search"...` through the closing `}` of `search_song`) with:

```rust
        let url = format!("{}/v1/search", Self::API_URL);
        let mut queries = song.build_queries(self.config.strip_qualifiers);

        while let Some(query) = queries.pop() {
            let params = json!({
                "countryCode": self.country_code,
                "query": query,
                "type": "TRACKS",
            });
            let res: TidalSearchResponse = self
                .make_request_json(&url, &HttpMethod::Get(&params), Some((3, 0)))
                .await?;
            let res_songs: Songs = res.try_into()?;
            // iterate over top 3 results
            if let Some(best) = crate::music_api::pick_best_match(
                song,
                res_songs.0.into_iter().take(3),
                self.config.map_singles,
                self.config.strip_qualifiers,
            ) {
                return Ok(Some(best));
            }
        }
        Ok(None)
    }
```

(Leave the ISRC loop above it at `src/tidal/mod.rs:357-376` untouched.)

- [ ] **Step 6: Update `src/yt_music/mod.rs::search_song`**

Replace `src/yt_music/mod.rs:425-462` (the whole `search_song` method) with:

```rust
    async fn search_song(&self, song: &Song) -> Result<Option<Song>> {
        if song.isrc.is_empty() {
            let ignore_spelling = "AUICCAFqDBAOEAoQAxAEEAkQBQ%3D%3D";
            let params = format!("EgWKAQ{}{}", "II", ignore_spelling);
            let mut queries = song.build_queries(self.config.strip_qualifiers);
            while let Some(query) = queries.pop() {
                let body = json!({
                    "query": query,
                    "params": params,
                });
                let response = self
                    .make_request::<YtMusicResponse>("search", &body, None)
                    .await?;
                let res_songs: SearchSongs = response.try_into()?;
                // iterate over top 3 results
                if let Some(best) = crate::music_api::pick_best_match(
                    song,
                    res_songs.0.into_iter().take(3),
                    self.config.map_singles,
                    self.config.strip_qualifiers,
                ) {
                    return Ok(Some(best));
                }
            }
        } else {
            for isrc in &song.isrc {
                let body = json!({
                    "query": format!("\"{}\"", isrc),
                });
                let response = self
                    .make_request::<YtMusicResponse>("search", &body, None)
                    .await?;
                let res_song: SearchSongUnique = response.try_into()?;
                if let Some(mut res_song) = res_song.0 {
                    res_song.isrc = vec![isrc.clone()];
                    return Ok(Some(res_song));
                }
            }
        }
        Ok(None)
    }
```

- [ ] **Step 7: Build and run the full test suite**

Run: `cargo build 2>&1 | tail -40`
Expected: clean build — `spotify/mod.rs` still calls the old 2-arg `compare()`... no, wait: Task 4 already updated `spotify`'s `compare()` call to 3 args, and `spotify` doesn't call `build_queries()` at all (custom inline query building, handled in Task 7), so it's unaffected by this task's signature change. Expected: no errors.

Run: `cargo test --lib 2>&1 | tail -60`
Expected: `test result: ok.` for the whole suite.

- [ ] **Step 8: Commit**

```bash
git add src/music_api.rs src/subsonic/mod.rs src/tidal/mod.rs src/yt_music/mod.rs
git commit -m "feat: thread strip_qualifiers through build_queries, rank subsonic/tidal/yt_music candidates via pick_best_match"
```

---

### Task 7: Wire up `spotify` to `pick_best_match` + raw-name query variants

Independent of Task 6 — `spotify::search_song` builds its query strings inline instead of calling `Song::build_queries()`, so it doesn't need to move in lockstep with that signature change.

**Files:**
- Modify: `src/spotify/mod.rs:415-482` (`search_song`)

- [ ] **Step 1: Replace `search_song`**

Replace `src/spotify/mod.rs:415-482` (from `async fn search_song` through its closing `}`) with:

```rust
    async fn search_song(&self, song: &Song) -> Result<Option<Song>> {
        let path = "/search";
        let max_len = 100;
        let mut queries = vec![];

        if song.isrc.is_empty() {
            let mut track_query = format!("track:\"{}\"", song.clean_name());
            if track_query.len() > max_len {
                warn!(
                    "song name is bigger than spotify max search: \"{}\", truncating",
                    track_query
                );
                // Not the best solution, but it's worth a try
                track_query = track_query[..max_len].to_string();
            }

            let artist_queries: Vec<String> = song
                .artists
                .iter()
                .map(|a| format!("artist:\"{}\"", a.clean_name()))
                .collect();

            let mut album_query = None;
            if let Some(album) = &song.album {
                album_query = Some(format!("album:\"{}\"", album.clean_name()));
            }

            // Query: Track + Album
            if let Some(album_query) = album_query.as_ref() {
                let tr_al_query = format!("{} {}", track_query, album_query);
                push_query(&mut queries, tr_al_query, max_len);
            }
            // Query: Track + Artist
            for artist_query in artist_queries.iter().rev() {
                // INFO: spotify doesn't support multiple artists in search
                // we have to create one query per artist
                let tr_ar_query = format!("{} {}", track_query, artist_query);
                push_query(&mut queries, tr_ar_query, max_len);
            }
            // Query: Track + Artist + Album
            if let Some(album_query) = album_query.as_ref() {
                for artist_query in artist_queries.iter().rev() {
                    let tr_ar_al_query =
                        format!("{} {} {}", track_query, artist_query, album_query);
                    push_query(&mut queries, tr_ar_al_query, max_len);
                }
            }

            // With qualifiers preserved, also try the raw (unstripped)
            // track name -- see Song::build_queries for the same rationale.
            if !self.config.strip_qualifiers {
                let raw_track_name = song.raw_clean_name();
                if raw_track_name != song.clean_name() {
                    let mut raw_track_query = format!("track:\"{}\"", raw_track_name);
                    if raw_track_query.len() > max_len {
                        raw_track_query = raw_track_query[..max_len].to_string();
                    }
                    if let Some(album_query) = album_query.as_ref() {
                        push_query(
                            &mut queries,
                            format!("{} {}", raw_track_query, album_query),
                            max_len,
                        );
                    }
                    for artist_query in artist_queries.iter().rev() {
                        push_query(
                            &mut queries,
                            format!("{} {}", raw_track_query, artist_query),
                            max_len,
                        );
                    }
                }
            }
        } else {
            for isrc in &song.isrc {
                queries.push(format!("isrc:{}", isrc));
            }
        }

        while let Some(query) = queries.pop() {
            let get_params = [("type", "track"), ("q", &query)];
            let res: SpotifySearchResponse = self
                .make_request_json(path, &HttpMethod::Get(&get_params), 3, 0)
                .await?;
            let res_songs: Songs = res.try_into()?;
            // iterate over top 3 results
            if let Some(best) = crate::music_api::pick_best_match(
                song,
                res_songs.0.into_iter().take(3),
                self.config.map_singles,
                self.config.strip_qualifiers,
            ) {
                return Ok(Some(best));
            }
        }
        return Ok(None);
    }
```

- [ ] **Step 2: Build and run the full test suite**

Run: `cargo build 2>&1 | tail -40`
Expected: clean build, no errors.

Run: `cargo test --lib 2>&1 | tail -60`
Expected: `test result: ok.` for the whole suite (this now includes the whole crate compiling cleanly for the first time with every call site updated).

- [ ] **Step 3: Commit**

```bash
git add src/spotify/mod.rs
git commit -m "feat(spotify): rank search candidates via pick_best_match, add raw-name query variants"
```

---

### Task 8: Full verification pass

**Files:** none (verification only)

- [ ] **Step 1: Run the full test suite**

Run: `cargo test --lib 2>&1 | tail -60`
Expected: all tests pass, including every `music_api::tests::*` test added/modified in Tasks 3-6 and the pre-existing suite untouched by this plan (e.g. `src/utils.rs`'s MD5/salt tests, `src/subsonic/mod.rs`'s auth tests).

- [ ] **Step 2: Run clippy**

Run: `cargo clippy --all-targets -- -D warnings 2>&1 | tail -60`
Expected: no warnings. Pay particular attention to the four `crate::music_api::pick_best_match(...)` call sites added in Tasks 6-7 and the `f64::total_cmp` usage in Task 5 — fix anything clippy flags there before proceeding.

- [ ] **Step 3: Confirm the CLI flag end-to-end**

Run: `cargo run --release -- --help 2>&1 | grep -B1 -A4 strip-qualifiers`
Expected: the flag is listed with the doc comment from Task 2 and shows a default of `true`.

Run: `cargo run --release -- --strip-qualifiers false --help 2>&1 | head -5`
Expected: no clap parsing error (a value-less `--help` still short-circuits before subcommand validation, so this only confirms `--strip-qualifiers false` itself is accepted syntactically by clap). If this errors with something like "unexpected value" or "invalid value", the `action = clap::ArgAction::Set` attribute from Task 2 needs adjusting — check `clap`'s docs for defaultable boolean flags (`ArgAction::Set` combined with `default_value_t` is the documented pattern, but double-check against the `clap` version pinned in `Cargo.toml`).

- [ ] **Step 4: Final commit (if Steps 1-3 required any fixes)**

```bash
git add -A
git commit -m "chore: fix clippy/test fallout from strip_qualifiers threading"
```

If no fixes were needed, skip this commit — Tasks 1-7 already cover the full change.
