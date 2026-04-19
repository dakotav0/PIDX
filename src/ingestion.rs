use std::collections::HashMap;

use uuid::Uuid;

use crate::models::bridge::{BridgeObservation, BridgeOrigination, BridgePacket};
use crate::models::confidence::{Origination, CORROBORATION_BONUS};
use crate::models::decay::FieldClass;
use crate::models::evidence::{Evidence, RegisterMetricName};
use crate::models::observation::{
    Observation, ObservationField, ObservationSource, ObservationStatus, ObservationValue,
};
use crate::models::profile::{BridgeLogEntry, DeltaItem, ProfileDocument, ProfileMeta, ReviewItem};
use crate::traits::IngestSource;

// ── IngestSource for bridge observations ─────────────────────────────────────

/// Pairs a single bridge observation's origination with the packet's shared
/// orientation and session_ref to produce an `IngestSource` implementation.
///
/// ## Rust lesson: lifetime parameters
///
/// The `'a` here is a lifetime — it tells the compiler that `BridgeObsSource`
/// cannot outlive the `BridgePacket` it borrows. Lifetimes are Rust's way of
/// making "this reference is valid as long as that thing exists" explicit. You
/// don't need to do arithmetic with them; the compiler just needs to know the
/// relationship so it can verify safety. This particular pattern — a short-lived
/// "view" struct that holds references into other data — is common in Rust.
struct BridgeObsSource<'a> {
    origination: BridgeOrigination,
    packet: &'a BridgePacket,
}

impl<'a> IngestSource for BridgeObsSource<'a> {
    fn origination(&self) -> Origination {
        Origination::from(self.origination)
    }
    fn orientation(&self) -> &str {
        &self.packet.orientation
    }
    fn session_ref(&self) -> &str {
        &self.packet.session_ref
    }
    // base_confidence() is provided by the trait using the above three methods —
    // no override needed. Local passive → 0.61, local sync → 0.55.
}

// ── Field routing ─────────────────────────────────────────────────────────────

