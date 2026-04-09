use chrono::{DateTime, NaiveDateTime, Utc};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::confidence::Origination;
use super::decay::FieldClass;

// ── ObservationStatus ────────────────────────────────────────────────────────

/// Lifecycle state of a single observation.
///
/// The valid transitions are:
///   proposed → confirmed (user or engine accepts it)
///   confirmed → delta    (a conflicting observation arrives)
///   confirmed → archived (cleanup pass retires it)
///   any      → rejected  (permanent terminal state — never deleted)
///
/// `delta` means the field is inert: two observations conflict and the engine
/// is waiting for explicit resolution before using either value.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum ObservationStatus {
    Proposed,
    Confirmed,
    Rejected,
    Delta,
    Archived,
}

// ── ObservationSource ────────────────────────────────────────────────────────

/// Full provenance for a single observation — who made it, from where, and when.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ObservationSource {
    pub origination: Origination,
    /// Free-form orientation string: "claude.sonnet-4-6", "local:gemma3:4b",
    /// "algorithmic", "user", etc. The confidence matrix uses its prefix.
    pub orientation: String,
    /// SHA-derived session identifier for audit trail.
    pub session_ref: String,
    /// ISO 8601 timestamp string. Stored as a String to stay compatible with
    /// Python's `datetime.utcnow().isoformat()` output (no timezone suffix).
    pub timestamp: String,
}

// ── DomainEntry ──────────────────────────────────────────────────────────────

/// A domain expertise cluster observation value.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct DomainEntry {
    pub label: String,
    /// Relative weight of this domain in the overall cluster (0.0–1.0).
    /// Defaults to 0.60 on deserialize — matches `profile.py:121`.
    #[serde(default = "default_domain_weight")]
    pub weight: f64,
    /// Optional expertise tier. e.g. "beginner" | "intermediate" | "expert" | "architect".
    /// Absent in old profiles — omitted from JSON when None.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub proficiency: Option<String>,
}

fn default_domain_weight() -> f64 {
    0.60
}

// ── ObservationValue ─────────────────────────────────────────────────────────

/// All concrete types an observation value can hold.
///
/// ## Why an enum instead of a generic Observation<T>?
///
/// Python's `Observation[T]` works because Pydantic erases the generic at
/// runtime — all it needs is a JSON blob. Rust generics are monomorphized:
/// `Observation<String>` and `Observation<DomainEntry>` are different types
/// with no common base, so you can't store them in the same `Vec`.
///
/// An enum is the idiomatic Rust alternative. It's a "sum type" — a value
/// that is exactly one of its variants at runtime. The compiler knows every
/// possible shape, and a `match` on it is exhaustive: add a new variant here
/// and every `match ObservationValue { ... }` in the codebase becomes a
/// compile error until you handle it.
///
/// `#[serde(untagged)]` tells serde to serialize/deserialize the inner value
/// directly without a wrapper key. `Text("hello")` → `"hello"` in JSON.
/// `Domain(DomainEntry { .. })` → `{"label": "...", "weight": 1.0}`.
/// This matches the Python output exactly.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(untagged)]
pub enum ObservationValue {
    Text(String),
    Number(f64),
    Domain(DomainEntry),
}

// ── Observation ───────────────────────────────────────────────────────────────

/// An atomic unit of profile data with full provenance.
///
/// Every field in a profile is built from one or more of these. The confidence
/// is never just a number — it degrades over time based on the field class.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct Observation {
    pub value: ObservationValue,
    pub source: ObservationSource,
    /// Base confidence from the origination × orientation matrix.
    /// Use `effective_confidence()` at read-time — never store the computed value.
    pub confidence: f64,
    /// Field-class decay modifier (1.0 for most observations).
    pub weight: f64,
    pub status: ObservationStatus,
    /// Incremented each time this observation is updated in-place.
    pub revision: u32,
    /// When true, skip decay entirely. Always set for user-originated observations
    /// on identity and value fields.
    pub decay_exempt: bool,
}

impl Observation {
    /// Compute effective confidence at a given point in time.
    ///
    /// Applies exponential decay: `base × e^(−λ × days_elapsed)`.
    /// `decay_exempt` observations always return their base confidence unchanged.
    ///
    /// If `as_of` is `None`, the current UTC time is used.
    /// If the timestamp can't be parsed, falls back to base confidence (no decay).
    pub fn effective_confidence(
        &self,
        field_class: FieldClass,
        as_of: Option<chrono::DateTime<Utc>>,
    ) -> f64 {
        if self.decay_exempt {
            return self.confidence;
        }

        let as_of = as_of.unwrap_or_else(Utc::now);

        // Parse the stored ISO 8601 timestamp. Python writes these without a
        // timezone suffix (e.g. "2024-01-15T10:30:00.123456"), so we parse as
        // NaiveDateTime and treat it as UTC for the elapsed-days calculation.
        // Rust writes RFC 3339 with an offset ("2026-04-07T06:45:37+00:00");
        // Python writes naive ISO 8601 ("2026-04-07T06:45:37.123456").
        // Try RFC 3339 first so Rust-generated timestamps decay correctly.
        let obs_time: DateTime<Utc> = if let Ok(dt) =
            DateTime::parse_from_rfc3339(&self.source.timestamp)
        {
            dt.with_timezone(&Utc)
        } else if let Ok(naive) = NaiveDateTime::parse_from_str(
            &self.source.timestamp,
            "%Y-%m-%dT%H:%M:%S%.f",
        ) {
            naive.and_utc()
        } else {
            // Unrecognised format — return base confidence, don't decay.
            return self.confidence;
        };

        let days = (as_of - obs_time).num_seconds() as f64 / 86400.0;
        let lam = field_class.lambda();
        self.confidence * (-lam * days).exp()
    }
}

