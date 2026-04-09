#![allow(clippy::enum_variant_names)]
//! Tool definitions and handler for the PIDX MCP server.
//!
//! Each tool struct is annotated with `#[mcp_tool]` (name + description shown to
//! the AI client) and derives `JsonSchema` + `Deserialize` (generates the JSON Schema
//! the client validates arguments against).
//!
//! `PidxHandler` implements `ServerHandler`:
//!   - `handle_list_tools_request` — returns all tool descriptors via `tool_box!`
//!   - `handle_call_tool_request`  — matches by tool name, calls the pidx lib
//!
//! ## Error handling convention
//!
//! `CallToolError` wraps a `std::error::Error`. We use plain `String` errors
//! (via `.map_err(|e| e.to_string())`) rather than `anyhow::Error` to stay
//! compatible with the SDK's trait bounds.

use async_trait::async_trait;
use rust_mcp_sdk::{
    macros::{self, JsonSchema},
    mcp_server::ServerHandler,
    schema::{
        CallToolError, CallToolRequestParams, CallToolResult, ListToolsResult,
        PaginatedRequestParams, RpcError,
    },
    tool_box, McpServer,
};
use serde::Deserialize;
use tracing::{info, warn};
use uuid::Uuid;

use pidx::{
    confirm_all_proposed, reject_all_proposed, ingest_bridge_packet, render_tier_output, run_corroboration,
    run_decay_pass, ProfileStore,
};

// ── Tool input structs ────────────────────────────────────────────────────────

/// List all profiles known to PIDX.
#[macros::mcp_tool(
    name = "pidx_list",
    description = "List all PIDX personality profiles. Returns user IDs, version, confidence score, and last-updated timestamp."
)]
#[derive(Debug, Deserialize, JsonSchema)]
pub struct PidxListTool {}

/// Retrieve a tier-scaled context block for a user.
#[macros::mcp_tool(
    name = "pidx_show",
    description = "Return a formatted context block for a user scaled to the requested tier (T1=minimal, T2=standard, T3=full). T1 fits in small context windows; T3 includes all confirmed observations."
)]
#[derive(Debug, Deserialize, JsonSchema)]
pub struct PidxShowTool {
    /// User ID (e.g. `test_user` or `dakota`)
    pub user_id: String,
    /// Output tier: `T1`, `T2`, or `T3`
    pub tier: String,
}

/// Get the observation status summary for a user.
#[macros::mcp_tool(
    name = "pidx_status",
    description = "Return a per-field observation summary: how many are confirmed, proposed, and flagged as deltas. Also reports open delta and review queue counts."
)]
#[derive(Debug, Deserialize, JsonSchema)]
pub struct PidxStatusTool {
    /// User ID
    pub user_id: String,
}

/// Ingest a bridge packet JSON file into a profile.
#[macros::mcp_tool(
    name = "pidx_ingest",
    description = "Ingest a bridge packet JSON file into a PIDX profile. The packet contains raw observations produced by an AI orientation layer. Returns counts of proposed observations and delta flags raised."
)]
#[derive(Debug, Deserialize, JsonSchema)]
pub struct PidxIngestTool {
    /// User ID
    pub user_id: String,
    /// Absolute path to the `.bridge.json` packet file
    pub packet_path: String,
}

/// Confirm a proposed observation.
#[macros::mcp_tool(
    name = "pidx_confirm",
    description = "Flip a proposed observation to confirmed status. Use pidx_status to find field paths and observation indexes."
)]
#[derive(Debug, Deserialize, JsonSchema)]
pub struct PidxConfirmTool {
    /// User ID
    pub user_id: String,
    /// Dot-path to the field (e.g. `working.mode`, `identity.reasoning.style`)
    pub field: String,
    /// Zero-based observation index within that field
    pub index: u64,
}

/// Reject a proposed observation.
#[macros::mcp_tool(
    name = "pidx_reject",
    description = "Reject a proposed observation, removing it from active consideration. The observation is marked rejected but not deleted, preserving audit history."
)]
#[derive(Debug, Deserialize, JsonSchema)]
pub struct PidxRejectTool {
    /// User ID
    pub user_id: String,
    /// Dot-path to the field
    pub field: String,
    /// Zero-based observation index
    pub index: u64,
}