/// Result of resolving a dot-path to a field in the profile.
enum FieldRoute<'a> {
    /// New slot created (or singleton field returned). Normal proposed observation.
    Field(&'a mut ObservationField, FieldClass),
    /// Existing proposed slot with the same value — increment `proposal_count` instead
    /// of pushing a new observation.
    DedupField(&'a mut ObservationField),
    /// `"register.evidence"` — additive path, handled separately.
    RegisterEvidence,
    /// Unrecognized path — skip silently, matching Python behaviour.
    Unknown,
}

/// Resolve a dot-path string to a mutable reference into the profile.
///
/// For list fields (`identity.core`, `domains`, `values`, `signals.*`), a new
/// empty `ObservationField` is appended and a reference to it is returned — each
/// bridge observation for these paths represents a distinct value in the list.
///
/// For singleton fields (`identity.reasoning.*`, `working.*`), the existing field
/// is returned by mutable reference so the observation is appended in-place.
///
/// ## Rust lesson: exhaustive match on strings
///
/// Python's `_get_or_create_field` uses `hasattr`/`getattr` reflection. Rust has
/// no runtime reflection, so we use a `match` on `&str`. This is actually safer:
/// add a new profile field path later and the compiler doesn't care — but you
/// won't accidentally silently drop packets for a path you forgot to add here.
/// The `_` arm makes "unknown path → skip" the explicit policy.
fn route_field<'a>(
    profile: &'a mut ProfileDocument,
    path: &str,
    incoming: &serde_json::Value,
) -> FieldRoute<'a> {
    match path {
        "register.evidence" => FieldRoute::RegisterEvidence,

        "identity.core" => {
            let match_res = find_matching_field(&profile.identity.core, incoming);
            let (idx, has_proposed) = match_res.unwrap_or_else(|| {
                profile.identity.core.push(ObservationField::default());
                (profile.identity.core.len() - 1, false)
            });
            if has_proposed {
                FieldRoute::DedupField(&mut profile.identity.core[idx])
            } else {
                FieldRoute::Field(&mut profile.identity.core[idx], FieldClass::Identity)
            }
        }

        "identity.reasoning.style" => {
            FieldRoute::Field(&mut profile.identity.reasoning.style, FieldClass::Identity)
        }
        "identity.reasoning.pattern" => FieldRoute::Field(
            &mut profile.identity.reasoning.pattern,
            FieldClass::Identity,
        ),
        "identity.reasoning.intake" => {
            FieldRoute::Field(&mut profile.identity.reasoning.intake, FieldClass::Identity)
        }
        "identity.reasoning.stance" => {
            FieldRoute::Field(&mut profile.identity.reasoning.stance, FieldClass::Identity)
        }

        "domains" => {
            let match_res = find_matching_field(&profile.domains, incoming);
            let (idx, has_proposed) = match_res.unwrap_or_else(|| {
                profile.domains.push(ObservationField::default());
                (profile.domains.len() - 1, false)
            });
            if has_proposed {
                FieldRoute::DedupField(&mut profile.domains[idx])
            } else {
                FieldRoute::Field(&mut profile.domains[idx], FieldClass::Domain)
            }
        }

        "values" => {
            let match_res = find_matching_field(&profile.values, incoming);
            let (idx, has_proposed) = match_res.unwrap_or_else(|| {
                profile.values.push(ObservationField::default());
                (profile.values.len() - 1, false)
            });
            if has_proposed {
                FieldRoute::DedupField(&mut profile.values[idx])
            } else {
                FieldRoute::Field(&mut profile.values[idx], FieldClass::Value)
            }
        }

        "signals.phrases" => {
            let match_res = find_matching_field(&profile.signals.phrases, incoming);
            let (idx, has_proposed) = match_res.unwrap_or_else(|| {
                profile.signals.phrases.push(ObservationField::default());
                (profile.signals.phrases.len() - 1, false)
            });
            if has_proposed {
                FieldRoute::DedupField(&mut profile.signals.phrases[idx])
            } else {
                FieldRoute::Field(&mut profile.signals.phrases[idx], FieldClass::Signal)
            }
        }
        "signals.avoidances" => {
            let match_res = find_matching_field(&profile.signals.avoidances, incoming);
            let (idx, has_proposed) = match_res.unwrap_or_else(|| {
                profile.signals.avoidances.push(ObservationField::default());
                (profile.signals.avoidances.len() - 1, false)
            });
            if has_proposed {
                FieldRoute::DedupField(&mut profile.signals.avoidances[idx])
            } else {
                FieldRoute::Field(&mut profile.signals.avoidances[idx], FieldClass::Signal)
            }
        }
        "signals.rhythms" => {
            let match_res = find_matching_field(&profile.signals.rhythms, incoming);
            let (idx, has_proposed) = match_res.unwrap_or_else(|| {
                profile.signals.rhythms.push(ObservationField::default());
                (profile.signals.rhythms.len() - 1, false)
            });
            if has_proposed {
                FieldRoute::DedupField(&mut profile.signals.rhythms[idx])
            } else {
                FieldRoute::Field(&mut profile.signals.rhythms[idx], FieldClass::Signal)
            }
        }
        "signals.framings" => {
            let match_res = find_matching_field(&profile.signals.framings, incoming);
            let (idx, has_proposed) = match_res.unwrap_or_else(|| {
                profile.signals.framings.push(ObservationField::default());
                (profile.signals.framings.len() - 1, false)
            });
            if has_proposed {
                FieldRoute::DedupField(&mut profile.signals.framings[idx])
            } else {
                FieldRoute::Field(&mut profile.signals.framings[idx], FieldClass::Signal)
            }
        }

        "working.mode" => FieldRoute::Field(&mut profile.working.mode, FieldClass::Working),
        "working.pace" => FieldRoute::Field(&mut profile.working.pace, FieldClass::Working),
        "working.feedback" => FieldRoute::Field(&mut profile.working.feedback, FieldClass::Working),
        "working.pattern" => FieldRoute::Field(&mut profile.working.pattern, FieldClass::Working),

        _ => FieldRoute::Unknown,
    }
}

// ── Value parsing & conflict detection ───────────────────────────────────────

/// Convert a raw `serde_json::Value` from a bridge packet into an `ObservationValue`.
///
/// Returns `None` for JSON types we don't recognize (arrays, booleans, null).
/// The calling code skips those entries rather than panicking.
fn parse_value(v: &serde_json::Value) -> Option<ObservationValue> {
    match v {
        serde_json::Value::String(s) => Some(ObservationValue::Text(s.clone())),
        serde_json::Value::Number(n) => n.as_f64().map(ObservationValue::Number),
        serde_json::Value::Object(_) => {
            // Try to deserialize as DomainEntry. If the object lacks a "label"
            // key this will fail and we return None, skipping the observation.
            serde_json::from_value(v.clone())
                .ok()
                .map(ObservationValue::Domain)
        }
        _ => None,
    }
}

/// Decide whether an incoming JSON value conflicts with an existing observation value.
///
/// Strings: conflict if they differ (case-insensitive, trimmed).
/// Domain objects: conflict if their labels differ.
/// Mismatched types: no conflict — the observation is simply proposed alongside.
fn values_conflict(existing: &ObservationValue, incoming: &serde_json::Value) -> bool {
    match (existing, incoming) {
        (ObservationValue::Text(a), serde_json::Value::String(b)) => {
            a.trim().to_lowercase() != b.trim().to_lowercase()
        }
        (ObservationValue::Domain(d), serde_json::Value::Object(_)) => {
            // Conflict if the incoming object parses as a DomainEntry with a different label.
            if let Some(serde_json::Value::String(incoming_label)) = incoming.get("label") {
                d.label.trim().to_lowercase() != incoming_label.trim().to_lowercase()
            } else {
                false
            }
        }
        (ObservationValue::Number(a), serde_json::Value::Number(b)) => {
            b.as_f64().map_or(false, |b| (*a - b).abs() > f64::EPSILON)
        }
        _ => false,
    }
}

