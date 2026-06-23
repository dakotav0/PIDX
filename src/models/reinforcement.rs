//! Passive reinforcement via bridge-packet diff.
//!
//! When a new bridge packet is ingested, its observations are compared against
//! the profile's existing confirmed observations. Matched observations get a
//! weight bump (hardening) — they've been reinforced by a new session.
//!
//! # Design
//!
//! - **Exact match** on field path + value identity (same logic as `values_match`).
//! - Semantic similarity is deferred to an optional annotation layer.
//! - Reinforcement slows decay: `effective_confidence` divides λ by weight.
//! - Unreinforced observations are not penalized — absence is not contradiction.
//!   They continue decaying at their current rate.
//!
//! # Future phases
//!
//! - Session-based decay (separate from calendar decay) tracks how many sessions
//!   pass without reinforcement for each observation.
//! - Calibration file (`calibration.yml`) makes `hardening_multiplier` and
//!   `max_weight` configurable per domain.

use super::observation::{Observation, ObservationField, ObservationStatus, ObservationValue};
use super::profile::ProfileDocument;

// ── ReinforcementConfig ──────────────────────────────────────────────────────

/// Passive reinforcement parameters.
///
/// Will move into the calibration seed (Phase 2) — currently hardcoded defaults.
#[derive(Debug, Clone)]
pub struct ReinforcementConfig {
    /// Multiplier applied to weight on each reinforcement match.
    /// e.g., 1.2 = +20% weight per reinforcement session.
    pub hardening_multiplier: f64,
    /// Cap to prevent runaway weight on heavily reinforced observations.
    pub max_weight: f64,
}

impl Default for ReinforcementConfig {
    fn default() -> Self {
        Self {
            hardening_multiplier: 1.2,
            max_weight: 5.0,
        }
    }
}

// ── ReinforcementResult ──────────────────────────────────────────────────────

/// Outcome of a reinforcement pass after bridge packet ingestion.
#[derive(Debug, Clone, Default)]
pub struct ReinforcementResult {
    /// How many confirmed observations were matched and reinforced.
    pub reinforced: u32,
    /// How many new observations were created (no existing match).
    pub new_count: u32,
}

// ── Core: reinforce after ingestion ──────────────────────────────────────────