/// Bulk-confirm all proposed observations under a field prefix.
#[macros::mcp_tool(
    name = "pidx_confirm_all",
    description = "Confirm all proposed observations under a dot-path prefix (e.g. `identity.core`, `signals`, `working`). Returns the count and list of fields updated."
)]
#[derive(Debug, Deserialize, JsonSchema)]
pub struct PidxConfirmAllTool {
    /// User ID
    pub user_id: String,
    /// Dot-path prefix to confirm under (e.g. `identity.core`, `working`)
    pub field_prefix: String,
}

/// Bulk-reject all proposed observations under a field prefix.
#[macros::mcp_tool(
    name = "pidx_reject_all",
    description = "Reject all proposed observations under a dot-path prefix. Returns the count and list of fields updated."
)]
#[derive(Debug, Deserialize, JsonSchema)]
pub struct PidxRejectAllTool {
    /// User ID
    pub user_id: String,
    /// Dot-path prefix to reject under
    pub field_prefix: String,
}

/// Clear specific pending queues or unconfirmed observations.
#[macros::mcp_tool(
    name = "pidx_clear",
    description = "Clear specific pending queues or unconfirmed observations from the profile. Target must be \"deltas\", \"reviews\", \"proposed\", or \"all\"."
)]
#[derive(Debug, Deserialize, JsonSchema)]
pub struct PidxClearTool {
    /// User ID
    pub user_id: String,
    /// Which data to clear: "deltas", "reviews", "proposed", or "all"
    pub target: String,
}

/// List open delta conflicts for a user.
#[macros::mcp_tool(
    name = "pidx_delta_list",
    description = "List all open (unresolved) delta conflicts for a user. Each delta has an ID, the conflicting field path, and both candidate observations (a and b)."
)]
#[derive(Debug, Deserialize, JsonSchema)]
pub struct PidxDeltaListTool {
    /// User ID
    pub user_id: String,
}

/// Resolve a delta conflict by choosing a or b.
#[macros::mcp_tool(
    name = "pidx_delta_resolve",
    description = "Resolve an open delta conflict. Pass the delta ID from pidx_delta_list and `keep` = \"a\" or \"b\" to choose which observation becomes confirmed (the other is rejected)."
)]
#[derive(Debug, Deserialize, JsonSchema)]
pub struct PidxDeltaResolveTool {
    /// User ID
    pub user_id: String,
    /// Delta ID from pidx_delta_list
    pub delta_id: String,
    /// Which side to keep: `"a"` or `"b"`
    pub keep: String,
}

/// List observations flagged for review by the decay pass.
#[macros::mcp_tool(
    name = "pidx_review_list",
    description = "List all observations pending review (flagged by the decay pass as low-confidence). Each item has an ID, field path, index, and effective confidence."
)]
#[derive(Debug, Deserialize, JsonSchema)]
pub struct PidxReviewListTool {
    /// User ID
    pub user_id: String,
}

/// Process a review item: solidify or discard the observation.
#[macros::mcp_tool(
    name = "pidx_review_process",
    description = "Process a review item. action = \"solidify\" marks the observation decay-exempt and confirms it; action = \"discard\" archives it."
)]
#[derive(Debug, Deserialize, JsonSchema)]
pub struct PidxReviewProcessTool {
    /// User ID
    pub user_id: String,
    /// Review item ID from pidx_review_list
    pub review_id: String,
    /// `"solidify"` to keep and exempt from decay, `"discard"` to archive
    pub action: String,
}

/// Attach a note to a profile field.
#[macros::mcp_tool(
    name = "pidx_annotate",
    description = "Attach a text note to a profile field. Pinned annotations appear in Rich-tier output. Author is set to \"mcp\"."
)]
#[derive(Debug, Deserialize, JsonSchema)]
pub struct PidxAnnotateTool {
    /// User ID
    pub user_id: String,
    /// Dot-path to the field to annotate (e.g. `working.mode`)
    pub field: String,
    /// The annotation text
    pub note: String,
    /// Whether to pin this annotation to Rich-tier output
    pub pinned: bool,
}

/// Compare two PIDX profiles and return a structured diff.
#[macros::mcp_tool(
    name = "pidx_diff",
    description = "Compare two PIDX profiles and return a structured diff of their register metrics, working style, and core identity observations."
)]
#[derive(Debug, Deserialize, JsonSchema)]
pub struct PidxDiffTool {
    /// First user ID
    pub user_a: String,
    /// Second user ID
    pub user_b: String,
}