/// Mirror of `values_conflict` — true when existing and incoming are the same value.
/// Used to identify duplicate proposed slots during deduplication.
fn values_match(existing: &ObservationValue, incoming: &serde_json::Value) -> bool {
    match (existing, incoming) {
        (ObservationValue::Text(a), serde_json::Value::String(b)) => {
            a.trim().to_lowercase() == b.trim().to_lowercase()
        }
        (ObservationValue::Domain(d), serde_json::Value::Object(_)) => {
            if let Some(serde_json::Value::String(label)) = incoming.get("label") {
                d.label.trim().to_lowercase() == label.trim().to_lowercase()
            } else {
                false
            }
        }
        (ObservationValue::Number(a), serde_json::Value::Number(b)) => {
            b.as_f64().map_or(false, |b| (*a - b).abs() <= f64::EPSILON)
        }
        _ => false,
    }
}

/// Scan a list-field slice for an existing slot whose value matches `incoming`.
/// Returns `Some((index, has_proposed_match))` if found, `None` otherwise.
fn find_matching_field(
    fields: &[ObservationField],
    incoming: &serde_json::Value,
) -> Option<(usize, bool)> {
    for (i, field) in fields.iter().enumerate() {
        if field
            .observations
            .iter()
            .any(|o| values_match(&o.value, incoming))
        {
            let has_proposed = field.observations.iter().any(|o| {
                o.status == ObservationStatus::Proposed && values_match(&o.value, incoming)
            });
            return Some((i, has_proposed));
        }
    }
    None
}

// ── Evidence ingestion ────────────────────────────────────────────────────────

/// Append an Evidence item to the correct RegisterMetric in the profile.
///
/// Evidence is always additive — no delta detection runs on it. The register
/// score is recomputed at read-time from the full evidence pool.
///
/// Uses an exhaustive `match` on `RegisterMetricName` rather than Python's
/// `hasattr`/`getattr`. If we add a new register dimension later, the compiler
/// will flag this match as non-exhaustive and force us to handle it.
fn ingest_evidence(profile: &mut ProfileDocument, bo: &BridgeObservation, _packet: &BridgePacket) {
    // "let-else" — if the pattern doesn't match, execute the else block.
    // The else block must diverge (return, break, continue, or panic).
    // This is the idiomatic Rust alternative to Python's early-return guards.
    let serde_json::Value::Object(_) = &bo.value else {
        return;
    };
    let Ok(ev) = serde_json::from_value::<Evidence>(bo.value.clone()) else {
        return;
    };

    match ev.metric {
        RegisterMetricName::Formality => profile.comm.formality.evidence.push(ev),
        RegisterMetricName::Directness => profile.comm.directness.evidence.push(ev),
        RegisterMetricName::Hedging => profile.comm.hedging.evidence.push(ev),
        RegisterMetricName::Humor => profile.comm.humor.evidence.push(ev),
        RegisterMetricName::Abstraction => profile.comm.abstraction.evidence.push(ev),
        RegisterMetricName::Affect => profile.comm.affect.evidence.push(ev),
    }
}

// ── Main ingestion function ───────────────────────────────────────────────────

