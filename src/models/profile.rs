use chrono::{DateTime, Utc};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::decay::FieldClass;
use super::evidence::RegisterMetric;
use super::observation::{Observation, ObservationField, ObservationStatus};

// ── Serde default helpers ─────────────────────────────────────────────────────

fn default_semver() -> String {
    "0.1.0".to_string()
}
fn default_threshold() -> f64 {
    0.20
}
fn default_cleanup_mode() -> String {
    "prompted".to_string()
}
fn default_cadence() -> String {
    "session".to_string()
}
fn default_author() -> String {
    "system".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(untagged)]
pub enum ProfileWrapper {
    Npc(Box<crate::models::miin_profile::MiinProfileDocument>),
    Human(Box<ProfileDocument>),
}

impl ProfileWrapper {
    pub fn meta(&self) -> &ProfileMeta {
        match self {
            Self::Human(p) => &p.meta,
            Self::Npc(p) => &p.meta,
        }
    }
    pub fn meta_mut(&mut self) -> &mut ProfileMeta {
        match self {
            Self::Human(p) => &mut p.meta,
            Self::Npc(p) => &mut p.meta,
        }
    }
    pub fn delta_queue(&self) -> &[DeltaItem] {
        match self {
            Self::Human(p) => &p.delta_queue,
            Self::Npc(p) => &p.delta_queue,
        }
    }
    pub fn delta_queue_mut(&mut self) -> &mut Vec<DeltaItem> {
        match self {
            Self::Human(p) => &mut p.delta_queue,
            Self::Npc(p) => &mut p.delta_queue,
        }
    }
    pub fn review_queue(&self) -> &[ReviewItem] {
        match self {
            Self::Human(p) => &p.review_queue,
            Self::Npc(p) => &p.review_queue,
        }
    }
    pub fn review_queue_mut(&mut self) -> &mut Vec<ReviewItem> {
        match self {
            Self::Human(p) => &mut p.review_queue,
            Self::Npc(p) => &mut p.review_queue,
        }
    }
    pub fn bridge_log(&self) -> &BridgeLog {
        match self {
            Self::Human(p) => &p.bridge_log,
            Self::Npc(p) => &p.bridge_log,
        }
    }
    pub fn bridge_log_mut(&mut self) -> &mut BridgeLog {
        match self {
            Self::Human(p) => &mut p.bridge_log,
            Self::Npc(p) => &mut p.bridge_log,
        }
    }
    pub fn annotations(&self) -> &[Annotation] {
        match self {
            Self::Human(p) => &p.annotations,
            Self::Npc(p) => &p.annotations,
        }
    }
    pub fn annotations_mut(&mut self) -> &mut Vec<Annotation> {
        match self {
            Self::Human(p) => &mut p.annotations,
            Self::Npc(p) => &mut p.annotations,
        }
    }
    pub fn recompute_overall_confidence(&mut self) {
        match self {
            Self::Human(p) => p.recompute_overall_confidence(),
            Self::Npc(p) => p.recompute_overall_confidence(),
        }
    }
}

// ── CleanupPolicy ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct CleanupPolicy {
    #[serde(default = "default_threshold")]
    pub threshold: f64,
    /// "prompted" | "background"
    #[serde(default = "default_cleanup_mode")]
    pub mode: String,
    /// "event" | "session" | "weekly" | "monthly"
    #[serde(default = "default_cadence")]
    pub cadence: String,
}

impl Default for CleanupPolicy {
    fn default() -> Self {
        Self {
            threshold: 0.20,
            mode: "prompted".to_string(),
            cadence: "session".to_string(),
        }
    }
}

// ── ProfileMeta ───────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ProfileMeta {
    pub id: String,
    #[serde(default = "default_semver")]
    pub version: String,
    #[serde(default = "default_semver")]
    pub schema_version: String,
    /// Timestamp of profile creation. Written as RFC 3339 UTC by Rust
    /// (`"2026-04-07T06:45:37.123456+00:00"`). Python-generated profiles use
    /// naive ISO 8601 (`"2026-04-07T00:45:37.688857"`); both are valid on read.
    #[serde(default = "ProfileMeta::now_utc")]
    pub created: String,
    #[serde(default = "ProfileMeta::now_utc")]
    pub updated: String,
    #[serde(default)]
    pub cleanup_policy: CleanupPolicy,
    #[serde(default)]
    pub overall_confidence: f64,
}

impl ProfileMeta {
    pub fn new(id: impl Into<String>) -> Self {
        let now = Self::now_utc();
        Self {
            id: id.into(),
            version: default_semver(),
            schema_version: default_semver(),
            created: now.clone(),
            updated: now,
            cleanup_policy: CleanupPolicy::default(),
            overall_confidence: 0.0,
        }
    }