/// Run a decay pass to flag low-confidence observations for review.
#[macros::mcp_tool(
    name = "pidx_decay",
    description = "Run a decay pass over all confirmed observations. Any observation whose effective confidence has fallen below the threshold is moved to the review queue. Returns the count of newly flagged observations."
)]
#[derive(Debug, Deserialize, JsonSchema)]
pub struct PidxDecayTool {
    /// User ID
    pub user_id: String,
    /// Confidence threshold below which observations are flagged (default 0.30)
    pub threshold: Option<f64>,
}

// Register all tools — generates `PidxTools` enum + `PidxTools::tools()` vec
tool_box!(PidxTools, [
    PidxListTool,
    PidxShowTool,
    PidxStatusTool,
    PidxIngestTool,
    PidxConfirmTool,
    PidxRejectTool,
    PidxConfirmAllTool,
    PidxRejectAllTool,
    PidxClearTool,
    PidxDeltaListTool,
    PidxDeltaResolveTool,
    PidxReviewListTool,
    PidxReviewProcessTool,
    PidxAnnotateTool,
    PidxDiffTool,
    PidxDecayTool
]);

// ── Handler ───────────────────────────────────────────────────────────────────

pub struct PidxHandler {
    store: ProfileStore,
}

impl PidxHandler {
    pub fn new() -> Self {
        let dir = ProfileStore::default_dir();
        tracing::info!(profiles_dir = %dir.display(), "PidxHandler initialised");
        Self {
            store: ProfileStore::new(dir),
        }
    }
}

// ── ServerHandler impl ────────────────────────────────────────────────────────

#[async_trait]
impl ServerHandler for PidxHandler {
    async fn handle_list_tools_request(
        &self,
        _request: Option<PaginatedRequestParams>,
        _runtime: std::sync::Arc<dyn McpServer>,
    ) -> std::result::Result<ListToolsResult, RpcError> {
        Ok(ListToolsResult {
            tools: PidxTools::tools(),
            meta: None,
            next_cursor: None,
        })
    }

    async fn handle_call_tool_request(
        &self,
        params: CallToolRequestParams,
        _runtime: std::sync::Arc<dyn McpServer>,
    ) -> std::result::Result<CallToolResult, CallToolError> {
        let tool = PidxTools::try_from(params.clone())
            .map_err(|_| CallToolError::unknown_tool(params.name.clone()))?;

        match tool {
            PidxTools::PidxListTool(_)           => self.handle_list().await,
            PidxTools::PidxShowTool(t)           => self.handle_show(t).await,
            PidxTools::PidxStatusTool(t)         => self.handle_status(t).await,
            PidxTools::PidxIngestTool(t)         => self.handle_ingest(t).await,
            PidxTools::PidxConfirmTool(t)        => self.handle_confirm(t).await,
            PidxTools::PidxRejectTool(t)         => self.handle_reject(t).await,
            PidxTools::PidxConfirmAllTool(t)     => self.handle_confirm_all(t).await,
            PidxTools::PidxRejectAllTool(t)      => self.handle_reject_all(t).await,
            PidxTools::PidxClearTool(t)          => self.handle_clear(t).await,
            PidxTools::PidxDeltaListTool(t)      => self.handle_delta_list(t).await,
            PidxTools::PidxDeltaResolveTool(t)   => self.handle_delta_resolve(t).await,
            PidxTools::PidxReviewListTool(t)     => self.handle_review_list(t).await,
            PidxTools::PidxReviewProcessTool(t)  => self.handle_review_process(t).await,
            PidxTools::PidxAnnotateTool(t)       => self.handle_annotate(t).await,
            PidxTools::PidxDiffTool(t)           => self.handle_diff(t).await,
            PidxTools::PidxDecayTool(t)          => self.handle_decay(t).await,
        }
    }
}

// ── Tool implementations ──────────────────────────────────────────────────────

type ToolResult = std::result::Result<CallToolResult, CallToolError>;

/// Convert any Display-able error into a CallToolError.
fn tool_err(msg: impl std::fmt::Display) -> CallToolError {
    CallToolError::new(std::io::Error::other(msg.to_string()))
}

/// Serialize a value to pretty JSON and wrap in a text content result.
fn json_text(val: &serde_json::Value) -> ToolResult {
    Ok(CallToolResult::text_content(vec![
        serde_json::to_string_pretty(val).unwrap_or_default().into()
    ]))
}