/// Ingest a `BridgePacket` into the profile.
///
/// For each observation in the packet:
/// - `register.evidence` → additive append to the correct `RegisterMetric`
/// - all other paths → route to the correct `ObservationField`, check for conflicts
///
/// Observations start as `"proposed"` — they are never auto-confirmed from bridge
/// packets. The user must explicitly confirm them in the session review.
///
/// Appends a `BridgeLogEntry` to `profile.bridge_log.processed` on completion.
///
/// Returns `(observations_proposed, deltas_flagged)`.
pub fn ingest_bridge_packet(
    profile: &mut ProfileDocument,
    packet: &BridgePacket,
    filename: &str,
) -> (usize, usize) {
    let mut proposed = 0usize;
    let mut deltas = 0usize;

    for bo in &packet.observations {
        // Register evidence: additive path, no routing or delta detection.
        if bo.field == "register.evidence" {
            ingest_evidence(profile, bo, packet);
            continue;
        }

        // Parse the raw JSON value into a typed ObservationValue.
        // If the value isn't a recognized type, skip this observation silently.
        let Some(obs_value) = parse_value(&bo.value) else {
            continue;
        };

        // Build the IngestSource view for this observation and get its confidence.
        let source_view = BridgeObsSource {
            origination: bo.origination,
            packet,
        };
        let base_conf = source_view.base_confidence();

        // Construct the full ObservationSource for storage.
        let source = ObservationSource {
            origination: Origination::from(bo.origination),
            orientation: packet.orientation.clone(),
            session_ref: packet.session_ref.clone(),
            timestamp: packet.timestamp.clone(),
        };

        let mut new_obs = Observation {
            value: obs_value,
            source,
            confidence: base_conf,
            weight: 1.0,
            status: ObservationStatus::Proposed,
            revision: 1,
            decay_exempt: false,
        };

        // Route the dot-path to the target field. Unknown paths are skipped.
        //
        // Rust lesson: borrow splitting
        // `route_field` returns a mutable reference *into* profile (the field).
        // While that reference is live, the borrow checker treats `profile` as
        // mutably borrowed — so we can't also push to `profile.delta_queue` inside
        // the same match arm. Fix: build the DeltaItem while the field reference is
        // live (cloning the data we need), then push to delta_queue *after* the match
        // arm ends and the field borrow is released.
        let mut pending_delta: Option<DeltaItem> = None;

        match route_field(profile, &bo.field, &bo.value) {
            FieldRoute::Unknown | FieldRoute::RegisterEvidence => continue,
            FieldRoute::DedupField(field) => {
                // Same value already proposed — increment counter, no new slot.
                field.proposal_count = field.proposal_count.saturating_add(1);
                proposed += 1;
            }
            FieldRoute::Field(field, _field_class) => {
                // Delta detection: find the first confirmed observation that conflicts.
                let conflict_idx = field.observations.iter().position(|o| {
                    o.status == ObservationStatus::Confirmed && values_conflict(&o.value, &bo.value)
                });

                if let Some(idx) = conflict_idx {
                    // Park the existing confirmed observation in delta status.
                    field.observations[idx].status = ObservationStatus::Delta;
                    new_obs.status = ObservationStatus::Delta;

                    // Clone data needed for DeltaItem — we'll push to delta_queue
                    // after this arm so the field borrow is fully released first.
                    pending_delta = Some(DeltaItem {
                        id: Uuid::new_v4().to_string(),
                        field: bo.field.clone(),
                        a: field.observations[idx].clone(),
                        b: new_obs.clone(),
                        created_at: crate::models::profile::ProfileMeta::now_utc(),
                        resolved: false,
                    });
                    field.observations.push(new_obs);
                    deltas += 1;
                } else {
                    field.observations.push(new_obs);
                    proposed += 1;
                }
            }
        }

        // Field borrow is gone here. Safe to mutably borrow delta_queue now.
        if let Some(delta) = pending_delta {
            profile.delta_queue.push(delta);
        }
    }

    // Audit log: record this packet regardless of outcome.
    profile.bridge_log.processed.push(BridgeLogEntry {
        filename: filename.to_string(),
        ingested_at: crate::models::profile::ProfileMeta::now_utc(),
        observations_proposed: proposed as u32,
        deltas_flagged: deltas as u32,
    });
    profile
        .bridge_log
        .pending_filenames
        .retain(|f| f != filename);

    (proposed, deltas)
}

// ── Corroboration ─────────────────────────────────────────────────────────────

/// Apply the corroboration confidence bonus across all confirmed observations.
///
/// If ≥2 independent orientations have confirmed the same value in the same field,
/// each confirming observation gets +0.08 confidence (capped at 1.0) and its
/// weight is set to 1.0. This rewards multi-source agreement.
///
/// Returns the number of observations that received the bonus.
pub fn run_corroboration(profile: &mut ProfileDocument) -> usize {
    let mut boosted = 0;

    // corroborate_field takes a single &mut ObservationField — safe to call
    // repeatedly because each call borrows a different part of the profile.
    corroborate_field(&mut profile.identity.reasoning.style, &mut boosted);
    corroborate_field(&mut profile.identity.reasoning.pattern, &mut boosted);
    corroborate_field(&mut profile.identity.reasoning.intake, &mut boosted);
    corroborate_field(&mut profile.identity.reasoning.stance, &mut boosted);

    for f in &mut profile.identity.core {
        corroborate_field(f, &mut boosted);
    }
    for f in &mut profile.domains {
        corroborate_field(f, &mut boosted);
    }
    for f in &mut profile.values {
        corroborate_field(f, &mut boosted);
    }
    for f in &mut profile.signals.phrases {
        corroborate_field(f, &mut boosted);
    }
    for f in &mut profile.signals.avoidances {
        corroborate_field(f, &mut boosted);
    }
    for f in &mut profile.signals.rhythms {
        corroborate_field(f, &mut boosted);
    }
    for f in &mut profile.signals.framings {
        corroborate_field(f, &mut boosted);
    }
    corroborate_field(&mut profile.working.mode, &mut boosted);
    corroborate_field(&mut profile.working.pace, &mut boosted);
    corroborate_field(&mut profile.working.feedback, &mut boosted);
    corroborate_field(&mut profile.working.pattern, &mut boosted);

    boosted
}

