//! PIDX — Personality Indexer library.
//!
//! This crate compiles as both a **library** (for Tauri commands, MCP server, and
//! test harnesses) and a **binary** (`pidx` CLI, via `src/main.rs`).
//!
//! # Public API surface
//!
//! Everything downstream consumers need is re-exported here. Tauri command handlers
//! and the MCP server import from `pidx::*` rather than digging into sub-modules.
//!
//! ## Rust lesson: lib + bin in one package
//!
//! When a Cargo package has both `src/lib.rs` and `src/main.rs`, Cargo emits two
//! build targets automatically:
//!
//! - `src/lib.rs` → the *library* crate (`pidx` as a dependency)
//! - `src/main.rs` → the *binary* crate (`pidx` CLI, links the library)
//!
//! The binary can use `crate::*` (intra-crate) or the library's public items
//! interchangeably. External crates (Tauri, MCP) import `pidx::*` from the library.
//! No workspace restructure needed at this stage — the single `Cargo.toml` handles
//! both targets.

#![allow(dead_code)]

pub mod ingestion;
pub mod models;
pub mod output;
pub mod storage;
pub mod traits;

// ── Key type re-exports ───────────────────────────────────────────────────────
//
// Re-export the most commonly needed types at the crate root so consumers
// can write `use pidx::ProfileStore` instead of `use pidx::storage::ProfileStore`.

pub use ingestion::{
    confirm_all_proposed, ingest_bridge_packet, reject_all_proposed, run_corroboration,
    run_decay_pass,
};
pub use models::profile::{ProfileDocument, ProfileWrapper};
pub use output::{render_tier_output, Tier};
pub use storage::ProfileStore;