impl PidxHandler {
    async fn handle_list(&self) -> ToolResult {
        let dir = self.store.dir();
        let mut users: Vec<serde_json::Value> = Vec::new();

        if dir.exists() {
            let entries = std::fs::read_dir(dir).map_err(tool_err)?;
            for entry in entries.flatten() {
                let path = entry.path();
                let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
                    continue;
                };
                let Some(user_id) = name.strip_suffix(".pidx.json") else {
                    continue;
                };
                if let Ok(mut profile) = self.store.load_or_create(user_id) {
                    profile.recompute_overall_confidence();
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
            a["user_id"].as_str().unwrap_or("").cmp(b["user_id"].as_str().unwrap_or(""))
        });

        info!(count = users.len(), "pidx_list");
        json_text(&serde_json::json!({ "count": users.len(), "users": users }))
    }

    async fn handle_show(&self, t: PidxShowTool) -> ToolResult {
        use pidx::output::Tier;
        let tier: Tier = t.tier.parse().map_err(tool_err)?;
        let mut profile = self.store.load_or_create(&t.user_id).map_err(tool_err)?;
        let output = render_tier_output(&mut profile, tier);
        info!(user_id = %t.user_id, tier = %t.tier, "pidx_show");
        Ok(CallToolResult::text_content(vec![output.into()]))
    }

    async fn handle_status(&self, t: PidxStatusTool) -> ToolResult {
        use pidx::models::observation::ObservationStatus;

        let mut profile = self.store.load_or_create(&t.user_id).map_err(tool_err)?;
        profile.recompute_overall_confidence();

        let mut summaries: Vec<serde_json::Value> = Vec::new();

        let scalar_fields: &[(&str, &pidx::models::observation::ObservationField)] = &[
            ("working.mode",               &profile.working.mode),
            ("working.pace",               &profile.working.pace),
            ("working.feedback",           &profile.working.feedback),
            ("working.pattern",            &profile.working.pattern),
            ("identity.reasoning.style",   &profile.identity.reasoning.style),
            ("identity.reasoning.pattern", &profile.identity.reasoning.pattern),
            ("identity.reasoning.intake",  &profile.identity.reasoning.intake),
            ("identity.reasoning.stance",  &profile.identity.reasoning.stance),
        ];

        for (path, field) in scalar_fields {
            if field.observations.is_empty() { continue; }
            let c = field.observations.iter().filter(|o| o.status == ObservationStatus::Confirmed).count();
            let p = field.observations.iter().filter(|o| o.status == ObservationStatus::Proposed).count();
            let d = field.observations.iter().filter(|o| o.status == ObservationStatus::Delta).count();
            summaries.push(serde_json::json!({ "path": path, "confirmed": c, "proposed": p, "delta": d }));
        }

        // List fields
        let list_fields: &[(&str, &[pidx::models::observation::ObservationField])] = &[
            ("identity.core",       &profile.identity.core),
            ("domains",             &profile.domains),
            ("values",              &profile.values),
            ("signals.phrases",     &profile.signals.phrases),
            ("signals.avoidances",  &profile.signals.avoidances),
            ("signals.rhythms",     &profile.signals.rhythms),
            ("signals.framings",    &profile.signals.framings),
        ];

        for (base, slice) in list_fields {
            for (i, f) in slice.iter().enumerate() {
                if f.observations.is_empty() { continue; }
                let c = f.observations.iter().filter(|o| o.status == ObservationStatus::Confirmed).count();
                let p = f.observations.iter().filter(|o| o.status == ObservationStatus::Proposed).count();
                let d = f.observations.iter().filter(|o| o.status == ObservationStatus::Delta).count();
                summaries.push(serde_json::json!({
                    "path": format!("{base}.{i}"),
                    "confirmed": c, "proposed": p, "delta": d
                }));
            }
        }

        info!(user_id = %t.user_id, "pidx_status");
        json_text(&serde_json::json!({
            "user_id": t.user_id,
            "version": profile.meta.version,
            "overall_confidence": profile.meta.overall_confidence,
            "updated": profile.meta.updated,
            "fields": summaries,
            "delta_queue_open": profile.delta_queue.iter().filter(|d| !d.resolved).count(),
            "review_queue_pending": profile.review_queue.iter().filter(|r| !r.resolved).count(),
        }))
    }