/// Match newly ingested observations against existing confirmed ones and
/// apply hardening (weight bump) to matched pairs.
///
/// Called after `ingest_bridge_packet()` finishes processing the observations
/// array — at that point all new observations are in the profile as `Proposed`
/// or `Confirmed` (if auto-confirmed). This function finds the overlap between
/// the just-ingested batch and the pre-existing confirmed observations and
/// reinforces the matches.
///
/// Reinforcement does NOT auto-confirm proposed observations; it only acts on
/// observations that were already confirmed before this packet arrived and that
/// have a matching value in the new batch.
pub fn reinforce_after_ingest(
    profile: &mut ProfileDocument,
    config: Option<&ReinforcementConfig>,
    _new_packet_session_ref: &str,
) -> ReinforcementResult {
    let default_cfg = ReinforcementConfig::default();
    let cfg = config.unwrap_or(&default_cfg);
    let mut result = ReinforcementResult::default();

    // ── Phase 1: Collect paths and values of confirmed observations ──
    // Store (path, value_as_string, obs_index) — no references into profile.
    let mut confirmed_pairs: Vec<(String, String, usize)> = Vec::new();

    fn value_key(val: &ObservationValue) -> String {
        match val {
            ObservationValue::Text(s) => s.trim().to_lowercase(),
            ObservationValue::Domain(d) => d.label.trim().to_lowercase(),
            ObservationValue::Number(n) => format!("{:.6}", n),
        }
    }

    fn collect_confirmed(
        prefix: &str,
        field: &ObservationField,
        out: &mut Vec<(String, String, usize)>,
    ) {
        for (obs_idx, obs) in field.observations.iter().enumerate() {
            if obs.status == ObservationStatus::Confirmed {
                out.push((prefix.to_string(), value_key(&obs.value), obs_idx));
            }
        }
    }

    fn collect_slice(
        prefix: &str,
        fields: &[ObservationField],
        out: &mut Vec<(String, String, usize)>,
    ) {
        for (field_idx, field) in fields.iter().enumerate() {
            for (obs_idx, obs) in field.observations.iter().enumerate() {
                if obs.status == ObservationStatus::Confirmed {
                    out.push((
                        format!("{prefix}[{field_idx}]"),
                        value_key(&obs.value),
                        obs_idx,
                    ));
                }
            }
        }
    }

    collect_slice(
        "identity.core",
        &profile.identity.core,
        &mut confirmed_pairs,
    );
    collect_confirmed(
        "identity.reasoning.style",
        &profile.identity.reasoning.style,
        &mut confirmed_pairs,
    );
    collect_confirmed(
        "identity.reasoning.pattern",
        &profile.identity.reasoning.pattern,
        &mut confirmed_pairs,
    );
    collect_confirmed(
        "identity.reasoning.intake",
        &profile.identity.reasoning.intake,
        &mut confirmed_pairs,
    );
    collect_confirmed(
        "identity.reasoning.stance",
        &profile.identity.reasoning.stance,
        &mut confirmed_pairs,
    );
    collect_slice("domains", &profile.domains, &mut confirmed_pairs);
    collect_slice("values", &profile.values, &mut confirmed_pairs);
    collect_slice(
        "signals.phrases",
        &profile.signals.phrases,
        &mut confirmed_pairs,
    );
    collect_slice(
        "signals.avoidances",
        &profile.signals.avoidances,
        &mut confirmed_pairs,
    );
    collect_slice(
        "signals.rhythms",
        &profile.signals.rhythms,
        &mut confirmed_pairs,
    );
    collect_slice(
        "signals.framings",
        &profile.signals.framings,
        &mut confirmed_pairs,
    );
    collect_confirmed("working.mode", &profile.working.mode, &mut confirmed_pairs);
    collect_confirmed("working.pace", &profile.working.pace, &mut confirmed_pairs);
    collect_confirmed(
        "working.feedback",
        &profile.working.feedback,
        &mut confirmed_pairs,
    );
    collect_confirmed(
        "working.pattern",
        &profile.working.pattern,
        &mut confirmed_pairs,
    );

    // ── Phase 2: Collect proposed values ──
    let mut proposed_keys: Vec<String> = Vec::new();

    fn collect_proposed(field: &ObservationField, out: &mut Vec<String>) {
        for obs in &field.observations {
            if obs.status == ObservationStatus::Proposed {
                out.push(value_key(&obs.value));
            }
        }
    }

    for field in &profile.identity.core {
        collect_proposed(field, &mut proposed_keys);
    }
    for field in &profile.domains {
        collect_proposed(field, &mut proposed_keys);
    }
    for field in &profile.values {
        collect_proposed(field, &mut proposed_keys);
    }
    for field in &profile.signals.phrases {
        collect_proposed(field, &mut proposed_keys);
    }
    for field in &profile.signals.avoidances {
        collect_proposed(field, &mut proposed_keys);
    }
    for field in &profile.signals.rhythms {
        collect_proposed(field, &mut proposed_keys);
    }
    for field in &profile.signals.framings {
        collect_proposed(field, &mut proposed_keys);
    }
    collect_proposed(&profile.working.mode, &mut proposed_keys);
    collect_proposed(&profile.working.pace, &mut proposed_keys);
    collect_proposed(&profile.working.feedback, &mut proposed_keys);
    collect_proposed(&profile.working.pattern, &mut proposed_keys);
    collect_proposed(&profile.identity.reasoning.style, &mut proposed_keys);
    collect_proposed(&profile.identity.reasoning.pattern, &mut proposed_keys);
    collect_proposed(&profile.identity.reasoning.intake, &mut proposed_keys);
    collect_proposed(&profile.identity.reasoning.stance, &mut proposed_keys);

    // ── Phase 3: Find matches ──
    let mut reinforced_pairs: Vec<(String, usize)> = Vec::new();

    for (c_path, c_val_key, c_obs_idx) in &confirmed_pairs {
        if proposed_keys.iter().any(|p| _keys_match(p, c_val_key)) {
            reinforced_pairs.push((c_path.clone(), *c_obs_idx));
        }
    }

    // Count new (proposed but no confirmed match)
    result.new_count = proposed_keys
        .iter()
        .filter(|p| !confirmed_pairs.iter().any(|(_, ck, _)| _keys_match(p, ck)))
        .count() as u32;

    // ── Phase 4: Apply weight bumps (mutable pass — all immutable borrows dropped) ──
    for (c_path, c_obs_idx) in &reinforced_pairs {
        if let Some(obs) = _get_obs_mut(profile, c_path, *c_obs_idx) {
            let hardening = cfg.hardening_multiplier;
            let max_w = cfg.max_weight;
            let new_weight = (obs.weight * hardening).min(max_w);
            obs.weight = new_weight;
            result.reinforced += 1;
        }
    }

    result
}

