# Title-Qualifier-Aware Matching — Design

## Goal

Fix missed/incorrect song matches on multi-artist and remix/version tracks
without changing default behavior for existing users.

## Background

`generic_name_clean()` (`src/utils.rs`) and `Song::clean_name()`
(`src/music_api.rs`) unconditionally strip all `(...)`/`[...]` content plus
anything after `" - "`/`" pts. "`/`" feat. "`. This is needed to match
`"Track"` against `"Track (feat. X)"` across platforms whose feat.-credit
formatting varies, but it also erases genuinely distinguishing info like
`"(Hardfloor Remix)"` or `" - Yuksek Remix"` — a track and every one of its
remixes collapse to the same cleaned name. `Song::compare()` never compares
artists (their order/presence isn't consistent across platforms), so once
the name collapses, a ±1s duration window is the only remaining
disambiguator, and candidate-selection code (search-result loops,
`SongIndex`) just takes the first candidate that happens to pass — not
necessarily the right one.

This was confirmed against the user's library and `debug/missing_likes.json`:

- 150/568 missing likes have multiple Spotify artists (Spotify now returns
  remixers/collaborators as full artist credits); 79 have at least one
  artist already present locally.
- 3,419 same-artist/same-album groups (9,613 tracks) in the local library
  collapse to an identical cleaned name under today's stripping — e.g. Ian
  Brown's `"Love Like a Fountain"` has 7 different mixes that all clean to
  `"love like a fountain"`.
- Concretely confirmed real cases of already-owned tracks being missed
  because a same-cleaned-name sibling won the (unranked) candidate pick —
  e.g. `Alpha Beta Gaga (feat Rhymfest) - Mark Ronson remix` exactly matches
  a local track (0.0s duration diff) but wasn't surfaced.
- Local files are typically MusicBrainz/Picard-tagged, and MusicBrainz
  rarely credits a remixer as a separate recording artist (it's baked into
  the title instead), so Spotify's newer full-artist-list credits don't line
  up with local artist tags — meaning artist-based search-query fallbacks
  for the remix artist are mostly dead weight, on top of the name-collapsing
  problem above.

## Non-goals

- No change to default behavior. Everything here is opt-out via a new flag.
- No artist-based comparison in `compare()` — still out of scope, per the
  existing comment (artist order/presence isn't consistent across
  platforms).
- No changes to `Album::clean_name()`/album-name matching — scope is track
  titles only; album-qualifier collapsing (e.g. `"(Deluxe Edition)"`) is a
  possible future extension, not addressed here.
- No release-group/MusicBrainz-based matching — that was explored as a
  separate spike (`~/bin/compare-missing-likes.py --musicbrainz-stats`,
  outside this repo) and is not part of this change.

## Approach

New `--strip-qualifiers` CLI flag, **default `true`** (today's exact
behavior, unchanged). Passing `--strip-qualifiers false` keeps the raw
(qualifier-preserving) title available alongside the stripped one, and uses
both where it helps: as an additional signal in `compare()`'s name check,
in search queries, and to rank candidates when multiple tie on the stripped
name.

### `src/utils.rs`

Split `generic_name_clean()` into two layers:

```rust
pub fn normalize_name(name: &str) -> String {
    // today's lowercase + punctuation/accent replacement, only
}

pub fn generic_name_clean(name: &str) -> String {
    let name = normalize_name(name);
    // today's part_re + clean_enclosure('(',')') + clean_enclosure('[',']')
}
```

`generic_name_clean()`'s output is unchanged for all existing callers.
`normalize_name()` is new: basic-normalized, qualifiers intact.

### `src/music_api.rs`

- `Song::clean_name()` — unchanged, still used whenever `strip_qualifiers`
  is `true`.
- New `Song::raw_clean_name(&self) -> String` → `normalize_name(&self.name)`.
  The qualifier-preserving "unprocessed identifier."
- Extract `pub(crate) fn name_score(a: &str, b: &str) -> f64` (today's
  inline `normalized_levenshtein(...).abs()`), reused by `compare()` and the
  new ranking helper below.
- `compare(&self, other: &Self, map_singles: bool, strip_qualifiers: bool) -> bool`:
  - `strip_qualifiers == true`: name score computed exactly as today
    (`name_score(self.clean_name(), other.clean_name())`).
  - `strip_qualifiers == false`: `score = max(stripped_score, raw_score)`.
    Never stricter than today, so recall only improves; a track whose
    stripped names collide but whose raw names also happen to match closely
    gets a stronger (but not exclusive) signal, while cross-platform
    formatting noise still falls back to the stripped comparison.
  - Rest of `compare()` (ISRC, duration, album) unchanged.
- `build_queries(&self, strip_qualifiers: bool) -> Vec<String>`: when
  `false`, also emits query variants built from `raw_clean_name()` alongside
  the stripped-name ones, so a remix's actual title text reaches the search
  backend instead of being discarded before the query is even built.
- New:
  ```rust
  /// Picks the best-matching candidate from `candidates` for `song`.
  /// With `strip_qualifiers` true (the default), returns the first
  /// candidate that passes `compare()` — identical to today's behavior.
  /// With it false, evaluates every candidate, keeps the ones passing
  /// `compare()`, and returns the one whose *raw* (qualifier-preserving)
  /// name is closest to `song`'s. Otherwise a remix and its original both
  /// matching on stripped name get picked arbitrarily by search-result
  /// order, which is what silently produced wrong/missed matches.
  pub fn pick_best_match(
      song: &Song,
      candidates: impl Iterator<Item = Song>,
      map_singles: bool,
      strip_qualifiers: bool,
  ) -> Option<Song>
  ```
  Implementation: if `strip_qualifiers`, `candidates.filter(|c|
  song.compare(c, map_singles, strip_qualifiers)).next()`. Otherwise, filter
  the same way, then `max_by` on `name_score(song.raw_clean_name(),
  c.raw_clean_name())`.
- `SongIndex::contains(&self, song: &Song, map_singles: bool, strip_qualifiers: bool) -> bool`:
  threads the new param to `compare()`. No ranking change needed — it only
  returns a bool.
- `PartialEq for Song` keeps calling `compare(other, false, false)` — same
  as today's `map_singles` handling, same-source comparisons short-circuit
  on `id` equality before reaching the name checks, so the values passed
  are inconsequential.

### `src/lib.rs`

```rust
/// Strip remix/mix/version qualifiers and feat. credits from track titles
/// before comparing/searching (default: on, matching today's behavior).
/// Disabling this keeps the raw title available alongside the stripped
/// one, using both to avoid conflating a track with its remixes when they
/// collapse to the same stripped name -- at the cost of being pickier about
/// cross-platform title-formatting differences (e.g. feat.-credit lists)
/// that the stripped comparison used to paper over.
#[arg(long, default_value_t = true, action = clap::ArgAction::Set)]
pub strip_qualifiers: bool,
```

Follows the existing `ConfigArgs`/`map_singles` pattern, but needs
`ArgAction::Set` (not the bare presence-flag style used by the other
booleans here) since the default is `true` and users need to be able to
pass `false` explicitly: `--strip-qualifiers false`.

### Call sites

- `src/sync.rs`: thread `config.strip_qualifiers` through the 5
  `SongIndex::contains(...)` calls.
- `src/subsonic/mod.rs`, `src/tidal/mod.rs`, `src/yt_music/mod.rs`,
  `src/spotify/mod.rs`: each `search_song()` has a
  `for res_song in res_songs.take(N) { if song.compare(&res_song, ...) { return ... } }`
  loop (near-identical across all four). Replace each with a call to
  `music_api::pick_best_match(song, res_songs.into_iter().take(N),
  self.config.map_singles, self.config.strip_qualifiers)`, deduplicating
  four copies of the same loop into one shared implementation.
- `song.build_queries()` call sites (subsonic, tidal, yt_music each call it
  directly — it's a plain `Song` method, not a trait default; spotify builds
  its query strings inline instead of calling it) pass
  `self.config.strip_qualifiers`.

## Testing

- Update existing `compare`/`SongIndex` tests in `src/music_api.rs` for the
  new parameter (pass `true` to preserve today's assertions unchanged).
- New tests:
  - `strip_qualifiers = false` distinguishes two same-artist tracks that
    collide under stripped names (e.g. `"Track"` vs
    `"Track (X Remix)"`, differing durations) — `compare()` no longer
    treats them as interchangeable via the max-of-two-scores logic (the
    duration check still gates this, so assert on a duration-matching pair
    for the positive case and a duration-mismatched pair for the negative).
  - `pick_best_match` picks the raw-name-closest candidate among several
    stripped-name-tied candidates, both with `strip_qualifiers = true`
    (first-match-wins, unchanged) and `= false` (best-match-wins).
  - `build_queries` emits raw-name query variants only when
    `strip_qualifiers = false`.
  - `normalize_name`/`generic_name_clean` unit tests in `src/utils.rs`
    (`generic_name_clean` output unchanged; `normalize_name` preserves
    qualifiers).
