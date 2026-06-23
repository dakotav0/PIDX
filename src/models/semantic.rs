//! Semantic similarity annotation layer for bridge-diff matching.
//!
//! When exact match fails, a model-in-the-loop can annotate observations with
//! similarity hints. The reinforcement engine uses these hints to match
//! semantically equivalent observations that differ in phrasing.
//!
//! # Design
//!
//! - **Off by default** — exact match is the primary mechanism. Semantic
//!   similarity is opt-in via `SemanticHints` in the bridge packet.
//! - **Hints are per-field** — each field path can carry a list of value pairs
//!   that the model considers semantically equivalent despite differing text.
//! - **Not automatable without a model** — the engine never infers similarity.
//!   A model must explicitly annotate pairs.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

// ── SemanticHints ────────────────────────────────────────────────────────────

/// Per-field semantic equivalence hints from a model-in-the-loop.
///
/// Each entry maps a field path to a list of `(canonical_value, variant_value)`
/// pairs. When the engine encounters `variant_value` in a new packet and
/// `canonical_value` exists as a confirmed observation, it treats them as
/// a reinforcement match.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SemanticHints {
    /// Field path → list of (canonical, variant) pairs.
    pub equivalences: HashMap<String, Vec<SemanticPair>>,
}

/// A pair of semantically equivalent observation values.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SemanticPair {
    /// The value that already exists as a confirmed observation.
    pub canonical: String,
    /// The variant that should match against the canonical.
    pub variant: String,
}

// ── Matching ─────────────────────────────────────────────────────────────────

impl SemanticHints {
    /// Check whether two text values are semantically equivalent according to
    /// the hints for the given field path.
    ///
    /// Returns `true` if the pair (or its reverse) appears in the equivalences.
    pub fn are_equivalent(&self, field_path: &str, a: &str, b: &str) -> bool {
        if let Some(pairs) = self.equivalences.get(field_path) {
            let a_lower = a.trim().to_lowercase();
            let b_lower = b.trim().to_lowercase();
            for pair in pairs {
                let c = pair.canonical.trim().to_lowercase();
                let v = pair.variant.trim().to_lowercase();
                if (a_lower == c && b_lower == v) || (a_lower == v && b_lower == c) {
                    return true;
                }
            }
        }
        false
    }

    /// Merge another set of hints into this one (union per field).
    pub fn merge(&mut self, other: &SemanticHints) {
        for (field, pairs) in &other.equivalences {
            self.equivalences
                .entry(field.clone())
                .or_default()
                .extend(pairs.clone());
        }
    }
}

// ── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exact_pair_matches() {
        let mut hints = SemanticHints::default();
        hints.equivalences.insert(
            "working.mode".into(),
            vec![SemanticPair {
                canonical: "sketch-first".into(),
                variant: "prototype-driven".into(),
            }],
        );
        assert!(hints.are_equivalent("working.mode", "sketch-first", "prototype-driven"));
    }

    #[test]
    fn reverse_pair_matches() {
        let mut hints = SemanticHints::default();
        hints.equivalences.insert(
            "working.mode".into(),
            vec![SemanticPair {
                canonical: "sketch-first".into(),
                variant: "prototype-driven".into(),
            }],
        );
        assert!(hints.are_equivalent("working.mode", "prototype-driven", "sketch-first"));
    }

    #[test]
    fn case_insensitive_match() {
        let mut hints = SemanticHints::default();
        hints.equivalences.insert(
            "working.mode".into(),
            vec![SemanticPair {
                canonical: "Sketch-First".into(),
                variant: "PROTOTYPE-driven".into(),
            }],
        );
        assert!(hints.are_equivalent("working.mode", "sketch-first", "prototype-driven"));
    }

    #[test]
    fn no_match_without_hints() {
        let hints = SemanticHints::default();
        assert!(!hints.are_equivalent("working.mode", "sketch-first", "spec-first"));
    }

    #[test]
    fn merge_combines_fields() {
        let mut a = SemanticHints::default();
        a.equivalences.insert(
            "working.mode".into(),
            vec![SemanticPair {
                canonical: "a".into(),
                variant: "b".into(),
            }],
        );

        let mut b = SemanticHints::default();
        b.equivalences.insert(
            "signals.phrases".into(),
            vec![SemanticPair {
                canonical: "x".into(),
                variant: "y".into(),
            }],
        );

        a.merge(&b);
        assert!(a.are_equivalent("working.mode", "a", "b"));
        assert!(a.are_equivalent("signals.phrases", "x", "y"));
    }
}