    async fn handle_ingest(&self, t: PidxIngestTool) -> ToolResult {
        use pidx::models::bridge::BridgePacket;

        let raw = std::fs::read_to_string(&t.packet_path)
            .map_err(|e| tool_err(format!("cannot read {}: {}", t.packet_path, e)))?;
        let packet: BridgePacket = serde_json::from_str(&raw)
            .map_err(|e| tool_err(format!("invalid bridge packet: {}", e)))?;

        let filename = std::path::Path::new(&t.packet_path)
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown.bridge.json");

        let mut profile = self.store.load_or_create(&t.user_id).map_err(tool_err)?;
        let (proposed, deltas) = ingest_bridge_packet(&mut profile, &packet, filename);
        run_corroboration(&mut profile);
        self.store.save(&mut profile).map_err(tool_err)?;

        info!(user_id = %t.user_id, proposed, deltas, "pidx_ingest");
        json_text(&serde_json::json!({
            "ok": true,
            "observations_proposed": proposed,
            "deltas_flagged": deltas,
        }))
    }

    async fn handle_confirm(&self, t: PidxConfirmTool) -> ToolResult {
        use pidx::models::observation::ObservationStatus;

        let mut profile = self.store.load_or_create(&t.user_id).map_err(tool_err)?;

        let idx = t.index as usize;
        let obs = resolve_obs_mut(&mut profile, &t.field, idx).map_err(tool_err)?;
        if obs.status != ObservationStatus::Proposed {
            return Err(tool_err(format!("observation is {:?}, not Proposed", obs.status)));
        }
        let val = format!("{:?}", obs.value);
        obs.status = ObservationStatus::Confirmed;

        self.store.save(&mut profile).map_err(tool_err)?;
        info!(user_id = %t.user_id, field = %t.field, index = t.index, "pidx_confirm");
        json_text(&serde_json::json!({
            "ok": true,
            "field": t.field,
            "index": t.index,
            "value": val,
        }))
    }

    async fn handle_reject(&self, t: PidxRejectTool) -> ToolResult {
        use pidx::models::observation::ObservationStatus;

        let mut profile = self.store.load_or_create(&t.user_id).map_err(tool_err)?;

        let idx = t.index as usize;
        let obs = resolve_obs_mut(&mut profile, &t.field, idx).map_err(tool_err)?;
        if obs.status != ObservationStatus::Proposed {
            return Err(tool_err(format!("observation is {:?}, not Proposed", obs.status)));
        }
        obs.status = ObservationStatus::Rejected;

        self.store.save(&mut profile).map_err(tool_err)?;
        warn!(user_id = %t.user_id, field = %t.field, index = t.index, "pidx_reject");
        json_text(&serde_json::json!({
            "ok": true,
            "field": t.field,
            "index": t.index,
        }))
    }

    async fn handle_confirm_all(&self, t: PidxConfirmAllTool) -> ToolResult {
        let mut profile = self.store.load_or_create(&t.user_id).map_err(tool_err)?;
        let confirmed = confirm_all_proposed(&mut profile, &t.field_prefix);
        let count = confirmed.len();
        if count > 0 {
            profile.recompute_overall_confidence();
            self.store.save(&mut profile).map_err(tool_err)?;
        }
        info!(user_id = %t.user_id, prefix = %t.field_prefix, count, "pidx_confirm_all");
        json_text(&serde_json::json!({
            "ok": true,
            "confirmed_count": count,
            "fields": confirmed,
        }))
    }

    async fn handle_reject_all(&self, t: PidxRejectAllTool) -> ToolResult {
        let mut profile = self.store.load_or_create(&t.user_id).map_err(tool_err)?;
        let rejected = reject_all_proposed(&mut profile, &t.field_prefix);
        let count = rejected.len();
        if count > 0 {
            self.store.save(&mut profile).map_err(tool_err)?;
        }
        info!(user_id = %t.user_id, prefix = %t.field_prefix, count, "pidx_reject_all");
        json_text(&serde_json::json!({
            "ok": true,
            "rejected_count": count,
            "fields": rejected,
        }))
    }

