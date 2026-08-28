# AGENTS.md

Rust CLI (edition 2024, tokio) that syncs playlists/likes between Spotify, YouTube Music, Tidal and ListenBrainz, and exports/imports/merges them as JSON.

## Commands

- Build: `cargo build`
- Lint: `cargo clippy --all-targets` (clippy `pedantic` is **deny** — must pass clean)
- Format: `cargo fmt`
- Offline tests: `cargo test` (or `just test`)
- Live tests: `just test-live [filter]` — hits real accounts, needs `.env` + cached OAuth tokens
- Dev runs: see `justfile` (`d_*` recipes, e.g. `just d_sp2ti`)

## Layout

- `src/main.rs`, `args.rs`, `build_api.rs` — CLI only (clap). `<src> <dst>` chained subcommands; `merge` is standalone.
- `src/lib.rs` — library root + shared `ConfigArgs`.
- `src/music_api.rs` — `MusicApi` trait, `Song`/`Playlist`/`Album`/`Artist`, song matching (`Song::compare`).
- `src/sync.rs` — sync logic. `export.rs` / `import.rs` / `merge.rs` — JSON file ops.
- `src/utils.rs` — name/ISRC cleaning, dedup, debug JSON dumps.
- `src/<platform>/` — one per platform:
  - `mod.rs`: API client + `MusicApi` impl
  - `model.rs`: serde structs of raw API responses
  - `response.rs`: `TryInto` conversions from raw models to `music_api` types
- `tests/offline/` — deterministic tests (mock platform, sync, merge, matching).
- `tests/live/` — same contract suite on real platforms, all `#[ignore]`d.
- `tests/common/` — shared: `contract.rs` (trait contract suite), `mock.rs` (in-memory `MusicApi`), `fixtures.rs`, `live.rs`.

## Rules

- Never delete user data: sync only adds songs
- Match songs by ISRC first, then fuzzy fallback (name/album Levenshtein + duration). Don't use artist names for matching.
- Platform quirks: YtMusic has no ISRC; ListenBrainz has no duration; only Spotify/Tidal are region-locked.
- New platform: add `src/<name>/` with the 3-file split, a `MusicApiType` variant, CLI args in both `MusicPlatformSrc` and `MusicPlatformDst`, the `build_api.rs` arm, and register it in the contract/live tests.
- New `MusicApi` behavior: add a contract test in `tests/common/contract.rs` and keep `MockApi` in sync.
- Errors: `color_eyre` (`Result`, `eyre!`, `wrap_err`). Logging: `tracing`.
- Contract tests use `ensure!`/`bail!`, not `assert!`, so cleanup still runs.
- Never commit secrets or tokens: `.env`, `*.json` (headers, exports, debug dumps) are gitignored.
