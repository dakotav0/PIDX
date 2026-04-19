//! Tauri command handlers.
//!
//! Each function is a thin async shim over the `pidx` library. The pattern:
//!
//! 1. Acquire the `AppState` lock (read or write depending on the command)
//! 2. Delegate to the pidx library function
//! 3. Return a serializable result — Tauri auto-serializes to JSON for the frontend
//!
//! ## Rust lesson: `#[tauri::command]`
//!
//! This attribute turns a regular async fn into a Tauri IPC handler. The function
//! signature determines the JSON schema the frontend must call it with. Tauri
//! deserializes arguments from JSON automatically — you don't write any glue code.
//! `State<T>` injects the managed app state (the Arc<RwLock<_>>) without cloning.
//!
//! ## Error handling
//!
//! Tauri commands return `Result<T, String>` where `String` is the error message
//! sent back to the frontend. `anyhow::Error` implements `.to_string()` so we map
//! with `.map_err(|e| e.to_string())` rather than writing custom error types yet.

use std::collections::HashMap;
use std::sync::Arc;

use tauri::State;
use tokio::sync::RwLock;
use tracing::{info, warn};

use pidx::models::profile::ProfileDocument;
use pidx::output::Tier;
use pidx::{
    confirm_all_proposed, ingest_bridge_packet, reject_all_proposed, render_tier_output,
    run_corroboration, run_decay_pass, ProfileStore,
};

// ── Shared app state ─────────────────────────────────────────────────────────

/// In-memory profile cache shared across all Tauri commands.
///
/// `Arc<RwLock<_>>` means:
/// - `Arc` — reference-counted, so multiple async tasks can hold a handle
/// - `RwLock` — many concurrent readers OR one exclusive writer; no data races
///
/// This is the canonical Rust pattern for shared mutable state in async code.
/// The cache is never written to disk here — that's still `ProfileStore::save`.
pub struct AppState {
    /// In-memory cache: user_id → loaded profile
    pub cache: Arc<RwLock<HashMap<String, ProfileDocument>>>,
    /// Disk persistence layer (sync, used via spawn_blocking in write commands)
    pub store: ProfileStore,
}

impl AppState {
    pub fn new() -> Self {
        let dir = find_profiles_dir();
        tracing::info!(profiles_dir = %dir.display(), "AppState initialised");
        Self {
            cache: Arc::new(RwLock::new(HashMap::new())),
            store: ProfileStore::new(dir),
        }
    }
}

impl Default for AppState {
    fn default() -> Self {
        Self::new()
    }
}

/// Resolve the profiles directory at runtime.
///
/// Priority:
/// 1. `PIDX_PROFILES_DIR` env var — same contract as the CLI
/// 2. Walk UP from `current_exe()` looking for an existing `profiles/` dir.
///    In Tauri dev the binary lives at `<workspace>/target/debug/pidx-ui.exe`;
///    3 hops up reaches the workspace root which has `profiles/` in it.
///    This makes `cargo tauri dev` work out of the box without any config.
/// 3. `./profiles` — original CLI fallback (CWD-relative)
fn find_profiles_dir() -> std::path::PathBuf {
    // 1. Env var
    if let Ok(dir) = std::env::var("PIDX_PROFILES_DIR") {
        return std::path::PathBuf::from(dir);
    }

    // 2. Walk up from the binary
    if let Ok(exe) = std::env::current_exe() {
        let mut candidate = exe;
        for _ in 0..6 {
            candidate = match candidate.parent() {
                Some(p) => p.to_path_buf(),
                None => break,
            };
            let profiles = candidate.join("profiles");
            if profiles.is_dir() {
                return profiles;
            }
        }
    }

    // 3. Fallback
    std::path::PathBuf::from("profiles")
}

// ── Helper: load profile (from cache or disk) ─────────────────────────────────

