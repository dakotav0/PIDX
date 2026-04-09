//! Traits for observation sources — the abstraction the Python engine never had.
//!
//! In the Python version, every ingest path passes raw `origination` and
//! `orientation` strings and calls a dict lookup ad-hoc. Here, any type that
//! can produce observations implements `IngestSource`, and the confidence
//! calculation is a *provided method* — free for every implementor.
//!
//! ## What this unlocks later
//!
//! When you build out ingestion, functions will look like:
//!
//! ```rust,ignore
//! fn ingest(source: &dyn IngestSource, observation: RawObservation) -> Observation { ... }
//! ```
//!
//! The `&dyn IngestSource` parameter (a "trait object") means you can pass any
//! concrete type — `BridgeFileSource`, `ActiveSessionSource`, `UserInputSource` —
//! and the function works on all of them without needing generics. The compiler
//! enforces at the call site that the argument implements the trait.

use crate::models::confidence::{get_base_confidence, Origination};

/// A source that can produce profile observations.
///
/// Implement this for each ingest path. The `base_confidence()` method has a
/// default implementation derived from the confidence matrix, so you only need
/// to define the three required methods.
///
/// ## Example
///
/// ```rust,ignore
/// struct BridgeFileSource {
///     orientation: String,
///     session_ref: String,
/// }
///
/// impl IngestSource for BridgeFileSource {
///     fn origination(&self) -> Origination { Origination::Passive }
///     fn orientation(&self) -> &str { &self.orientation }
///     fn session_ref(&self) -> &str { &self.session_ref }
/// }
///
/// // base_confidence() is free — returns 0.61 for Passive × "local:..."
/// let confidence = source.base_confidence();
/// ```
pub trait IngestSource {
    /// Where this observation originated (user, active AI, passive AI, etc.).
    fn origination(&self) -> Origination;

    /// Which system or model produced it (e.g. "claude.sonnet-4-6", "local:gemma3:4b").
    fn orientation(&self) -> &str;

    /// Session identifier for audit trail linkage.
    fn session_ref(&self) -> &str;

    /// Base confidence for observations from this source.
    ///
    /// This is a *provided method* — you get it for free when you implement the
    /// three required methods above. It calls the confidence matrix lookup so
    /// you never have to do it manually in each ingest path.
    ///
    /// Override this only if a specific source has non-standard confidence rules.
    fn base_confidence(&self) -> f64 {
        get_base_confidence(self.origination(), self.orientation())
    }
}