    async fn handle_clear(&self, t: PidxClearTool) -> ToolResult {
        let mut profile = self.store.load_or_create(&t.user_id).map_err(tool_err)?;
        let mut cleared_count = 0;

        if t.target == "deltas" || t.target == "all" {
            cleared_count += profile.delta_queue.len();
            profile.delta_queue.clear();
        }
        if t.target == "reviews" || t.target == "all" {
            cleared_count += profile.review_queue.len();
            profile.review_queue.clear();
        }
        if t.target == "proposed" || t.target == "all" {
            let matching = reject_all_proposed(&mut profile, "");
            cleared_count += matching.len();
        }

        if cleared_count > 0 {
            self.store.save(&mut profile).map_err(tool_err)?;
        }
        info!(user_id = %t.user_id, target = %t.target, cleared_count, "pidx_clear");
        json_text(&serde_json::json!({
            "ok": true,
            "target": t.target,
            "cleared_count": cleared_count,
        }))
    }

    async fn handle_delta_list(&self, t: PidxDeltaListTool) -> ToolResult {
        let profile = self.store.load_or_create(&t.user_id).map_err(tool_err)?;
        let items: Vec<serde_json::Value> = profile.delta_queue.iter()
            .filter(|d| !d.resolved)
            .map(|d| serde_json::json!({
                "id": d.id,
                "field": d.field,
                "created_at": d.created_at,
                "a": {
                    "session_ref": d.a.source.session_ref,
                    "value": format!("{:?}", d.a.value),
                    "confidence": d.a.confidence,
                },
                "b": {
                    "session_ref": d.b.source.session_ref,
                    "value": format!("{:?}", d.b.value),
                    "confidence": d.b.confidence,
                },
            }))
            .collect();
        info!(user_id = %t.user_id, count = items.len(), "pidx_delta_list");
        json_text(&serde_json::json!({ "user_id": t.user_id, "deltas": items }))
    }

    async fn handle_delta_resolve(&self, t: PidxDeltaResolveTool) -> ToolResult {
        use pidx::models::observation::ObservationStatus;

        if t.keep != "a" && t.keep != "b" {
            return Err(tool_err("keep must be \"a\" or \"b\""));
        }

        let mut profile = self.store.load_or_create(&t.user_id).map_err(tool_err)?;

        // Borrow-split: collect what we need, release immutable borrow.
        let found = profile.delta_queue.iter().find(|d| d.id == t.delta_id && !d.resolved)
            .map(|d| (
                d.field.clone(),
                d.a.source.session_ref.clone(),
                d.b.source.session_ref.clone(),
            ));

        let (field_path, session_a, session_b) = found
            .ok_or_else(|| tool_err(format!("delta '{}' not found or already resolved", t.delta_id)))?;

        let (keep_session, reject_session) = if t.keep == "a" {
            (session_a, session_b)
        } else {
            (session_b, session_a)
        };

        // Mark the delta resolved.
        if let Some(d) = profile.delta_queue.iter_mut().find(|d| d.id == t.delta_id) {
            d.resolved = true;
        }

        // Flip observation statuses in the actual field.
        if let Some(field) = resolve_field_mut(&mut profile, &field_path) {
            for obs in field.observations.iter_mut().filter(|o| o.status == ObservationStatus::Delta) {
                if obs.source.session_ref == keep_session {
                    obs.status = ObservationStatus::Confirmed;
                } else if obs.source.session_ref == reject_session {
                    obs.status = ObservationStatus::Rejected;
                }
            }
        }

        profile.recompute_overall_confidence();
        self.store.save(&mut profile).map_err(tool_err)?;
        info!(user_id = %t.user_id, delta_id = %t.delta_id, keep = %t.keep, "pidx_delta_resolve");
        json_text(&serde_json::json!({
            "ok": true,
            "delta_id": t.delta_id,
            "kept": t.keep,
            "field": field_path,
        }))
    }

    async fn handle_review_list(&self, t: PidxReviewListTool) -> ToolResult {
        let profile = self.store.load_or_create(&t.user_id).map_err(tool_err)?;
        let items: Vec<serde_json::Value> = profile.review_queue.iter()
            .filter(|r| !r.resolved)
            .map(|r| serde_json::json!({
                "id": r.id,
                "field": r.field,
                "index": r.observation_index,
                "effective_confidence": r.effective_confidence,
                "flagged_at": r.flagged_at,
            }))
            .collect();
        info!(user_id = %t.user_id, count = items.len(), "pidx_review_list");
        json_text(&serde_json::json!({ "user_id": t.user_id, "review_items": items }))
    }

