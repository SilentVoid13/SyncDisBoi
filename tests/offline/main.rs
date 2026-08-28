//! Deterministic tests that need no network: the contract suite against an
//! in-memory platform, and the platform-agnostic logic (sync, merge, song
//! matching, name cleaning). Run with `cargo test`.

#[path = "../common/mod.rs"]
mod common;

mod contract;
mod export_import;
mod merge;
mod song;
mod sync;
mod utils;