/// Load a profile — returns cached version if present, otherwise reads from disk
/// and populates the cache. Read lock path only; no write to disk.
async fn load_cached(state: &AppState, user_id: &str) -> anyhow::Result<ProfileDocument> {
    // Fast path: already in cache
    {
        let cache = state.cache.read().await;
        if let Some(profile) = cache.get(user_id) {
            return Ok(profile.clone());
        }
    }

    // Slow path: load from disk, insert into cache
    let profile = state.store.load_or_create(user_id)?;
    {
        let mut cache = state.cache.write().await;
        cache.insert(user_id.to_string(), profile.clone());
    }
    Ok(profile)
}

/// Invalidate a user's cache entry after a write command mutates and saves the profile.
async fn invalidate(state: &AppState, user_id: &str) {
    let mut cache = state.cache.write().await;
    cache.remove(user_id);
}

// ── Read commands ─────────────────────────────────────────────────────────────

/// List all profiles in PIDX_PROFILES_DIR.
#[tauri::command]
pub async fn list_users(state: State<'_, AppState>) -> Result<serde_json::Value, String> {
    use std::fs;

    // Use the directory that AppState resolved (via find_profiles_dir walk-up),
    // NOT ProfileStore::default_dir() which only checks CWD/env-var and will
    // point to the wrong path in a Tauri dev session.
    let dir = state.store.dir();
    let mut users: Vec<serde_json::Value> = Vec::new();

    if dir.exists() {
        let entries = fs::read_dir(dir).map_err(|e| e.to_string())?;
        for entry in entries.flatten() {
            let path = entry.path();
            let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
                continue;
            };
            let Some(user_id) = name.strip_suffix(".pidx.json") else {
                continue;
            };
            if let Ok(profile) = load_cached(&state, user_id).await {
                users.push(serde_json::json!({
                    "user_id": user_id,
                    "version": profile.meta.version,
                    "updated": profile.meta.updated,
                    "overall_confidence": profile.meta.overall_confidence,
                }));
            }
        }
    }

    users.sort_by(|a, b| {
        let a = a["user_id"].as_str().unwrap_or("");
        let b = b["user_id"].as_str().unwrap_or("");
        a.cmp(b)
    });

    info!(count = users.len(), "list_users");
    Ok(serde_json::json!({ "count": users.len(), "users": users }))
}

/// Get the full profile document as JSON.
#[tauri::command]
pub async fn get_profile(
    user_id: String,
    state: State<'_, AppState>,
) -> Result<serde_json::Value, String> {
    let mut profile = load_cached(&state, &user_id)
        .await
        .map_err(|e| e.to_string())?;
    profile.recompute_overall_confidence();
    serde_json::to_value(&profile).map_err(|e| e.to_string())
}

/// Render a tier-scaled context block.
#[tauri::command]
pub async fn get_show(
    user_id: String,
    tier: String,
    state: State<'_, AppState>,
) -> Result<String, String> {
    let tier: Tier = tier.parse().map_err(|e: String| e)?;
    let mut profile = load_cached(&state, &user_id)
        .await
        .map_err(|e| e.to_string())?;
    Ok(render_tier_output(&mut profile, tier))
}

