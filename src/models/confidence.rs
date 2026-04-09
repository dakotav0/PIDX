use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Where an observation came from — who or what generated it.
///
/// This is one axis of the confidence matrix. Combined with the orientation
/// (which system produced it), it determines the base confidence score.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum Origination {
    /// The user stated this directly about themselves.
    User,
    /// An AI model actively elicited this (structured prompting).
    Active,
    /// An AI model inferred this passively from conversation.
    Passive,
    /// Synced from a local model bridge session.
    Sync,
    /// Computed automatically (decay cleanup, version bumps, etc.).
    System,
}

/// Corroboration bonus applied when ≥2 independent orientations confirm the same value.
/// Each confirming observation gets +0.08, capped at 1.00.
pub const CORROBORATION_BONUS: f64 = 0.08;

/// Look up base confidence from the origination × orientation matrix.
///
/// `orientation` is a free-form string like `"claude.sonnet-4-6"`, `"local:gemma3:4b"`,
/// `"algorithmic"`, or `"user"`. The matrix matches on the prefix before the first `.`
/// or `:` separator — so `"claude.sonnet-4-6"` matches the `"claude"` row.
///
/// If no row matches, the fallback is 0.45 (system/unknown confidence floor).
///
/// ## The matrix
/// | Origination | Orientation prefix | Confidence |
/// |-------------|-------------------|------------|
/// | User        | user              | 1.00       |
/// | Active      | claude            | 0.91       |
/// | Passive     | claude            | 0.78       |
/// | Passive     | local             | 0.61       |
/// | Sync        | local             | 0.55       |
/// | System      | algorithmic       | 0.45       |
pub fn get_base_confidence(origination: Origination, orientation: &str) -> f64 {
    // Extract the family prefix: "claude.sonnet-4-6" → "claude", "local:gemma3:4b" → "local"
    let prefix = orientation
        .split(['.', ':'])
        .next()
        .unwrap_or(orientation);

    // Rust's match on a tuple is exhaustive — every (Origination, prefix) combination
    // not listed here falls through to the wildcard arm. The compiler guarantees we
    // can't accidentally miss a case we explicitly listed.
    match (origination, prefix) {
        (Origination::User,   "user")        => 1.00,
        (Origination::Active, "claude")      => 0.91,
        (Origination::Passive, "claude")     => 0.78,
        (Origination::Passive, "local")      => 0.61,
        (Origination::Sync,   "local")       => 0.55,
        (Origination::System, "algorithmic") => 0.45,
        _                                    => 0.45,
    }
}