    /// Current UTC time as an RFC 3339 string. Used as a serde default and
    /// internally by bump_version(). The explicit `+00:00` offset removes all
    /// timezone ambiguity — the fix for the CST/UTC drift in Python profiles.
    pub fn now_utc() -> String {
        Utc::now().to_rfc3339()
    }

    /// Increment the patch segment of the semver version string and refresh
    /// the `updated` timestamp.
    ///
    /// `"0.1.0"` → `"0.1.1"`. If the version string doesn't parse as semver,
    /// the version is left unchanged but `updated` is still refreshed.
    pub fn bump_version(&mut self) {
        let parts: Vec<&str> = self.version.splitn(3, '.').collect();
        if parts.len() == 3 {
            if let Ok(patch) = parts[2].parse::<u32>() {
                self.version = format!("{}.{}.{}", parts[0], parts[1], patch + 1);
            }
        }
        self.updated = Self::now_utc();
    }
}

// ── Delta & review queues ─────────────────────────────────────────────────────

/// An unresolved conflict between two observations for the same field.
///
/// The field is inert (neither `a` nor `b` is used as the active value) until
/// the conflict is explicitly resolved by the user or engine.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct DeltaItem {
    pub id: String,
    /// Dot-path to the conflicting field (e.g. `"identity.core[0]"`).
    pub field: String,
    pub a: Observation,
    pub b: Observation,
    #[serde(default = "ProfileMeta::now_utc")]
    pub created_at: String,
    #[serde(default)]
    pub resolved: bool,
}

/// An observation that has decayed below the cleanup threshold and is
/// awaiting user review before being archived or discarded.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ReviewItem {
    pub id: String,
    pub field: String,
    /// Index into the relevant `ObservationField.observations` list.
    pub observation_index: usize,
    pub effective_confidence: f64,
    #[serde(default = "ProfileMeta::now_utc")]
    pub flagged_at: String,
    #[serde(default)]
    pub resolved: bool,
}

// ── Annotation ────────────────────────────────────────────────────────────────

/// A user or system note attached to a specific profile field.
///
/// Annotations never decay. Pinned annotations appear in Rich-tier output.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct Annotation {
    pub id: String,
    pub field: String,
    pub note: String,
    /// `"user"` | `"system"`
    #[serde(default = "default_author")]
    pub author: String,
    #[serde(default = "ProfileMeta::now_utc")]
    pub created_at: String,
    #[serde(default)]
    pub pinned: bool,
}

// ── Bridge log ────────────────────────────────────────────────────────────────

/// Audit record for one processed bridge packet.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct BridgeLogEntry {
    pub filename: String,
    #[serde(default = "ProfileMeta::now_utc")]
    pub ingested_at: String,
    #[serde(default)]
    pub observations_proposed: u32,
    #[serde(default)]
    pub deltas_flagged: u32,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
pub struct BridgeLog {
    #[serde(default)]
    pub processed: Vec<BridgeLogEntry>,
    #[serde(default)]
    pub pending_filenames: Vec<String>,
}

// ── Identity ──────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
pub struct IdentityReasoning {
    #[serde(default)]
    pub style: ObservationField,
    #[serde(default)]
    pub pattern: ObservationField,
    #[serde(default)]
    pub intake: ObservationField,
    /// Default epistemic/affective orientation toward uncertainty.
    /// Singleton — conflicting observations trigger delta detection.
    /// Examples: "skeptical-by-default", "curious-first", "deferential until evidence".
    #[serde(default)]
    pub stance: ObservationField,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
pub struct Identity {
    /// Each element tracks one distinct core personality trait.
    #[serde(default)]
    pub core: Vec<ObservationField>,
    #[serde(default)]
    pub reasoning: IdentityReasoning,
}

// ── Register ──────────────────────────────────────────────────────────────────

/// Six communication register dimensions, each backed by an evidence pool.
///
/// Serializes as `"comm"` in JSON (the Python field name, not its alias).
#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
pub struct Register {
    #[serde(default)]
    pub formality: RegisterMetric,
    #[serde(default)]
    pub directness: RegisterMetric,
    #[serde(default)]
    pub hedging: RegisterMetric,
    #[serde(default)]
    pub humor: RegisterMetric,
    #[serde(default)]
    pub abstraction: RegisterMetric,
    /// Emotional warmth / expressiveness vs. affective neutrality.
    /// High affect (8+): warm, emotionally present language.
    /// Low affect (2−): detached, neutral, professionally distanced.
    #[serde(default)]
    pub affect: RegisterMetric,
}

// ── Signals ───────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
pub struct Signals {
    #[serde(default)]
    pub phrases: Vec<ObservationField>,
    #[serde(default)]
    pub avoidances: Vec<ObservationField>,
    #[serde(default)]
    pub rhythms: Vec<ObservationField>,
    /// Conceptual scaffolds the user reaches for — systems-first, narrative-first,
    /// empirical-first, relational-first. Different in kind from surface phrases.
    #[serde(default)]
    pub framings: Vec<ObservationField>,
}

// ── Working style ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
pub struct Working {
    #[serde(default)]
    pub mode: ObservationField,
    #[serde(default)]
    pub pace: ObservationField,
    #[serde(default)]
    pub feedback: ObservationField,
    #[serde(default)]
    pub pattern: ObservationField,
}

// ── ProfileDocument ───────────────────────────────────────────────────────────

/// The complete PIDX profile for one user.
///
/// This is the root struct that serializes to/from `{user_id}.pidx.json`.
/// All sub-fields default to empty so a profile with only `meta` populated
/// deserializes cleanly.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ProfileDocument {
    pub meta: ProfileMeta,
    #[serde(default)]
    pub identity: Identity,
    /// Communication register. JSON key is `"comm"` — the Python attribute name,
    /// not its Pydantic alias (`"register"`). Both names exist in Python but
    /// `model_dump_json()` writes the attribute name, so saved files use `"comm"`.
    #[serde(default)]
    pub comm: Register,
    #[serde(default)]
    pub domains: Vec<ObservationField>,
    #[serde(default)]
    pub values: Vec<ObservationField>,
    #[serde(default)]
    pub signals: Signals,
    #[serde(default)]
    pub working: Working,
    #[serde(default)]
    pub annotations: Vec<Annotation>,
    #[serde(default)]
    pub delta_queue: Vec<DeltaItem>,
    #[serde(default)]
    pub review_queue: Vec<ReviewItem>,
    #[serde(default)]
    pub bridge_log: BridgeLog,
}

