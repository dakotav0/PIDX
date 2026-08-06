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
pub mod reads;
pub mod storage;
pub mod traits;

// ── Key type re-exports ───────────────────────────────────────────────────────
//
// Re-export the most commonly needed types at the crate root so consumers
// can write `use pidx::ProfileStore` instead of `use pidx::storage::ProfileStore`.

pub use ingestion::{
    confirm_all_proposed, ingest_and_reinforce, ingest_bridge_packet, reject_all_proposed,
    resolve_field_mut, run_corroboration, run_decay_pass,
};
pub use models::calibration::{derive_and_store_calibration, derive_calibration, CalibrationSeed};
pub use models::compaction::{compact_profile, CompactReport};
pub use models::pidx_type::PidxType;
pub use models::profile::ProfileDocument;
pub use models::reinforcement::{reinforce_after_ingest, ReinforcementConfig, ReinforcementResult};
pub use output::{render_tier_output, Tier};
pub use reads::{get_field_rows, list_observations, ObservationQuery, ObservationRow};
pub use storage::ProfileStore;