/// Apply the corroboration bonus to a single `ObservationField`.
///
/// Groups confirmed observations by a canonical value key. If a group has
/// observations from ≥2 different orientations, all observations in the group
/// receive the bonus.
fn corroborate_field(field: &mut ObservationField, boosted: &mut usize) {
    // Collect indices of confirmed observations.
    let confirmed_indices: Vec<usize> = field
        .observations
        .iter()
        .enumerate()
        .filter(|(_, o)| o.status == ObservationStatus::Confirmed)
        .map(|(i, _)| i)
        .collect();

    if confirmed_indices.len() < 2 {
        return;
    }

    // Group by canonical value key (label for Domain, lowercased text for Text).
    let mut groups: HashMap<String, Vec<usize>> = HashMap::new();
    for &i in &confirmed_indices {
        let key = value_key(&field.observations[i].value);
        groups.entry(key).or_default().push(i);
    }

    // For each group with ≥2 independent orientations, apply the bonus.
    for group_indices in groups.values() {
        if group_indices.len() < 2 {
            continue;
        }
        let orientation_count = group_indices
            .iter()
            .map(|&i| field.observations[i].source.orientation.as_str())
            .collect::<std::collections::HashSet<_>>()
            .len();

        if orientation_count >= 2 {
            for &i in group_indices {
                let obs = &mut field.observations[i];
                if obs.confidence < 1.0 {
                    obs.confidence = (obs.confidence + CORROBORATION_BONUS).min(1.0);
                    obs.weight = 1.0;
                    *boosted += 1;
                }
            }
        }
    }
}

/// Canonical string key used for grouping observations with the same value.
fn value_key(v: &ObservationValue) -> String {
    match v {
        ObservationValue::Text(s) => s.trim().to_lowercase(),
        ObservationValue::Domain(d) => d.label.trim().to_lowercase(),
        ObservationValue::Number(n) => n.to_string(),
    }
}

// ── Bulk confirm ─────────────────────────────────────────────────────────────

/// Flip every `Proposed` observation to `Confirmed` in all fields whose path
/// starts with `prefix`. Returns the paths that were actually modified.
///
/// Used by both the CLI's `confirm-all` command and the Tauri `confirm_all` command.
pub fn confirm_all_proposed(profile: &mut ProfileDocument, prefix: &str) -> Vec<String> {
    // Immutable pass: collect paths of fields that have at least one Proposed obs.
    let matching: Vec<String> = {
        let mut v = Vec::new();
        let mut push_if = |path: &str, field: &ObservationField| {
            if path.starts_with(prefix)
                && field
                    .observations
                    .iter()
                    .any(|o| o.status == ObservationStatus::Proposed)
            {
                v.push(path.to_string());
            }
        };
        for (i, f) in profile.identity.core.iter().enumerate() {
            push_if(&format!("identity.core.{i}"), f);
        }
        push_if(
            "identity.reasoning.style",
            &profile.identity.reasoning.style,
        );
        push_if(
            "identity.reasoning.pattern",
            &profile.identity.reasoning.pattern,
        );
        push_if(
            "identity.reasoning.intake",
            &profile.identity.reasoning.intake,
        );
        push_if(
            "identity.reasoning.stance",
            &profile.identity.reasoning.stance,
        );
        for (i, f) in profile.domains.iter().enumerate() {
            push_if(&format!("domains.{i}"), f);
        }
        for (i, f) in profile.values.iter().enumerate() {
            push_if(&format!("values.{i}"), f);
        }
        for (i, f) in profile.signals.phrases.iter().enumerate() {
            push_if(&format!("signals.phrases.{i}"), f);
        }
        for (i, f) in profile.signals.avoidances.iter().enumerate() {
            push_if(&format!("signals.avoidances.{i}"), f);
        }
        for (i, f) in profile.signals.rhythms.iter().enumerate() {
            push_if(&format!("signals.rhythms.{i}"), f);
        }
        for (i, f) in profile.signals.framings.iter().enumerate() {
            push_if(&format!("signals.framings.{i}"), f);
        }
        push_if("working.mode", &profile.working.mode);
        push_if("working.pace", &profile.working.pace);
        push_if("working.feedback", &profile.working.feedback);
        push_if("working.pattern", &profile.working.pattern);
        v
    };

    let mut confirmed_paths = Vec::new();
    for path in &matching {
        // Resolve path to a mutable field reference using the same rules as route_field.
        let field_opt = resolve_field_for_confirm(profile, path);
        if let Some(field) = field_opt {
            let mut any = false;
            for obs in field.observations.iter_mut() {
                if obs.status == ObservationStatus::Proposed {
                    obs.status = ObservationStatus::Confirmed;
                    any = true;
                }
            }
            if any {
                confirmed_paths.push(path.clone());
            }
        }
    }
    confirmed_paths
}