impl ProfileDocument {
    /// Create a blank profile for the given user id.
    ///
    /// Timestamps are written as RFC 3339 UTC — unambiguous, unlike the naive
    /// ISO 8601 strings Python wrote. Existing Python profiles still load fine.
    pub fn new(id: impl Into<String>) -> Self {
        Self {
            meta: ProfileMeta::new(id),
            identity: Identity::default(),
            comm: Register::default(),
            domains: Vec::new(),
            values: Vec::new(),
            signals: Signals::default(),
            working: Working::default(),
            annotations: Vec::new(),
            delta_queue: Vec::new(),
            review_queue: Vec::new(),
            bridge_log: BridgeLog::default(),
        }
    }

    /// Recompute `meta.overall_confidence` as the mean effective confidence
    /// of all confirmed observations across the profile.
    ///
    /// Register evidence does not contribute — register scores are computed at
    /// read-time, never stored. Mirrors `profile.py:160`.
    pub fn recompute_overall_confidence(&mut self) {
        let now = Utc::now();
        let mut scores: Vec<f64> = Vec::new();

        // Nested function — takes an explicit slice rather than closing over anything,
        // which keeps the borrow checker happy when we later mutate self.meta.
        fn collect(
            fields: &[ObservationField],
            field_class: FieldClass,
            now: DateTime<Utc>,
            scores: &mut Vec<f64>,
        ) {
            for field in fields {
                for obs in &field.observations {
                    if obs.status == ObservationStatus::Confirmed {
                        scores.push(obs.effective_confidence(field_class, Some(now)));
                    }
                }
            }
        }

        collect(&self.identity.core, FieldClass::Identity, now, &mut scores);

        // Reasoning fields are individual ObservationFields, not a slice, so inline:
        for field in [
            &self.identity.reasoning.style,
            &self.identity.reasoning.pattern,
            &self.identity.reasoning.intake,
        ] {
            for obs in &field.observations {
                if obs.status == ObservationStatus::Confirmed {
                    scores.push(obs.effective_confidence(FieldClass::Identity, Some(now)));
                }
            }
        }

        collect(&self.domains, FieldClass::Domain, now, &mut scores);
        collect(&self.values, FieldClass::Value, now, &mut scores);
        collect(&self.signals.phrases, FieldClass::Signal, now, &mut scores);
        collect(&self.signals.avoidances, FieldClass::Signal, now, &mut scores);
        collect(&self.signals.rhythms, FieldClass::Signal, now, &mut scores);

        for field in [
            &self.working.mode,
            &self.working.pace,
            &self.working.feedback,
            &self.working.pattern,
        ] {
            for obs in &field.observations {
                if obs.status == ObservationStatus::Confirmed {
                    scores.push(obs.effective_confidence(FieldClass::Working, Some(now)));
                }
            }
        }

        self.meta.overall_confidence = if scores.is_empty() {
            0.0
        } else {
            let mean = scores.iter().sum::<f64>() / scores.len() as f64;
            // Round to 4 decimal places — matches Python's round(..., 4)
            (mean * 10_000.0).round() / 10_000.0
        };
    }
}