/// Get observation status summary.
#[tauri::command]
pub async fn get_status(
    user_id: String,
    state: State<'_, AppState>,
) -> Result<serde_json::Value, String> {
    let mut profile = load_cached(&state, &user_id)
        .await
        .map_err(|e| e.to_string())?;
    profile.recompute_overall_confidence();

    let fields: Vec<serde_json::Value> = {
        use pidx::models::observation::ObservationStatus;

        // Walk all fields in the same order as the CLI `all_fields` helper
        let mut summaries = Vec::new();
        let field_iter: Vec<(&str, &pidx::models::observation::ObservationField)> = vec![
            ("working.mode", &profile.working.mode),
            ("working.pace", &profile.working.pace),
            ("working.feedback", &profile.working.feedback),
            ("working.pattern", &profile.working.pattern),
            (
                "identity.reasoning.style",
                &profile.identity.reasoning.style,
            ),
            (
                "identity.reasoning.pattern",
                &profile.identity.reasoning.pattern,
            ),
            (
                "identity.reasoning.intake",
                &profile.identity.reasoning.intake,
            ),
            (
                "identity.reasoning.stance",
                &profile.identity.reasoning.stance,
            ),
        ];

        for (path, field) in field_iter {
            if field.observations.is_empty() {
                continue;
            }
            let c = field
                .observations
                .iter()
                .filter(|o| o.status == ObservationStatus::Confirmed)
                .count();
            let p = field
                .observations
                .iter()
                .filter(|o| o.status == ObservationStatus::Proposed)
                .count();
            let d = field
                .observations
                .iter()
                .filter(|o| o.status == ObservationStatus::Delta)
                .count();
            summaries.push(
                serde_json::json!({ "path": path, "confirmed": c, "proposed": p, "delta": d }),
            );
        }

        // List fields
        macro_rules! push_list {
            ($list:expr, $prefix:expr) => {
                for (i, f) in $list.iter().enumerate() {
                    if f.observations.is_empty() { continue; }
                    let c = f.observations.iter().filter(|o| o.status == ObservationStatus::Confirmed).count();
                    let p = f.observations.iter().filter(|o| o.status == ObservationStatus::Proposed).count();
                    let d = f.observations.iter().filter(|o| o.status == ObservationStatus::Delta).count();
                    summaries.push(serde_json::json!({ "path": format!("{}.{i}", $prefix), "confirmed": c, "proposed": p, "delta": d }));
                }
            };
        }
        push_list!(profile.identity.core, "identity.core");
        push_list!(profile.domains, "domains");
        push_list!(profile.values, "values");
        push_list!(profile.signals.phrases, "signals.phrases");
        push_list!(profile.signals.avoidances, "signals.avoidances");
        push_list!(profile.signals.rhythms, "signals.rhythms");
        push_list!(profile.signals.framings, "signals.framings");

        summaries
    };

    Ok(serde_json::json!({
        "user_id": user_id,
        "version": profile.meta.version,
        "overall_confidence": profile.meta.overall_confidence,
        "updated": profile.meta.updated,
        "fields": fields,
        "delta_queue_open": profile.delta_queue.iter().filter(|d| !d.resolved).count(),
        "review_queue_pending": profile.review_queue.iter().filter(|r| !r.resolved).count(),
    }))
}

// ── Write commands ────────────────────────────────────────────────────────────

/// Flip a proposed observation to confirmed.
#[tauri::command]
pub async fn confirm_observation(
    user_id: String,
    field: String,
    index: usize,
    state: State<'_, AppState>,
) -> Result<serde_json::Value, String> {
    use pidx::models::observation::ObservationStatus;

    let mut profile = state
        .store
        .load_or_create(&user_id)
        .map_err(|e| e.to_string())?;

    let obs = find_obs_mut(&mut profile, &field, index)?;
    if obs.status != ObservationStatus::Proposed {
        return Err(format!("observation is {:?}, not Proposed", obs.status));
    }
    let val = format!("{:?}", obs.value);
    obs.status = ObservationStatus::Confirmed;

    state.store.save(&mut profile).map_err(|e| e.to_string())?;
    invalidate(&state, &user_id).await;
    info!(user_id, field, index, "confirmed");

    Ok(
        serde_json::json!({ "ok": true, "field": field, "index": index, "value": val, "new_status": "confirmed" }),
    )
}

/// Reject a proposed observation.
#[tauri::command]
pub async fn reject_observation(
    user_id: String,
    field: String,
    index: usize,
    state: State<'_, AppState>,
) -> Result<serde_json::Value, String> {
    use pidx::models::observation::ObservationStatus;

    let mut profile = state
        .store
        .load_or_create(&user_id)
        .map_err(|e| e.to_string())?;

    let obs = find_obs_mut(&mut profile, &field, index)?;
    if obs.status != ObservationStatus::Proposed {
        return Err(format!("observation is {:?}, not Proposed", obs.status));
    }
    obs.status = ObservationStatus::Rejected;

    state.store.save(&mut profile).map_err(|e| e.to_string())?;
    invalidate(&state, &user_id).await;
    warn!(user_id, field, index, "rejected");

    Ok(serde_json::json!({ "ok": true, "field": field, "index": index, "new_status": "rejected" }))
}