/// Flip every `Proposed` observation to `Rejected` in all fields whose path
/// starts with `prefix`. Returns the paths that were actually modified.
pub fn reject_all_proposed(profile: &mut ProfileDocument, prefix: &str) -> Vec<String> {
    let matching: Vec<String> = {
        let mut v = Vec::new();
        let mut push_if = |path: &str, field: &ObservationField| {
            if path.starts_with(prefix)
                && field
                    .observations
                    .iter()
                    .any(|o| o.status == ObservationStatus::Proposed)
            {
                v.push(path.to_string());
            }
        };
        for (i, f) in profile.identity.core.iter().enumerate() {
            push_if(&format!("identity.core.{i}"), f);
        }
        push_if(
            "identity.reasoning.style",
            &profile.identity.reasoning.style,
        );
        push_if(
            "identity.reasoning.pattern",
            &profile.identity.reasoning.pattern,
        );
        push_if(
            "identity.reasoning.intake",
            &profile.identity.reasoning.intake,
        );
        push_if(
            "identity.reasoning.stance",
            &profile.identity.reasoning.stance,
        );
        for (i, f) in profile.domains.iter().enumerate() {
            push_if(&format!("domains.{i}"), f);
        }
        for (i, f) in profile.values.iter().enumerate() {
            push_if(&format!("values.{i}"), f);
        }
        for (i, f) in profile.signals.phrases.iter().enumerate() {
            push_if(&format!("signals.phrases.{i}"), f);
        }
        for (i, f) in profile.signals.avoidances.iter().enumerate() {
            push_if(&format!("signals.avoidances.{i}"), f);
        }
        for (i, f) in profile.signals.rhythms.iter().enumerate() {
            push_if(&format!("signals.rhythms.{i}"), f);
        }
        for (i, f) in profile.signals.framings.iter().enumerate() {
            push_if(&format!("signals.framings.{i}"), f);
        }
        push_if("working.mode", &profile.working.mode);
        push_if("working.pace", &profile.working.pace);
        push_if("working.feedback", &profile.working.feedback);
        push_if("working.pattern", &profile.working.pattern);
        v
    };

    let mut rejected_paths = Vec::new();
    for path in &matching {
        let field_opt = resolve_field_for_confirm(profile, path);
        if let Some(field) = field_opt {
            let mut any = false;
            for obs in field.observations.iter_mut() {
                if obs.status == ObservationStatus::Proposed {
                    obs.status = ObservationStatus::Rejected;
                    any = true;
                }
            }
            if any {
                rejected_paths.push(path.clone());
            }
        }
    }
    rejected_paths
}

/// Minimal path-resolver for the confirm-all operation. Mirrors the CLI's
/// `resolve_field_mut` but lives in the library so Tauri can share it.
fn resolve_field_for_confirm<'a>(
    profile: &'a mut ProfileDocument,
    path: &str,
) -> Option<&'a mut ObservationField> {
    let parts: Vec<&str> = path.splitn(3, '.').collect();
    match parts.as_slice() {
        ["identity", "core", rest] => profile.identity.core.get_mut(rest.parse::<usize>().ok()?),
        ["identity", "reasoning", name] => match *name {
            "style" => Some(&mut profile.identity.reasoning.style),
            "pattern" => Some(&mut profile.identity.reasoning.pattern),
            "intake" => Some(&mut profile.identity.reasoning.intake),
            "stance" => Some(&mut profile.identity.reasoning.stance),
            _ => None,
        },
        ["domains", idx] => profile.domains.get_mut(idx.parse::<usize>().ok()?),
        ["values", idx] => profile.values.get_mut(idx.parse::<usize>().ok()?),
        ["signals", cat, idx] => {
            let i: usize = idx.parse().ok()?;
            match *cat {
                "phrases" => profile.signals.phrases.get_mut(i),
                "avoidances" => profile.signals.avoidances.get_mut(i),
                "rhythms" => profile.signals.rhythms.get_mut(i),
                "framings" => profile.signals.framings.get_mut(i),
                _ => None,
            }
        }
        ["working", name] => match *name {
            "mode" => Some(&mut profile.working.mode),
            "pace" => Some(&mut profile.working.pace),
            "feedback" => Some(&mut profile.working.feedback),
            "pattern" => Some(&mut profile.working.pattern),
            _ => None,
        },
        _ => None,
    }
}

// ── Decay pass ────────────────────────────────────────────────────────────────

