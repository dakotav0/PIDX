use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::confidence::Origination;

// ── BridgeOrigination ─────────────────────────────────────────────────────────

/// The only origination values a bridge packet is permitted to carry.
///
/// Bridge packets come from local model sessions — they can never claim "active"
/// (structured elicitation) or "user" (self-report) origination. Using a separate
/// enum instead of `Origination` makes this constraint enforced at the type level:
/// you cannot accidentally construct a bridge observation with user-level confidence.
///
/// Deserialization is intentionally lenient: any unrecognized string falls back to
/// `Passive` rather than hard-failing. This prevents a hallucinated origination value
/// from aborting an entire packet ingest.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum BridgeOrigination {
    Passive,
    Sync,
}

impl BridgeOrigination {
    /// Serde default fn — used when `origination` is missing from the packet JSON.
    pub fn default_passive() -> BridgeOrigination {
        BridgeOrigination::Passive
    }

    /// Custom deserializer that falls back to Passive for any unknown/bad value.
    ///
    /// LLMs frequently hallucinate origination strings like "active" or "user".
    /// Rather than panicking on the first bad packet, we silently downgrade to
    /// Passive (the lowest-trust bridge-allowed tier). The ingest log captures
    /// which packet was processed, so the downgrade is auditable.
    pub fn deserialize_with_fallback<'de, D>(d: D) -> Result<BridgeOrigination, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let s = String::deserialize(d).unwrap_or_default();
        Ok(match s.as_str() {
            "sync" => BridgeOrigination::Sync,
            _ => BridgeOrigination::Passive,
        })
    }
}

impl From<BridgeOrigination> for Origination {
    fn from(b: BridgeOrigination) -> Origination {
        match b {
            BridgeOrigination::Passive => Origination::Passive,
            BridgeOrigination::Sync => Origination::Sync,
        }
    }
}

// ── BridgeObservation ─────────────────────────────────────────────────────────

/// A single observation payload within a bridge packet.
///
/// `value` is typed as `serde_json::Value` — the Rust equivalent of Python's `Any`.
/// It represents whatever JSON the local model produced: a bare string for text
/// observations, a `{"label": ..., "weight": ...}` object for domains, or an
/// Evidence-compatible dict for register observations. The ingestion layer
/// decides how to interpret it based on `field`.
///
/// Unknown top-level keys are silently ignored by serde's default behavior —
/// no `#[serde(deny_unknown_fields)]` means hallucinated extra fields are dropped
/// without error.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BridgeObservation {
    /// Dot-path identifying the target field, e.g. `"identity.core"`,
    /// `"signals.phrases"`, `"register.evidence"`.
    pub field: String,
    /// Raw JSON value produced by the local model. Parsed at ingest time.
    pub value: Value,
    #[serde(
        deserialize_with = "BridgeOrigination::deserialize_with_fallback",
        default = "BridgeOrigination::default_passive"
    )]
    pub origination: BridgeOrigination,
    /// The source text that caused this observation, for audit purposes.
    pub raw: Option<String>,
}

// ── BridgePacket ──────────────────────────────────────────────────────────────

/// An inbound packet from a local model session.
///
/// Written to disk as `{session}.bridge.json` by the local model or bridge
/// script, then picked up by the bridge watcher and fed into `ingest_bridge_packet`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BridgePacket {
    #[serde(default = "default_bridge_version")]
    pub bridge_version: String,
    /// The local model identity, e.g. `"local:gemma3:4b"`.
    pub orientation: String,
    pub session_ref: String,
    /// ISO 8601 session start timestamp.
    pub timestamp: String,
    #[serde(default)]
    pub observations: Vec<BridgeObservation>,
}

fn default_bridge_version() -> String {
    "0.1".to_string()
}