/// Ingest a bridge packet file into a profile.
#[tauri::command]
pub async fn ingest_packet(
    user_id: String,
    packet_path: String,
    state: State<'_, AppState>,
) -> Result<serde_json::Value, String> {
    use pidx::models::bridge::BridgePacket;

    let raw = std::fs::read_to_string(&packet_path)
        .map_err(|e| format!("cannot read {packet_path}: {e}"))?;
    let packet: BridgePacket =
        serde_json::from_str(&raw).map_err(|e| format!("invalid bridge packet: {e}"))?;

    let filename = std::path::Path::new(&packet_path)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("unknown.bridge.json");

    let mut profile = state
        .store
        .load_or_create(&user_id)
        .map_err(|e| e.to_string())?;
    let (proposed, deltas) = ingest_bridge_packet(&mut profile, &packet, filename);
    run_corroboration(&mut profile);
    state.store.save(&mut profile).map_err(|e| e.to_string())?;
    invalidate(&state, &user_id).await;

    info!(user_id, proposed, deltas, "ingest_packet");
    Ok(
        serde_json::json!({ "ok": true, "observations_proposed": proposed, "deltas_flagged": deltas }),
    )
}

/// Resolve a delta conflict — keep one side, reject the other.
#[tauri::command]
pub async fn resolve_delta(
    user_id: String,
    delta_id: String,
    keep: String,
    state: State<'_, AppState>,
) -> Result<serde_json::Value, String> {
    use pidx::models::observation::ObservationStatus;

    let mut profile = state
        .store
        .load_or_create(&user_id)
        .map_err(|e| e.to_string())?;

    // Pull what we need before mutating — borrow-split pattern.
    let (field_path, keep_session, reject_session) = {
        let d = profile
            .delta_queue
            .iter()
            .find(|d| d.id == delta_id && !d.resolved)
            .ok_or_else(|| format!("no open delta with id '{delta_id}'"))?;
        let (keep_obs, reject_obs) = if keep == "a" {
            (&d.a, &d.b)
        } else {
            (&d.b, &d.a)
        };
        (
            d.field.clone(),
            keep_obs.source.session_ref.clone(),
            reject_obs.source.session_ref.clone(),
        )
    };

    for d in profile.delta_queue.iter_mut() {
        if d.id == delta_id {
            d.resolved = true;
            break;
        }
    }

    if let Some(field) = resolve_field_mut(&mut profile, &field_path) {
        for obs in field.observations.iter_mut() {
            if obs.status == ObservationStatus::Delta {
                if obs.source.session_ref == keep_session {
                    obs.status = ObservationStatus::Confirmed;
                } else if obs.source.session_ref == reject_session {
                    obs.status = ObservationStatus::Rejected;
                }
            }
        }
    }

    state.store.save(&mut profile).map_err(|e| e.to_string())?;
    invalidate(&state, &user_id).await;
    info!(user_id, delta_id, keep, "resolve_delta");

    Ok(serde_json::json!({ "ok": true, "delta_id": delta_id, "kept": keep, "field": field_path }))
}

/// Confirm all proposed observations whose path starts with a prefix.
#[tauri::command]
pub async fn confirm_all(
    user_id: String,
    field_prefix: String,
    state: State<'_, AppState>,
) -> Result<serde_json::Value, String> {
    let mut profile = state
        .store
        .load_or_create(&user_id)
        .map_err(|e| e.to_string())?;
    let confirmed_fields = confirm_all_proposed(&mut profile, &field_prefix);
    let count = confirmed_fields.len();
    if count > 0 {
        profile.recompute_overall_confidence();
        state.store.save(&mut profile).map_err(|e| e.to_string())?;
        invalidate(&state, &user_id).await;
    }
    info!(user_id, field_prefix, count, "confirm_all");
    Ok(serde_json::json!({ "ok": true, "confirmed_count": count, "fields": confirmed_fields }))
}