/// Case-insensitive key match for reinforcement.
fn _keys_match(a: &str, b: &str) -> bool {
    a.trim().to_lowercase() == b.trim().to_lowercase()
}

/// Get a mutable reference to an observation at the given field path and index.
fn _get_obs_mut<'a>(
    profile: &'a mut ProfileDocument,
    path: &str,
    idx: usize,
) -> Option<&'a mut Observation> {
    // Parse prefix and index: "identity.core[0]" → ("identity.core", Some(0))
    let (prefix, list_idx) = if let Some(stripped) = path.strip_suffix(']') {
        let parts: Vec<&str> = stripped.rsplitn(2, '[').collect();
        if parts.len() == 2 {
            (parts[1], parts[0].parse::<usize>().ok())
        } else {
            (path, None)
        }
    } else {
        (path, None)
    };

    match prefix {
        "identity.core" => {
            let list_idx = list_idx?;
            profile
                .identity
                .core
                .get_mut(list_idx)?
                .observations
                .get_mut(idx)
        }
        "identity.reasoning.style" => profile.identity.reasoning.style.observations.get_mut(idx),
        "identity.reasoning.pattern" => {
            profile.identity.reasoning.pattern.observations.get_mut(idx)
        }
        "identity.reasoning.intake" => profile.identity.reasoning.intake.observations.get_mut(idx),
        "identity.reasoning.stance" => profile.identity.reasoning.stance.observations.get_mut(idx),
        "domains" => {
            let list_idx = list_idx?;
            profile.domains.get_mut(list_idx)?.observations.get_mut(idx)
        }
        "values" => {
            let list_idx = list_idx?;
            profile.values.get_mut(list_idx)?.observations.get_mut(idx)
        }
        "signals.phrases" => {
            let list_idx = list_idx?;
            profile
                .signals
                .phrases
                .get_mut(list_idx)?
                .observations
                .get_mut(idx)
        }
        "signals.avoidances" => {
            let list_idx = list_idx?;
            profile
                .signals
                .avoidances
                .get_mut(list_idx)?
                .observations
                .get_mut(idx)
        }
        "signals.rhythms" => {
            let list_idx = list_idx?;
            profile
                .signals
                .rhythms
                .get_mut(list_idx)?
                .observations
                .get_mut(idx)
        }
        "signals.framings" => {
            let list_idx = list_idx?;
            profile
                .signals
                .framings
                .get_mut(list_idx)?
                .observations
                .get_mut(idx)
        }
        "working.mode" => profile.working.mode.observations.get_mut(idx),
        "working.pace" => profile.working.pace.observations.get_mut(idx),
        "working.feedback" => profile.working.feedback.observations.get_mut(idx),
        "working.pattern" => profile.working.pattern.observations.get_mut(idx),
        _ => None,
    }
}

// ── Value identity comparison ────────────────────────────────────────────────