/// Walk all confirmed observations and flag those whose effective confidence has
/// decayed below `threshold` into the profile's `review_queue`.
///
/// Already-queued unresolved items are not duplicated. Returns the number of
/// newly created `ReviewItem`s.
///
/// Typical threshold: `0.30`. Identity fields decay slowly (λ=0.0005), so they
/// take years to reach 0.30. Signal fields (λ=0.0200) can reach it in weeks.
pub fn run_decay_pass(profile: &mut ProfileDocument, threshold: f64) -> usize {
    use chrono::Utc;
    let now = Utc::now();

    // ── Immutable scan ────────────────────────────────────────────────────────
    // Collect (path, obs_index, effective_conf) for all confirmed observations
    // that have decayed below threshold. All borrows on profile are released when
    // this block exits.
    let candidates: Vec<(String, usize, f64)> = {
        let mut v: Vec<(String, usize, f64)> = Vec::new();

        let scan = |v: &mut Vec<_>, path: &str, field: &ObservationField, fc: FieldClass| {
            for (idx, obs) in field.observations.iter().enumerate() {
                if obs.status != ObservationStatus::Confirmed || obs.decay_exempt {
                    continue;
                }
                let c = obs.effective_confidence(fc, Some(now));
                if c < threshold {
                    v.push((path.to_string(), idx, c));
                }
            }
        };

        // Identity
        for (i, f) in profile.identity.core.iter().enumerate() {
            scan(
                &mut v,
                &format!("identity.core.{i}"),
                f,
                FieldClass::Identity,
            );
        }
        scan(
            &mut v,
            "identity.reasoning.style",
            &profile.identity.reasoning.style,
            FieldClass::Identity,
        );
        scan(
            &mut v,
            "identity.reasoning.pattern",
            &profile.identity.reasoning.pattern,
            FieldClass::Identity,
        );
        scan(
            &mut v,
            "identity.reasoning.intake",
            &profile.identity.reasoning.intake,
            FieldClass::Identity,
        );
        scan(
            &mut v,
            "identity.reasoning.stance",
            &profile.identity.reasoning.stance,
            FieldClass::Identity,
        );
        // Domains / Values
        for (i, f) in profile.domains.iter().enumerate() {
            scan(&mut v, &format!("domains.{i}"), f, FieldClass::Domain);
        }
        for (i, f) in profile.values.iter().enumerate() {
            scan(&mut v, &format!("values.{i}"), f, FieldClass::Value);
        }
        // Signals
        for (i, f) in profile.signals.phrases.iter().enumerate() {
            scan(
                &mut v,
                &format!("signals.phrases.{i}"),
                f,
                FieldClass::Signal,
            );
        }
        for (i, f) in profile.signals.avoidances.iter().enumerate() {
            scan(
                &mut v,
                &format!("signals.avoidances.{i}"),
                f,
                FieldClass::Signal,
            );
        }
        for (i, f) in profile.signals.rhythms.iter().enumerate() {
            scan(
                &mut v,
                &format!("signals.rhythms.{i}"),
                f,
                FieldClass::Signal,
            );
        }
        for (i, f) in profile.signals.framings.iter().enumerate() {
            scan(
                &mut v,
                &format!("signals.framings.{i}"),
                f,
                FieldClass::Signal,
            );
        }
        // Working
        scan(
            &mut v,
            "working.mode",
            &profile.working.mode,
            FieldClass::Working,
        );
        scan(
            &mut v,
            "working.pace",
            &profile.working.pace,
            FieldClass::Working,
        );
        scan(
            &mut v,
            "working.feedback",
            &profile.working.feedback,
            FieldClass::Working,
        );
        scan(
            &mut v,
            "working.pattern",
            &profile.working.pattern,
            FieldClass::Working,
        );

        v
    };
    // All &ObservationField borrows released here. ─────────────────────────────

    // Remove candidates already in the review queue as unresolved.
    let to_flag: Vec<_> = candidates
        .into_iter()
        .filter(|(path, idx, _)| {
            !profile
                .review_queue
                .iter()
                .any(|r| !r.resolved && r.field == *path && r.observation_index == *idx)
        })
        .collect();

    let count = to_flag.len();
    for (path, obs_idx, eff_conf) in to_flag {
        profile.review_queue.push(ReviewItem {
            id: Uuid::new_v4().to_string(),
            field: path,
            observation_index: obs_idx,
            effective_confidence: eff_conf,
            flagged_at: ProfileMeta::now_utc(),
            resolved: false,
        });
    }
    count
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::profile::ProfileDocument;
    use serde_json::json;

    /// Helper: build a minimal bridge packet with a single text observation.
    fn single_obs_packet(field: &str, value: serde_json::Value) -> BridgePacket {
        BridgePacket {
            bridge_version: "0.1".to_string(),
            orientation: "local:gemma3:4b".to_string(),
            session_ref: "test-session-abc".to_string(),
            timestamp: "2026-04-07T06:00:00+00:00".to_string(),
            observations: vec![BridgeObservation {
                field: field.to_string(),
                value,
                origination: BridgeOrigination::Passive,
                raw: None,
            }],
        }
    }

    #[test]
    fn ingest_text_to_identity_core() {
        let mut profile = ProfileDocument::new("test");
        let packet = single_obs_packet("identity.core", json!("curious"));

        let (proposed, deltas) = ingest_bridge_packet(&mut profile, &packet, "test.bridge.json");

        assert_eq!(proposed, 1);
        assert_eq!(deltas, 0);
        assert_eq!(profile.identity.core.len(), 1);

        let obs = &profile.identity.core[0].observations[0];
        assert_eq!(obs.status, ObservationStatus::Proposed);
        // Local passive × "local:..." prefix → 0.61
        assert!((obs.confidence - 0.61).abs() < f64::EPSILON);
    }

    #[test]
    fn ingest_to_working_field() {
        let mut profile = ProfileDocument::new("test");
        let packet = single_obs_packet("working.mode", json!("sketch-first"));

        let (proposed, _) = ingest_bridge_packet(&mut profile, &packet, "");
        assert_eq!(proposed, 1);

        let obs = &profile.working.mode.observations[0];
        assert_eq!(obs.status, ObservationStatus::Proposed);
    }

    #[test]
    fn delta_detected_on_conflict() {
        // Delta detection requires a SINGLETON field — one that route_field returns
        // by mutable reference rather than creating a new slot. "working.mode" is a
        // singleton: both packets land in the same ObservationField, so the second
        // can see the confirmed first and detect the conflict.
        //
        // List fields (identity.core, values, domains, signals.*) always create a
        // new slot per observation — they represent independent items, not alternative
        // values for the same trait, so they can never conflict.
        let mut profile = ProfileDocument::new("test");

        // First packet: "sketch-first" proposed, then manually confirm it.
        let p1 = single_obs_packet("working.mode", json!("sketch-first"));
        ingest_bridge_packet(&mut profile, &p1, "p1.bridge.json");
        profile.working.mode.observations[0].status = ObservationStatus::Confirmed;

        // Second packet: "spec-first" — conflicts with confirmed "sketch-first".
        let p2 = single_obs_packet("working.mode", json!("spec-first"));
        let (proposed, deltas) = ingest_bridge_packet(&mut profile, &p2, "p2.bridge.json");

        assert_eq!(proposed, 0);
        assert_eq!(deltas, 1);
        assert_eq!(profile.delta_queue.len(), 1);

        // Both observations are now in delta status.
        let obs = &profile.working.mode.observations;
        assert!(obs.iter().all(|o| o.status == ObservationStatus::Delta));
    }

    #[test]
    fn unknown_path_is_skipped() {
        let mut profile = ProfileDocument::new("test");
        let packet = single_obs_packet("nonexistent.path", json!("value"));

        let (proposed, deltas) = ingest_bridge_packet(&mut profile, &packet, "");
        assert_eq!(proposed, 0);
        assert_eq!(deltas, 0);
    }

    #[test]
    fn bridge_log_entry_appended() {
        let mut profile = ProfileDocument::new("test");
        let packet = single_obs_packet("values", json!("open source"));

        ingest_bridge_packet(&mut profile, &packet, "session1.bridge.json");

        assert_eq!(profile.bridge_log.processed.len(), 1);
        assert_eq!(
            profile.bridge_log.processed[0].filename,
            "session1.bridge.json"
        );
        assert_eq!(profile.bridge_log.processed[0].observations_proposed, 1);
    }

    #[test]
    fn corroboration_bonus_applied() {
        // Corroboration requires two observations with the SAME VALUE from DIFFERENT
        // orientations in the SAME ObservationField. We use "identity.reasoning.style"
        // — a singleton field — so both packets land in the same field. List paths
        // (values, domains, etc.) create a new slot per packet, so their observations
        // end up in separate fields and can't corroborate each other.
        let mut profile = ProfileDocument::new("test");

        let make_packet = |orientation: &str, session: &str| BridgePacket {
            bridge_version: "0.1".to_string(),
            orientation: orientation.to_string(),
            session_ref: session.to_string(),
            timestamp: "2026-04-07T06:00:00+00:00".to_string(),
            observations: vec![BridgeObservation {
                field: "identity.reasoning.style".to_string(),
                value: json!("systems-first"),
                origination: BridgeOrigination::Passive,
                raw: None,
            }],
        };

        ingest_bridge_packet(
            &mut profile,
            &make_packet("local:gemma3:4b", "s1"),
            "s1.bridge.json",
        );
        ingest_bridge_packet(
            &mut profile,
            &make_packet("local:llama3:8b", "s2"),
            "s2.bridge.json",
        );

        // Manually confirm both (simulating user review pass).
        for obs in &mut profile.identity.reasoning.style.observations {
            obs.status = ObservationStatus::Confirmed;
        }

        let boosted = run_corroboration(&mut profile);
        assert_eq!(boosted, 2);

        // Each should have gained +0.08 on top of its base 0.61.
        for obs in &profile.identity.reasoning.style.observations {
            assert!((obs.confidence - (0.61 + CORROBORATION_BONUS)).abs() < 1e-9);
        }
    }
}