/// Reject all proposed observations whose path starts with a prefix.
#[tauri::command]
pub async fn reject_all(
    user_id: String,
    field_prefix: String,
    state: State<'_, AppState>,
) -> Result<serde_json::Value, String> {
    let mut profile = state
        .store
        .load_or_create(&user_id)
        .map_err(|e| e.to_string())?;
    let rejected_fields = reject_all_proposed(&mut profile, &field_prefix);
    let count = rejected_fields.len();
    if count > 0 {
        state.store.save(&mut profile).map_err(|e| e.to_string())?;
        invalidate(&state, &user_id).await;
    }
    info!(user_id, field_prefix, count, "reject_all");
    Ok(serde_json::json!({ "ok": true, "rejected_count": count, "fields": rejected_fields }))
}

/// Clear specific pending queues or unconfirmed observations.
#[tauri::command]
pub async fn clear(
    user_id: String,
    target: String,
    state: State<'_, AppState>,
) -> Result<serde_json::Value, String> {
    let mut profile = state
        .store
        .load_or_create(&user_id)
        .map_err(|e| e.to_string())?;
    let mut cleared_count = 0;

    if target == "deltas" || target == "all" {
        cleared_count += profile.delta_queue.len();
        profile.delta_queue.clear();
    }
    if target == "reviews" || target == "all" {
        cleared_count += profile.review_queue.len();
        profile.review_queue.clear();
    }
    if target == "proposed" || target == "all" {
        let matching = reject_all_proposed(&mut profile, "");
        cleared_count += matching.len();
    }

    if cleared_count > 0 {
        state.store.save(&mut profile).map_err(|e| e.to_string())?;
        invalidate(&state, &user_id).await;
    }
    info!(user_id, target, cleared_count, "clear");
    Ok(serde_json::json!({ "ok": true, "target": target, "cleared_count": cleared_count }))
}

/// Add an annotation to a field.
#[tauri::command]
pub async fn annotate(
    user_id: String,
    field: String,
    note: String,
    pinned: bool,
    state: State<'_, AppState>,
) -> Result<serde_json::Value, String> {
    use pidx::models::profile::{Annotation, ProfileMeta};

    let mut profile = state
        .store
        .load_or_create(&user_id)
        .map_err(|e| e.to_string())?;

    if resolve_field_mut(&mut profile, &field).is_none() {
        return Err(format!("unknown field path '{field}'"));
    }

    let id = uuid::Uuid::new_v4().to_string();
    profile.annotations.push(Annotation {
        id: id.clone(),
        field: field.clone(),
        note: note.clone(),
        author: "user".to_string(),
        created_at: ProfileMeta::now_utc(),
        pinned,
    });

    state.store.save(&mut profile).map_err(|e| e.to_string())?;
    invalidate(&state, &user_id).await;
    info!(user_id, field, pinned, "annotate");

    Ok(serde_json::json!({ "ok": true, "id": id, "field": field, "note": note, "pinned": pinned }))
}

/// Apply time-based confidence decay and flag low-confidence observations for review.
#[tauri::command]
pub async fn decay(
    user_id: String,
    threshold: Option<f64>,
    state: State<'_, AppState>,
) -> Result<serde_json::Value, String> {
    let threshold = threshold.unwrap_or(0.30);
    let mut profile = state
        .store
        .load_or_create(&user_id)
        .map_err(|e| e.to_string())?;
    let newly_flagged = run_decay_pass(&mut profile, threshold);
    if newly_flagged > 0 {
        profile.recompute_overall_confidence();
        state.store.save(&mut profile).map_err(|e| e.to_string())?;
        invalidate(&state, &user_id).await;
    }
    let pending = profile.review_queue.iter().filter(|r| !r.resolved).count();
    info!(user_id, newly_flagged, pending, "decay");
    Ok(serde_json::json!({
        "ok": true,
        "threshold": threshold,
        "newly_flagged": newly_flagged,
        "review_queue_pending": pending,
    }))
}

