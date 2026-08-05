# Spotify Multi-Market ISRC Enrichment — Design

## Goal

When a source Spotify "like" fails to find a destination match, retry once
with additional ISRCs discovered by re-querying the same track under a small
set of alternate Spotify markets, before giving up. Default sync behavior
(flag off) is unchanged.

## Background

Spotify's [track relinking](https://developer.spotify.com/documentation/web-api/concepts/track-relinking)
means the same nominal track, queried under a different `market`, can
resolve to a different regional release with its own `id` and
`external_ids.isrc` -- a different ISRC for what a listener would consider
the same recording. `ConfigArgs::diff_country`'s existing doc comment
already names this risk directly ("some songs will have different ISRC
codes" across countries), but no request in the codebase currently sends a
`market` query parameter at all: every Spotify lookup implicitly uses the
authenticated account's home market only.

`Song::compare()` already treats a mismatched (non-overlapping) ISRC as
non-conclusive rather than a hard rejection -- it falls through to
name/duration/album matching. So today's single-market ISRC is a *lost
opportunity*, not a correctness bug: exactly the kind of "should have
matched confidently via ISRC, but fell back to (and sometimes failed) fuzzy
matching instead" case documented by the `~/bin/compare-missing-likes.py`
spike and this session's `is_single()`/album-check fixes.

`GET /v1/tracks/{id}?market=XX` returns the same track-object JSON shape
already modeled by `SpotifySongResponse` (and reused everywhere else via
`impl TryInto<Song> for SpotifySongResponse`), so parsing is pure reuse.
`SpotifyApi::make_request_json` already retries on HTTP 429 using
Spotify's `Retry-After` header, so the new calls inherit rate-limit backoff
for free.

## Non-goals

- Playlists. `synchronize_playlists()` is untouched; this only wires into
  `synchronize_likes()`.
- Any platform other than Spotify implementing real enrichment. The `Song`
  ISRC-relinking-by-market quirk is Spotify-specific; other platforms get a
  no-op default trait method.
- Deriving probe markets from local-candidate ISRC country-code prefixes
  (a smarter follow-up, filed separately -- see project memory
  `project_isrc_enrichment_followup.md`). This design only covers a static,
  user-overridable market list.
- Caching enriched ISRCs across separate `sync` invocations. Each run
  re-derives them; at this scale (only the residual unmatched-like set,
  each run) the repeated cost is acceptable.

## Approach

### `src/lib.rs` -- new `ConfigArgs` fields

```rust
/// Attempt to discover additional ISRCs for a Spotify like that fails to
/// find a destination match, by re-querying the track under the markets
/// in --isrc-markets (Spotify's track relinking can serve a different
/// regional release -- and ISRC -- per market). Adds up to
/// `isrc_markets.len()` extra Spotify requests per unmatched like.
#[arg(long, default_value = "false")]
pub isrc_enrich: bool,

/// Markets to probe when --isrc-enrich is set, as a comma-separated list
/// of ISO 3166-1 alpha-2 country codes.
#[arg(long, value_delimiter = ',', default_value = "US,GB,DE,JP,BR")]
pub isrc_markets: Vec<String>,
```

Default market list picked for broad catalog/label-region coverage (North
America, UK, continental Europe, Japan, South America) without being
exhaustive -- user-overridable for anyone who knows their library skews
toward a specific region.

### `src/music_api.rs` -- new `MusicApi` trait method, default no-op

```rust
/// Attempts to discover additional ISRCs for `song` by re-querying under
/// alternate markets/storefronts. Populates `song.isrc` with any newly
/// found codes (existing codes are preserved; duplicates are skipped).
/// Default: no-op. Only Spotify currently has documented per-market ISRC
/// variance (track relinking); other platforms don't need this.
async fn enrich_isrc(&self, _song: &mut Song, _markets: &[String]) -> Result<()> {
    Ok(())
}
```

### `src/spotify/mod.rs` -- real implementation

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

Only the ISRC is merged in -- `song.name`/`song.album`/`song.duration_ms`
are left untouched, since the goal is strictly to widen the ISRC-overlap
signal for the *existing* song, not to trust a different region's metadata
for name/duration/album comparison.

### `src/sync.rs` -- integration point

Inside `synchronize_likes()`'s existing concurrent search closure (the
`stream::iter(to_search).map(|src_like| async move { ... }).buffered(config.search_concurrency)`
block), not as a separate pass -- this reuses the existing concurrency
bound instead of adding new unbounded sequential work:

```rust
.map(|src_like| async move {
    let mut result = dst_api.search_song(&src_like).await;
    if config.isrc_enrich
        && matches!(result, Ok(None))
    {
        let mut enriched = src_like.clone();
        if src_api.enrich_isrc(&mut enriched, &config.isrc_markets).await.is_ok()
            && enriched.isrc.len() > src_like.isrc.len()
        {
            result = dst_api.search_song(&enriched).await;
        }
    }
    (src_like, result)
})
```

Gather-then-retry-once: all configured markets are probed and merged into
one enriched `Song` first, then a single destination re-search is
attempted -- not one destination search per market. If `enrich_isrc`
itself errors (e.g. a transient HTTP failure), that's swallowed via
`.is_ok()` and the original (unmatched) result stands; enrichment is a
best-effort improvement, not a hard requirement for the sync to proceed.

## Testing

- No unit test for the trait's default no-op body: it's an unconditional
  `Ok(())` that doesn't touch `song`, true by inspection, and exercising it
  would mean stubbing a full `MusicApi` impl (~13 methods) for one
  assertion -- disproportionate to what there is to verify.
- No unit test for `SpotifyApi::enrich_isrc`'s HTTP call itself, consistent
  with the rest of the codebase's testing approach (network code isn't
  unit-tested; `make_request_json`, `search_song`, etc. have no direct
  unit tests either). Verified manually instead: `cargo run -- --help`
  shows both new flags with correct defaults, and `--isrc-markets` parses
  a comma-separated override correctly (e.g.
  `--isrc-markets US,CA` produces `["US", "CA"]`).
- Existing `src/sync.rs` has no unit tests today (network-dependent,
  no mock `MusicApi`); this stays consistent with that -- the new branch
  in `synchronize_likes()` is exercised via the same manual
  spotify-to-subsonic sync runs already used to validate this session's
  earlier fixes, not a new automated test.