// ── ObservationField ──────────────────────────────────────────────────────────

/// A keyed collection of observations for a single profile field.
///
/// Multiple observations can exist for the same field (e.g. the same trait
/// observed in different sessions). Methods here operate over the collection
/// to surface the current active value or detect conflicts.
#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
pub struct ObservationField {
    pub observations: Vec<Observation>,
    /// How many times this specific value has been proposed across all bridge packets.
    /// Starts at 1 (the first proposal). Incremented instead of creating a duplicate
    /// slot when the same value arrives in a subsequent packet. Counts as a confidence
    /// signal: high repetition → higher salience, shown as "(×N)" in status output.
    #[serde(default = "default_proposal_count", skip_serializing_if = "is_one")]
    pub proposal_count: u32,
}

fn default_proposal_count() -> u32 { 1 }
fn is_one(n: &u32) -> bool { *n == 1 }

impl ObservationField {
    /// The current active value: the confirmed observation with the highest
    /// effective confidence at this moment.
    ///
    /// Returns `None` if there are no confirmed observations (e.g. all are
    /// proposed, delta, or rejected).
    pub fn active(&self, field_class: FieldClass) -> Option<&ObservationValue> {
        let now = Utc::now();
        self.observations
            .iter()
            .filter(|o| o.status == ObservationStatus::Confirmed)
            .max_by(|a, b| {
                let ca = a.effective_confidence(field_class, Some(now));
                let cb = b.effective_confidence(field_class, Some(now));
                // partial_cmp can return None for NaN; fall back to Equal so we
                // don't panic on bad data.
                ca.partial_cmp(&cb).unwrap_or(std::cmp::Ordering::Equal)
            })
            .map(|o| &o.value)
    }

    /// True if any observation in this field is in the `delta` state,
    /// meaning an unresolved conflict is blocking the field.
    pub fn is_in_delta(&self) -> bool {
        self.observations
            .iter()
            .any(|o| o.status == ObservationStatus::Delta)
    }

    /// The conflicting pair if this field is in delta, otherwise `None`.
    ///
    /// Returns references to the first two delta observations. The caller
    /// must resolve the conflict before the field becomes active again.
    pub fn delta_pair(&self) -> Option<[&Observation; 2]> {
        let deltas: Vec<&Observation> = self
            .observations
            .iter()
            .filter(|o| o.status == ObservationStatus::Delta)
            .collect();

        if deltas.len() >= 2 {
            Some([deltas[0], deltas[1]])
        } else {
            None
        }
    }

    /// Returns the maximum effective confidence among confirmed observations.
    pub fn overall_confidence(&self) -> f64 {
        // Since we don't know the field class here, we'll assume Identity (0.0005) 
        // as a stable baseline for confidence reporting, or just use the base confidence.
        // The Python engine uses the max base confidence for this metric.
        self.observations
            .iter()
            .filter(|o| o.status == ObservationStatus::Confirmed)
            .map(|o| o.confidence)
            .fold(0.0, f64::max)
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{
        confidence::Origination,
        decay::FieldClass,
    };

    fn make_obs(timestamp: &str) -> Observation {
        Observation {
            value: ObservationValue::Text("test".into()),
            source: ObservationSource {
                origination: Origination::Passive,
                orientation: "local:gemma3:4b".into(),
                session_ref: "s1".into(),
                timestamp: timestamp.to_string(),
            },
            confidence: 1.0,
            weight: 1.0,
            status: ObservationStatus::Confirmed,
            revision: 1,
            decay_exempt: false,
        }
    }

    /// Rust-written profiles use RFC 3339. Decay must apply — if this returns 1.0
    /// the timestamp wasn't parsed and decay silently skipped.
    #[test]
    fn rfc3339_timestamp_decays() {
        // A timestamp 1000 days in the past — signal λ=0.0200 should cut confidence
        // to e^(−0.02×1000) ≈ 2×10^−9, well below 1.0.
        let past = Utc::now() - chrono::Duration::days(1000);
        let ts = past.to_rfc3339();
        let obs = make_obs(&ts);

        let eff = obs.effective_confidence(FieldClass::Signal, None);
        assert!(eff < 0.01, "RFC 3339 timestamp should decay: got {eff}");
    }

    /// Python-written timestamps (naive ISO 8601) must still decay correctly.
    #[test]
    fn naive_iso8601_timestamp_decays() {
        let past = Utc::now() - chrono::Duration::days(1000);
        // Python format: no timezone suffix, microseconds
        let ts = past.format("%Y-%m-%dT%H:%M:%S%.6f").to_string();
        let obs = make_obs(&ts);

        let eff = obs.effective_confidence(FieldClass::Signal, None);
        assert!(eff < 0.01, "Naive ISO 8601 timestamp should decay: got {eff}");
    }

    /// decay_exempt observations always return base confidence, regardless of timestamp.
    #[test]
    fn decay_exempt_ignores_timestamp() {
        let past = Utc::now() - chrono::Duration::days(10_000);
        let ts = past.to_rfc3339();
        let mut obs = make_obs(&ts);
        obs.decay_exempt = true;

        let eff = obs.effective_confidence(FieldClass::Signal, None);
        assert!((eff - 1.0).abs() < f64::EPSILON);
    }
}