/// True when two observation values are identical (exact match).
///
/// Used for reinforcement matching. Semantic similarity is deferred to the
/// optional annotation layer (Phase 2+).
fn values_identical(a: &ObservationValue, b: &ObservationValue) -> bool {
    match (a, b) {
        (ObservationValue::Text(a), ObservationValue::Text(b)) => {
            a.trim().to_lowercase() == b.trim().to_lowercase()
        }
        (ObservationValue::Domain(a), ObservationValue::Domain(b)) => {
            a.label.trim().to_lowercase() == b.label.trim().to_lowercase()
        }
        (ObservationValue::Number(a), ObservationValue::Number(b)) => {
            (*a - *b).abs() <= f64::EPSILON
        }
        _ => false,
    }
}

// ── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::confidence::Origination;
    use crate::models::observation::{
        Observation, ObservationField, ObservationSource, ObservationStatus, ObservationValue,
    };
    use crate::models::profile::ProfileDocument;
    use serde_json::json;

    fn make_confirmed_obs(value: &str, weight: f64) -> Observation {
        Observation {
            value: ObservationValue::Text(value.to_string()),
            source: ObservationSource {
                origination: Origination::Passive,
                orientation: "test".into(),
                session_ref: "old-session".into(),
                timestamp: "2026-01-01T00:00:00Z".into(),
            },
            confidence: 0.8,
            weight,
            status: ObservationStatus::Confirmed,
            revision: 1,
            decay_exempt: false,
        }
    }

    fn make_proposed_obs(value: &str) -> Observation {
        Observation {
            value: ObservationValue::Text(value.to_string()),
            source: ObservationSource {
                origination: Origination::Passive,
                orientation: "test".into(),
                session_ref: "new-session".into(),
                timestamp: "2026-05-26T00:00:00Z".into(),
            },
            confidence: 0.8,
            weight: 1.0,
            status: ObservationStatus::Proposed,
            revision: 0,
            decay_exempt: false,
        }
    }

    #[test]
    fn values_identical_matches_strings_case_insensitive() {
        let a = ObservationValue::Text("systems-first".into());
        let b = ObservationValue::Text("Systems-First".into());
        assert!(values_identical(&a, &b));
    }

    #[test]
    fn reinforcement_bumps_weight() {
        let mut profile = ProfileDocument::new("test");

        // Pre-populate a confirmed observation
        profile
            .working
            .mode
            .observations
            .push(make_confirmed_obs("sketch-first", 1.0));

        // Add a matching proposed observation (simulating post-ingestion state)
        profile
            .working
            .mode
            .observations
            .push(make_proposed_obs("sketch-first"));

        let cfg = ReinforcementConfig::default();
        let result = reinforce_after_ingest(&mut profile, Some(&cfg), "new-session");

        assert_eq!(result.reinforced, 1);
        let confirmed = &profile.working.mode.observations[0];
        assert!((confirmed.weight - 1.2).abs() < 1e-9);
        assert_eq!(confirmed.status, ObservationStatus::Confirmed);
    }

    #[test]
    fn new_observation_not_reinforced() {
        let mut profile = ProfileDocument::new("test");

        profile
            .working
            .mode
            .observations
            .push(make_confirmed_obs("sketch-first", 1.0));
        profile
            .working
            .mode
            .observations
            .push(make_proposed_obs("spec-first")); // different value

        let cfg = ReinforcementConfig::default();
        let result = reinforce_after_ingest(&mut profile, Some(&cfg), "new-session");

        assert_eq!(result.reinforced, 0);
        assert_eq!(result.new_count, 1);
        // Weight unchanged
        assert!((profile.working.mode.observations[0].weight - 1.0).abs() < 1e-9);
    }

    #[test]
    fn weight_capped_at_max() {
        let mut profile = ProfileDocument::new("test");

        profile
            .working
            .mode
            .observations
            .push(make_confirmed_obs("sketch-first", 4.5));
        profile
            .working
            .mode
            .observations
            .push(make_proposed_obs("sketch-first"));

        let cfg = ReinforcementConfig::default(); // max_weight = 5.0
        reinforce_after_ingest(&mut profile, Some(&cfg), "new-session");

        let confirmed = &profile.working.mode.observations[0];
        // 4.5 * 1.2 = 5.4, capped at 5.0
        assert!((confirmed.weight - 5.0).abs() < 1e-9);
    }
}