/// Ingest a bridge packet supplied as a JSON string (UI-authored, no file needed).
#[tauri::command]
pub async fn ingest_packet_content(
    user_id: String,
    packet_json: String,
    state: State<'_, AppState>,
) -> Result<serde_json::Value, String> {
    use pidx::models::bridge::BridgePacket;

    let packet: BridgePacket =
        serde_json::from_str(&packet_json).map_err(|e| format!("invalid bridge packet: {e}"))?;

    let mut profile = state
        .store
        .load_or_create(&user_id)
        .map_err(|e| e.to_string())?;
    let (proposed, deltas) = ingest_bridge_packet(&mut profile, &packet, "ui-authored");
    run_corroboration(&mut profile);
    state.store.save(&mut profile).map_err(|e| e.to_string())?;
    invalidate(&state, &user_id).await;

    info!(user_id, proposed, deltas, "ingest_packet_content");
    Ok(serde_json::json!({ "ok": true, "observations_proposed": proposed, "deltas_flagged": deltas }))
}

/// Resolve a decayed observation from the review queue.
///
/// `action` is `"keep"` (leave the observation as-is, dismiss the review item)
/// or `"discard"` (archive the observation and dismiss the review item).
#[tauri::command]
pub async fn resolve_review(
    user_id: String,
    review_id: String,
    action: String,
    state: State<'_, AppState>,
) -> Result<serde_json::Value, String> {
    use pidx::models::observation::ObservationStatus;

    let mut profile = state
        .store
        .load_or_create(&user_id)
        .map_err(|e| e.to_string())?;

    let (field, obs_index) = {
        let item = profile
            .review_queue
            .iter()
            .find(|r| r.id == review_id && !r.resolved)
            .ok_or_else(|| format!("no open review with id '{review_id}'"))?;
        (item.field.clone(), item.observation_index)
    };

    for r in profile.review_queue.iter_mut() {
        if r.id == review_id {
            r.resolved = true;
            break;
        }
    }

    if action == "discard" {
        if let Some(field_ref) = resolve_field_mut(&mut profile, &field) {
            if let Some(obs) = field_ref.observations.get_mut(obs_index) {
                obs.status = ObservationStatus::Archived;
            }
        }
    }

    state.store.save(&mut profile).map_err(|e| e.to_string())?;
    invalidate(&state, &user_id).await;
    info!(user_id, review_id, action, "resolve_review");

    Ok(serde_json::json!({ "ok": true, "review_id": review_id, "action": action, "field": field }))
}

// ── Internal helpers ──────────────────────────────────────────────────────────

/// Resolve a field path to a mutable observation reference.
/// Mirrors `resolve_field_mut` in the CLI but returns a `Result<&mut Observation>`.
fn find_obs_mut<'a>(
    profile: &'a mut ProfileDocument,
    path: &str,
    index: usize,
) -> Result<&'a mut pidx::models::observation::Observation, String> {
    let field =
        resolve_field_mut(profile, path).ok_or_else(|| format!("unknown field path '{path}'"))?;
    let len = field.observations.len();
    field
        .observations
        .get_mut(index)
        .ok_or_else(|| format!("index {index} out of range (field has {len} observations)"))
}

/// Resolve a dot-path to a mutable `ObservationField`.
/// Duplicates the CLI helper — will be unified when shared logic moves to pidx::lib.
fn resolve_field_mut<'a>(
    profile: &'a mut ProfileDocument,
    path: &str,
) -> Option<&'a mut pidx::models::observation::ObservationField> {
    let parts: Vec<&str> = path.splitn(3, '.').collect();
    match parts.as_slice() {
        ["identity", "core", rest] => {
            let idx: usize = rest.parse().ok()?;
            profile.identity.core.get_mut(idx)
        }
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
            let idx: usize = idx.parse().ok()?;
            match *cat {
                "phrases" => profile.signals.phrases.get_mut(idx),
                "avoidances" => profile.signals.avoidances.get_mut(idx),
                "rhythms" => profile.signals.rhythms.get_mut(idx),
                "framings" => profile.signals.framings.get_mut(idx),
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