    async fn handle_review_process(&self, t: PidxReviewProcessTool) -> ToolResult {
        use pidx::models::observation::ObservationStatus;

        if t.action != "solidify" && t.action != "discard" {
            return Err(tool_err("action must be \"solidify\" or \"discard\""));
        }

        let mut profile = self.store.load_or_create(&t.user_id).map_err(tool_err)?;

        // Borrow-split: collect field + index, release borrow.
        let found = profile.review_queue.iter()
            .find(|r| r.id == t.review_id && !r.resolved)
            .map(|r| (r.field.clone(), r.observation_index));

        let (field_path, obs_idx) = found
            .ok_or_else(|| tool_err(format!("review item '{}' not found or already resolved", t.review_id)))?;

        // Mark resolved.
        if let Some(r) = profile.review_queue.iter_mut().find(|r| r.id == t.review_id) {
            r.resolved = true;
        }

        // Apply action to the observation.
        if let Some(field) = resolve_field_mut(&mut profile, &field_path) {
            if let Some(obs) = field.observations.get_mut(obs_idx) {
                match t.action.as_str() {
                    "solidify" => {
                        obs.decay_exempt = true;
                        obs.status = ObservationStatus::Confirmed;
                    }
                    "discard" => {
                        obs.status = ObservationStatus::Archived;
                    }
                    _ => {}
                }
            }
        }

        self.store.save(&mut profile).map_err(tool_err)?;
        info!(user_id = %t.user_id, review_id = %t.review_id, action = %t.action, "pidx_review_process");
        json_text(&serde_json::json!({
            "ok": true,
            "review_id": t.review_id,
            "action": t.action,
            "field": field_path,
            "index": obs_idx,
        }))
    }

    async fn handle_annotate(&self, t: PidxAnnotateTool) -> ToolResult {
        use pidx::models::profile::Annotation;

        let mut profile = self.store.load_or_create(&t.user_id).map_err(tool_err)?;

        // Validate the field path exists.
        if resolve_field_mut(&mut profile, &t.field).is_none() {
            return Err(tool_err(format!("unknown field path '{}'", t.field)));
        }

        let id = Uuid::new_v4().to_string();
        profile.annotations.push(Annotation {
            id: id.clone(),
            field: t.field.clone(),
            note: t.note.clone(),
            author: "mcp".to_string(),
            created_at: chrono::Utc::now().to_rfc3339(),
            pinned: t.pinned,
        });

        self.store.save(&mut profile).map_err(tool_err)?;
        info!(user_id = %t.user_id, field = %t.field, pinned = t.pinned, "pidx_annotate");
        json_text(&serde_json::json!({
            "ok": true,
            "id": id,
            "field": t.field,
            "pinned": t.pinned,
        }))
    }

    async fn handle_diff(&self, t: PidxDiffTool) -> ToolResult {
        use pidx::models::observation::ObservationStatus;

        let mut profile_a = self.store.load_or_create(&t.user_a).map_err(tool_err)?;
        let mut profile_b = self.store.load_or_create(&t.user_b).map_err(tool_err)?;
        profile_a.recompute_overall_confidence();
        profile_b.recompute_overall_confidence();

        // Register diff
        let reg_a = &profile_a.comm;
        let reg_b = &profile_b.comm;
        let register_diff = serde_json::json!([
            { "metric": "formality",   "a": reg_a.formality.score(None),   "b": reg_b.formality.score(None) },
            { "metric": "directness",  "a": reg_a.directness.score(None),  "b": reg_b.directness.score(None) },
            { "metric": "hedging",     "a": reg_a.hedging.score(None),     "b": reg_b.hedging.score(None) },
            { "metric": "humor",       "a": reg_a.humor.score(None),       "b": reg_b.humor.score(None) },
            { "metric": "abstraction", "a": reg_a.abstraction.score(None), "b": reg_b.abstraction.score(None) },
            { "metric": "affect",      "a": reg_a.affect.score(None),      "b": reg_b.affect.score(None) },
        ]);

        // Working diff
        let working_diff = serde_json::json!({
            "mode_a":     active_text(&profile_a.working.mode),
            "mode_b":     active_text(&profile_b.working.mode),
            "pace_a":     active_text(&profile_a.working.pace),
            "pace_b":     active_text(&profile_b.working.pace),
            "feedback_a": active_text(&profile_a.working.feedback),
            "feedback_b": active_text(&profile_b.working.feedback),
        });

        // Core identity — surface confirmed text observations
        let core_a: Vec<serde_json::Value> = profile_a.identity.core.iter()
            .filter_map(|f| f.observations.iter()
                .find(|o| o.status == ObservationStatus::Confirmed)
                .map(|o| serde_json::json!(format!("{:?}", o.value))))
            .collect();
        let core_b: Vec<serde_json::Value> = profile_b.identity.core.iter()
            .filter_map(|f| f.observations.iter()
                .find(|o| o.status == ObservationStatus::Confirmed)
                .map(|o| serde_json::json!(format!("{:?}", o.value))))
            .collect();

        info!(user_a = %t.user_a, user_b = %t.user_b, "pidx_diff");
        json_text(&serde_json::json!({
            "user_a": t.user_a,
            "user_b": t.user_b,
            "overall_confidence_a": profile_a.meta.overall_confidence,
            "overall_confidence_b": profile_b.meta.overall_confidence,
            "core_a": core_a,
            "core_b": core_b,
            "register_diff": register_diff,
            "working_diff": working_diff,
        }))
    }

