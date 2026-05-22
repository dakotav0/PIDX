use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::confidence::Origination;

// ── BridgeOrigination ─────────────────────────────────────────────────────────

/// Origination values a bridge packet is permitted to carry.
///
/// v0.1 packets could only use `passive` or `sync`. v0.2 narrative-analyst
/// packets use `active` (structured elicitation), which maps to the
/// `Active × claude.*` → 0.91 row in the confidence matrix.
///
/// `user` origination is still forbidden at the bridge layer — only the
/// user themselves can set that via CLI/MCP annotate.
///
/// Deserialization falls back to `Passive` for any unrecognized string rather
/// than hard-failing the entire packet.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum BridgeOrigination {
    Active,
    Passive,
    Sync,
}

impl BridgeOrigination {
    pub fn default_passive() -> BridgeOrigination {
        BridgeOrigination::Passive
    }

    pub fn deserialize_with_fallback<'de, D>(d: D) -> Result<BridgeOrigination, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let s = String::deserialize(d).unwrap_or_default();
        Ok(match s.as_str() {
            "active" => BridgeOrigination::Active,
            "sync"   => BridgeOrigination::Sync,
            _        => BridgeOrigination::Passive,
        })
    }
}

impl From<BridgeOrigination> for Origination {
    fn from(b: BridgeOrigination) -> Origination {
        match b {
            BridgeOrigination::Active  => Origination::Active,
            BridgeOrigination::Passive => Origination::Passive,
            BridgeOrigination::Sync => Origination::Sync,
        }
    }
}

// ── BridgeSource (v0.2) ───────────────────────────────────────────────────────

/// Source metadata carried in v0.2 packets — analogous to the flat
/// `orientation`/`session_ref`/`timestamp` fields of v0.1 but grouped and
/// extended with a `type` discriminant and per-observation origination.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BridgeSource {
    /// Classifier for the observation mechanism, e.g. `"session_analysis"`.
    #[serde(rename = "type", default)]
    pub source_type: String,
    #[serde(
        deserialize_with = "BridgeOrigination::deserialize_with_fallback",
        default = "BridgeOrigination::default_passive"
    )]
    pub origination: BridgeOrigination,
    pub orientation: String,
    pub session_ref: String,
    pub timestamp: String,
}

// ── BridgeObservation (v0.1) ──────────────────────────────────────────────────

/// Single observation in a v0.1 flat-array packet.
///
/// Unknown top-level keys are silently ignored — no `#[serde(deny_unknown_fields)]`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BridgeObservation {
    /// Dot-path to the target field, e.g. `"identity.core"`, `"signals.phrases"`.
    pub field: String,
    pub value: Value,
    #[serde(
        deserialize_with = "BridgeOrigination::deserialize_with_fallback",
        default = "BridgeOrigination::default_passive"
    )]
    pub origination: BridgeOrigination,
    pub raw: Option<String>,
}

// ── BridgeObservationV2 (v0.2) ────────────────────────────────────────────────

/// Observation entry inside the v0.2 field-keyed `observations_proposed` map.
///
/// `confidence`, `weight`, `status`, `revision`, `decay_exempt` are deserialized
/// and silently dropped — the engine always computes these server-side (axiom 2).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BridgeObservationV2 {
    pub value: Value,
    pub source: BridgeSource,
}

// ── BridgeDeltaFlags ──────────────────────────────────────────────────────────

/// Hints from the source about which existing observations to act on.
///
/// `confirm`: field prefixes whose proposed observations the source believes
///   are safe to auto-confirm. Treated like `confirm_all_proposed(prefix)`.
/// `revise` / `deprecate`: logged as intent but not auto-actioned (trust boundary).
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct BridgeDeltaFlags {
    #[serde(default)]
    pub confirm: Vec<String>,
    #[serde(default)]
    pub revise: Vec<String>,
    #[serde(default)]
    pub deprecate: Vec<String>,
}

// ── BridgeDyadicNotes ─────────────────────────────────────────────────────────

/// Relational metadata about a specific pairing between two profiles.
///
/// Stored as a decay-exempt annotation on the target profile rather than as a
/// first-class schema field — a dedicated `dyadic` document type is deferred to v0.3.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BridgeDyadicNotes {
    /// Pairing identifier, e.g. `"dakota–naomi"`.
    pub pairing: String,
    #[serde(default)]
    pub complementarity_finding: String,
    pub risk_flag: Option<String>,
}

// ── BridgePacket ──────────────────────────────────────────────────────────────

/// Inbound packet from a model session. Supports both the v0.1 flat format
/// and the v0.2 structured format produced by narrative-analyst orientations.
///
/// ## Backward compatibility
///
/// A v0.1 packet has `bridge_version`, flat `orientation`/`session_ref`/`timestamp`,
/// and an `observations` array. All of those fields continue to work unchanged.
///
/// A v0.2 packet uses `bridge_format_version`, a nested `source` object, and an
/// `observations_proposed` field-keyed map. Both formats can coexist in one packet
/// (edge case: a migrating producer that sends both arrays).
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct BridgePacket {
    /// Renamed from `bridge_version` in v0.2. Both names are accepted.
    #[serde(alias = "bridge_version", default = "default_bridge_format_version")]
    pub bridge_format_version: String,

    // ── v0.2 nested source ────────────────────────────────────────────────────
    /// Source metadata object. When present, overrides the flat fields below.
    #[serde(default)]
    pub source: Option<BridgeSource>,

    // ── v0.1 flat source fields (also fallback when `source` is absent) ───────
    #[serde(default)]
    pub orientation: String,
    #[serde(default)]
    pub session_ref: String,
    #[serde(default)]
    pub timestamp: String,

    // ── Routing ───────────────────────────────────────────────────────────────
    /// When set, the engine uses this as the profile ID without requiring an
    /// explicit `user_id` argument from the caller.
    pub target_profile: Option<String>,

    /// Informational version stamps — stored in the bridge log entry, no
    /// optimistic-locking behavior.
    pub target_version: Option<String>,
    pub previous_version: Option<String>,

    // ── Observations ──────────────────────────────────────────────────────────
    /// v0.1: flat array with per-observation `field` and `origination`.
    #[serde(default)]
    pub observations: Vec<BridgeObservation>,

    /// v0.2: field-keyed map; each observation carries its own `source`.
    #[serde(default)]
    pub observations_proposed: HashMap<String, Vec<BridgeObservationV2>>,

    // ── Delta / dyadic metadata ───────────────────────────────────────────────
    /// Source hints about which prefixes to auto-confirm after ingestion.
    #[serde(default)]
    pub deltas_flagged: Option<BridgeDeltaFlags>,

    /// Relational metadata about a profile pairing; stored as an annotation.
    #[serde(default)]
    pub dyadic_notes: Option<BridgeDyadicNotes>,
}

impl BridgePacket {
    /// Orientation — from `source.orientation` if v0.2, else flat `orientation`.
    pub fn effective_orientation(&self) -> &str {
        self.source.as_ref().map_or(self.orientation.as_str(), |s| s.orientation.as_str())
    }

    /// Session reference.
    pub fn effective_session_ref(&self) -> &str {
        self.source.as_ref().map_or(self.session_ref.as_str(), |s| s.session_ref.as_str())
    }

    /// ISO 8601 timestamp.
    pub fn effective_timestamp(&self) -> &str {
        self.source.as_ref().map_or(self.timestamp.as_str(), |s| s.timestamp.as_str())
    }
}

fn default_bridge_format_version() -> String {
    "0.1".to_string()
}
