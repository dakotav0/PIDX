use chrono::{NaiveDateTime, Utc};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::decay::FieldClass;

// ── EvidenceType ──────────────────────────────────────────────────────────────

/// The linguistic pattern that produced a piece of register evidence.
///
/// Each type maps to one or more `RegisterMetricName` dimensions via the
/// signal taxonomy. For example, `HedgingPhrase` feeds into `Hedging` (+1),
/// while `IronicUnderstatement` feeds into `Humor` (+1) — *not* `Hedging`,
/// even though it looks like hedging on the surface.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum EvidenceType {
    HedgingPhrase,
    DirectAssertion,
    QualificationClause,
    QuestionPattern,
    IronicUnderstatement,
    TechnicalRegister,
    CasualRegister,
    HumorMarker,
    AbstractFraming,
    ConcreteExample,
}

// ── RegisterMetricName ────────────────────────────────────────────────────────

/// Communication register dimensions.
///
/// Each is an independent evidence pool — a score of 5.0 means neutral
/// (no evidence), 0.0 is the minimum, 10.0 is max.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum RegisterMetricName {
    Formality,
    Directness,
    Hedging,
    Humor,
    Abstraction,
    /// Emotional warmth / expressiveness vs. affective neutrality.
    /// High affect (8+): warm, emotionally present language.
    /// Low affect (2−): detached, neutral, professionally distanced.
    Affect,
}

// ── Evidence ──────────────────────────────────────────────────────────────────

/// A single observed signal contributing to a communication register dimension.
///
/// Evidence is additive — unlike `Observation`, evidence items are never in
/// conflict. When a new `BridgePacket` arrives, its register evidence is
/// appended to the existing pool; no delta detection runs on it.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct Evidence {
    /// ISO 8601 timestamp (no timezone suffix, matching Python output).
    pub observed_at: String,
    pub session_ref: String,
    /// Which model or system produced this evidence item.
    pub orientation: String,
    pub evidence_type: EvidenceType,
    /// The actual phrase or pattern that was observed verbatim.
    pub raw: String,
    /// Which register dimension this evidence feeds.
    pub metric: RegisterMetricName,
    /// Directional contribution to the score: +1 (high), 0 (neutral), -1 (low).
    ///
    /// `i8` is the right primitive here — smallest signed integer. We'll add
    /// a validated newtype if we ever need to enforce the -1/0/+1 constraint
    /// at the type level, but that's premature for now.
    pub signal: i8,
    /// Recency/strength weight. Convention: 0.3 isolated, 0.6 repeated, 0.9 sustained.
    pub weight: f64,
    /// When true, this evidence item contributes its full weight regardless of age.
    pub decay_exempt: bool,
}

// ── RegisterMetric ────────────────────────────────────────────────────────────

/// Evidence pool for a single communication register dimension.
///
/// The score is **always computed at read-time** — never stored. This ensures
/// the score always reflects the current age of evidence. An empty pool
/// returns a neutral score of 5.0.
#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
pub struct RegisterMetric {
    pub evidence: Vec<Evidence>,
}

impl RegisterMetric {
    /// Compute the register score at a given point in time.
    ///
    /// Formula (mirrors Python exactly):
    ///   1. For each evidence item, compute a decay-adjusted weight:
    ///      w = item.weight × e^(−λ_register × days_elapsed)
    ///   2. Weighted average of signals:
    ///      raw = Σ(signal × w) / Σ(w)   → [-1.0, 1.0]
    ///   3. Map to [0.0, 10.0]:
    ///      score = (raw + 1.0) × 5.0
    ///
    /// Returns 5.0 (neutral) when the evidence pool is empty or total weight is zero.
    pub fn score(&self, as_of: Option<chrono::DateTime<Utc>>) -> f64 {
        if self.evidence.is_empty() {
            return 5.0;
        }

        let as_of = as_of.unwrap_or_else(Utc::now);
        let lam = FieldClass::Register.lambda();

        let mut total_weight = 0.0_f64;
        let mut weighted_signal = 0.0_f64;

        for e in &self.evidence {
            let decay = if e.decay_exempt {
                1.0
            } else {
                match NaiveDateTime::parse_from_str(&e.observed_at, "%Y-%m-%dT%H:%M:%S%.f") {
                    Ok(t) => {
                        let days = (as_of - t.and_utc()).num_seconds() as f64 / 86400.0;
                        (-lam * days).exp()
                    }
                    Err(_) => 1.0, // unparseable timestamp: treat as no decay
                }
            };

            let w = e.weight * decay;
            total_weight += w;
            weighted_signal += e.signal as f64 * w;
        }

        if total_weight == 0.0 {
            return 5.0;
        }

        let raw = weighted_signal / total_weight; // [-1.0, 1.0]
        // Round to 2 decimal places, matching Python's round(..., 2)
        (((raw + 1.0) * 5.0) * 100.0).round() / 100.0
    }

    /// Human-readable label for the current score.
    ///
    /// Returns `&'static str` — a string literal baked into the binary.
    /// This is idiomatic Rust for a fixed set of return values; no heap
    /// allocation needed.
    pub fn score_label(&self, as_of: Option<chrono::DateTime<Utc>>) -> &'static str {
        let s = self.score(as_of);
        if s >= 8.0 {
            "high"
        } else if s >= 6.0 {
            "moderate-high"
        } else if s >= 4.0 {
            "moderate"
        } else if s >= 2.0 {
            "moderate-low"
        } else {
            "low"
        }
    }
}