    async fn handle_decay(&self, t: PidxDecayTool) -> ToolResult {
        let mut profile = self.store.load_or_create(&t.user_id).map_err(tool_err)?;
        let threshold = t.threshold.unwrap_or(0.30);
        let newly_flagged = run_decay_pass(&mut profile, threshold);
        let pending = profile.review_queue.iter().filter(|r| !r.resolved).count();
        if newly_flagged > 0 {
            self.store.save(&mut profile).map_err(tool_err)?;
        }
        info!(user_id = %t.user_id, threshold, newly_flagged, "pidx_decay");
        json_text(&serde_json::json!({
            "ok": true,
            "newly_flagged": newly_flagged,
            "review_queue_pending": pending,
            "threshold": threshold,
        }))
    }
}

// ── Field resolvers (mirrors Tauri commands.rs) ───────────────────────────────

fn resolve_obs_mut<'a>(
    profile: &'a mut pidx::ProfileDocument,
    path: &str,
    index: usize,
) -> Result<&'a mut pidx::models::observation::Observation, String> {
    let field = resolve_field_mut(profile, path)
        .ok_or_else(|| format!("unknown field path '{path}'"))?;
    let len = field.observations.len();
    field
        .observations
        .get_mut(index)
        .ok_or_else(|| format!("index {index} out of range (field has {len} observations)"))
}

fn resolve_field_mut<'a>(
    profile: &'a mut pidx::ProfileDocument,
    path: &str,
) -> Option<&'a mut pidx::models::observation::ObservationField> {
    let parts: Vec<&str> = path.splitn(3, '.').collect();
    match parts.as_slice() {
        ["identity", "core", rest] => {
            let idx: usize = rest.parse().ok()?;
            profile.identity.core.get_mut(idx)
        }
        ["identity", "reasoning", name] => match *name {
            "style"   => Some(&mut profile.identity.reasoning.style),
            "pattern" => Some(&mut profile.identity.reasoning.pattern),
            "intake"  => Some(&mut profile.identity.reasoning.intake),
            "stance"  => Some(&mut profile.identity.reasoning.stance),
            _ => None,
        },
        ["domains", idx] => profile.domains.get_mut(idx.parse::<usize>().ok()?),
        ["values",  idx] => profile.values.get_mut(idx.parse::<usize>().ok()?),
        ["signals", cat, idx] => {
            let idx: usize = idx.parse().ok()?;
            match *cat {
                "phrases"    => profile.signals.phrases.get_mut(idx),
                "avoidances" => profile.signals.avoidances.get_mut(idx),
                "rhythms"    => profile.signals.rhythms.get_mut(idx),
                "framings"   => profile.signals.framings.get_mut(idx),
                _ => None,
            }
        }
        ["working", name] => match *name {
            "mode"     => Some(&mut profile.working.mode),
            "pace"     => Some(&mut profile.working.pace),
            "feedback" => Some(&mut profile.working.feedback),
            "pattern"  => Some(&mut profile.working.pattern),
            _ => None,
        },
        _ => None,
    }
}

// ── Diff helpers ──────────────────────────────────────────────────────────────

fn active_text(field: &pidx::models::observation::ObservationField) -> Option<String> {
    use pidx::models::decay::FieldClass;
    use pidx::models::observation::ObservationValue;
    field.active(FieldClass::Working).map(|v| match v {
        ObservationValue::Text(s) => s.clone(),
        ObservationValue::Domain(d) => d.label.clone(),
    })
}
