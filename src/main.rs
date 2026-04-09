#![allow(dead_code)]
mod ingestion;
mod models;
mod output;
mod storage;
mod traits;

use std::collections::HashMap;

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use serde::Serialize;
use tracing::error;

use models::bridge::BridgePacket;
use models::decay::FieldClass;
use models::observation::{ObservationField, ObservationStatus, ObservationValue};
use models::profile::{Annotation, DeltaItem, ProfileMeta, ReviewItem, ProfileWrapper};
use output::Tier;
use storage::ProfileStore;

// ── Output format ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
enum Format {
    Human,
    Json,
}

// ── JSON output structs ───────────────────────────────────────────────────────

/// Envelope for all write-command results in JSON mode.
///
/// Agents always read from stdout regardless of success or failure. On error,
/// `ok` is false and `error` carries the message; the process exits 1 either way.
#[derive(Serialize)]
struct ActionResult {
    ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    field: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    index: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    value: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    new_status: Option<String>,
    // ingest-specific
    #[serde(skip_serializing_if = "Option::is_none")]
    observations_proposed: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    overall_confidence: Option<f64>,
}

#[derive(Serialize)]
struct ConfirmAllResult {
    ok: bool,
    confirmed_count: usize,
    /// Paths that had at least one observation confirmed.
    fields: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<String>,
}

impl ActionResult {
    fn err(msg: impl ToString) -> Self {
        Self {
            ok: false,
            error: Some(msg.to_string()),
            field: None, index: None, value: None, new_status: None,
            observations_proposed: None, overall_confidence: None,
        }
    }
}

/// Per-field row in status JSON output.
#[derive(Serialize)]
struct FieldSummaryJson {
    path: String,
    confirmed: usize,
    proposed: usize,
    delta: usize,
    /// How many times this value was proposed (deduplicated across packets).
    /// Only present when > 1.
    #[serde(skip_serializing_if = "Option::is_none")]
    proposal_count: Option<u32>,
    /// First proposed/confirmed observation value, truncated to 80 chars.
    preview: Option<String>,
}

#[derive(Serialize)]
struct TotalsJson { confirmed: usize, proposed: usize, delta: usize }

#[derive(Serialize)]
struct StatusJson<'a> {
    user_id: &'a str,
    version: &'a str,
    overall_confidence: f64,
    updated: &'a str,
    fields: Vec<FieldSummaryJson>,
    totals: TotalsJson,
    delta_queue_open: usize,
    review_queue_pending: usize,
    bridge_log_processed: usize,
}

#[derive(Serialize)]
struct RegisterScoreJson { score: f64, label: &'static str }

#[derive(Serialize)]
struct DomainJson { label: String, weight_pct: u32 }

#[derive(Serialize)]
struct SideJson { orientation: String, value: String }

#[derive(Serialize)]
struct ConflictJson { id: String, field: String, a: SideJson, b: SideJson }

#[derive(Serialize)]
struct AnnotationJson { id: String, field: String, note: String, author: String, created_at: String, pinned: bool }

/// Structured profile snapshot for JSON `show` output.
///
/// Tier determines which fields are populated; empty collections are omitted
/// from the serialized JSON via `skip_serializing_if`.
#[derive(Serialize)]
struct ShowJson {
    tier: String,
    version: String,
    overall_confidence: f64,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    core: Vec<String>,
    #[serde(skip_serializing_if = "HashMap::is_empty")]
    register: HashMap<String, Option<RegisterScoreJson>>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    domains: Vec<DomainJson>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    values: Vec<String>,
    #[serde(skip_serializing_if = "HashMap::is_empty")]
    reasoning: HashMap<String, Option<String>>,
    #[serde(skip_serializing_if = "HashMap::is_empty")]
    working: HashMap<String, Option<String>>,
    // Rich-only:
    #[serde(skip_serializing_if = "HashMap::is_empty")]
    signals: HashMap<String, Vec<String>>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    annotations: Vec<AnnotationJson>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    unresolved_conflicts: Vec<ConflictJson>,
}

// ── CLI definition ────────────────────────────────────────────────────────────

/// PIDX — Personality Indexer CLI
#[derive(Parser)]
#[command(name = "pidx", version, about)]
struct Cli {
    /// Output format
    #[arg(long, global = true, value_enum, default_value_t = Format::Human)]
    format: Format,

    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Render a profile context block for LLM injection
    Show {
        user_id: String,
        /// Output resolution tier
        #[arg(long, short, value_enum, default_value_t = Tier::Standard)]
        tier: Tier,
    },

    /// Print a summary of all observations and their statuses
    Status { user_id: String },

    /// Flip a proposed observation to confirmed
    ///
    /// FIELD_PATH uses dot-notation matching the profile structure:
    ///   identity.core.0    identity.reasoning.style
    ///   domains.0          values.0
    ///   signals.phrases.0  working.mode
    Confirm {
        user_id: String,
        field: String,
        #[arg(default_value_t = 0)]
        index: usize,
    },

    /// Reject a proposed observation permanently
    Reject {
        user_id: String,
        field: String,
        #[arg(default_value_t = 0)]
        index: usize,
    },

    /// Confirm all proposed observations whose path starts with a given prefix
    ///
    /// FIELD_PREFIX is a dot-path prefix matched against field paths:
    ///   signals.phrases   → all phrase slots
    ///   domains           → all domain slots
    ///   working           → all four working fields
    ///   identity          → all identity subfields
    ConfirmAll {
        user_id: String,
        /// Dot-path prefix to match (e.g. "signals", "domains", "working")
        field: String,
    },

    /// Reject all proposed observations whose path starts with a given prefix
    RejectAll {
        user_id: String,
        /// Dot-path prefix to match (e.g. "signals", "domains", "working")
        field: String,
    },

    /// Clear specific pending queues or unconfirmed observations from the profile
    Clear {
        user_id: String,
        /// Which data to clear: 'deltas', 'reviews', 'proposed', or 'all'
        #[arg(value_parser = ["deltas", "reviews", "proposed", "all"])]
        target: String,
    },

    /// Delta queue operations
    #[command(subcommand)]
    Delta(DeltaCmd),

    /// Review queue operations
    #[command(subcommand)]
    Review(ReviewCmd),

    /// Add a permanent annotation to a field
    Annotate {
        user_id: String,
        field: String,
        note: String,
        #[arg(long)]
        pinned: bool,
    },

    /// Compare two profiles
    Diff {
        user_a: String,
        user_b: String,
    },

    /// Ingest a bridge packet (.bridge.json) into a profile
    Ingest {
        user_id: String,
        packet: std::path::PathBuf,
    },

    /// List all profiles in the profiles directory
    ListUsers,

    /// Apply time-based confidence decay and flag low-confidence observations for review
    Decay {
        user_id: String,
        /// Confidence threshold below which observations are flagged (default: 0.30)
        #[arg(long, default_value_t = 0.30)]
        threshold: f64,
    },

    /// Watch a directory for incoming .bridge.json files and auto-ingest them
    ///
    /// Files are ingested as they appear, then moved to a .processed/ sub-directory.
    /// Defaults to the platform mailbox dir (PIDX_MAILBOX_DIR to override).
    Watch {
        user_id: String,
        /// Directory to watch (default: platform mailbox dir)
        #[arg(long, short)]
        dir: Option<std::path::PathBuf>,
    },
}

#[derive(Subcommand)]
enum DeltaCmd {
    /// List open (unresolved) deltas
    List { user_id: String },

    /// Resolve a delta by keeping one side and rejecting the other
    Resolve {
        user_id: String,
        id: String,
        /// Which observation to keep: 'a' (first) or 'b' (second)
        #[arg(long, value_parser = ["a", "b"])]
        keep: String,
    },
}

#[derive(Subcommand)]
enum ReviewCmd {
    /// List pending review items
    List { user_id: String },

    /// Process a pending review item
    Process {
        user_id: String,
        id: String,
        /// Action to take: 'solidify' or 'discard'
        #[arg(long, value_parser = ["solidify", "discard"])]
        action: String,
    },
}

// ── Field path helpers ────────────────────────────────────────────────────────

fn all_fields(wrapper: &ProfileWrapper) -> Vec<(String, &ObservationField)> {
    let profile = match wrapper {
        ProfileWrapper::Human(p) => p,
        ProfileWrapper::Npc(_) => return vec![], // NPC fields could be mapped later
    };
    let mut v: Vec<(String, &ObservationField)> = Vec::new();
    for (i, f) in profile.identity.core.iter().enumerate() {
        v.push((format!("identity.core.{i}"), f));
    }
    v.push(("identity.reasoning.style".into(),   &profile.identity.reasoning.style));
    v.push(("identity.reasoning.pattern".into(), &profile.identity.reasoning.pattern));
    v.push(("identity.reasoning.intake".into(),  &profile.identity.reasoning.intake));
    v.push(("identity.reasoning.stance".into(),  &profile.identity.reasoning.stance));
    for (i, f) in profile.domains.iter().enumerate()            { v.push((format!("domains.{i}"), f)); }
    for (i, f) in profile.values.iter().enumerate()             { v.push((format!("values.{i}"), f)); }
    for (i, f) in profile.signals.phrases.iter().enumerate()    { v.push((format!("signals.phrases.{i}"), f)); }
    for (i, f) in profile.signals.avoidances.iter().enumerate() { v.push((format!("signals.avoidances.{i}"), f)); }
    for (i, f) in profile.signals.rhythms.iter().enumerate()    { v.push((format!("signals.rhythms.{i}"), f)); }
    for (i, f) in profile.signals.framings.iter().enumerate()   { v.push((format!("signals.framings.{i}"), f)); }
    v.push(("working.mode".into(),     &profile.working.mode));
    v.push(("working.pace".into(),     &profile.working.pace));
    v.push(("working.feedback".into(), &profile.working.feedback));
    v.push(("working.pattern".into(),  &profile.working.pattern));
    v
}

fn resolve_field_mut<'a>(
    wrapper: &'a mut ProfileWrapper,
    path: &str,
) -> Option<&'a mut ObservationField> {
    let profile = match wrapper {
        ProfileWrapper::Human(p) => p,
        ProfileWrapper::Npc(_) => return None,
    };
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
            _         => None,
        },
        ["domains", idx]  => profile.domains.get_mut(idx.parse::<usize>().ok()?),
        ["values",  idx]  => profile.values.get_mut(idx.parse::<usize>().ok()?),
        ["signals", cat, idx] => {
            let idx: usize = idx.parse().ok()?;
            match *cat {
                "phrases"    => profile.signals.phrases.get_mut(idx),
                "avoidances" => profile.signals.avoidances.get_mut(idx),
                "rhythms"    => profile.signals.rhythms.get_mut(idx),
                "framings"   => profile.signals.framings.get_mut(idx),
                _            => None,
            }
        }
        ["working", name] => match *name {
            "mode"     => Some(&mut profile.working.mode),
            "pace"     => Some(&mut profile.working.pace),
            "feedback" => Some(&mut profile.working.feedback),
            "pattern"  => Some(&mut profile.working.pattern),
            _          => None,
        },
        _ => None,
    }
}

/// Short display string for any ObservationValue.
fn val_str(v: &ObservationValue) -> String {
    match v {
        ObservationValue::Text(s)   => s.clone(),
        ObservationValue::Number(n) => n.to_string(),
        ObservationValue::Domain(d) => format!("{} ({:.0}%)", d.label, d.weight * 100.0),
    }
}

/// Active text value for a field (the confirmed obs with highest confidence), or None.
fn active_text(field: &ObservationField, fc: FieldClass) -> Option<String> {
    field.active(fc).map(|v| match v {
        ObservationValue::Text(s)   => s.clone(),
        ObservationValue::Number(n) => n.to_string(),
        ObservationValue::Domain(d) => d.label.clone(),
    })
}

/// Truncate a string to max_len, appending "…" if truncated.
fn truncate(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        format!("{}…", &s[..max_len])
    }
}

// ── Command handlers ──────────────────────────────────────────────────────────

fn cmd_show(user_id: &str, tier: Tier, format: Format) -> Result<()> {
    let store = ProfileStore::new(ProfileStore::default_dir());
    let mut profile = store.load_or_create(user_id)?;

    if format == Format::Json {
        let json = build_show_json(&mut profile, tier);
        println!("{}", serde_json::to_string_pretty(&json)?);
        return Ok(());
    }

    let out = output::render_tier_output(&mut profile, tier);
    if out.is_empty() {
        eprintln!("No confirmed observations for '{user_id}' at tier {tier}.");
        eprintln!("Run `pidx status {user_id}` to see pending observations.");
    } else {
        println!("{out}");
    }
    Ok(())
}

fn build_show_json(wrapper: &mut ProfileWrapper, tier: Tier) -> ShowJson {
    let profile = match wrapper {
        ProfileWrapper::Human(p) => p,
        ProfileWrapper::Npc(p) => return build_npc_show_json(p, tier),
    };
    use chrono::Utc;

    profile.recompute_overall_confidence();
    let now = Utc::now();

    // ── core (all tiers) ─────────────────────────────────────────────────
    let core: Vec<String> = profile.identity.core.iter()
        .filter_map(|f| active_text(f, FieldClass::Identity))
        .take(3)
        .collect();

    let mut register = HashMap::new();
    let mut working  = HashMap::new();
    let mut domains  = Vec::new();
    let mut values   = Vec::new();
    let mut reasoning = HashMap::new();
    let mut signals: HashMap<String, Vec<String>> = HashMap::new();
    let mut annotations: Vec<AnnotationJson> = Vec::new();
    let mut unresolved_conflicts = Vec::new();

    if tier != Tier::Nano {
        // ── register (micro+) ────────────────────────────────────────────
        let reg = &profile.comm;
        for (name, metric) in [
            ("formality",   &reg.formality),
            ("directness",  &reg.directness),
            ("hedging",     &reg.hedging),
            ("humor",       &reg.humor),
            ("abstraction", &reg.abstraction),
            ("affect",      &reg.affect),
        ] {
            let entry = if metric.evidence.is_empty() {
                None
            } else {
                Some(RegisterScoreJson {
                    score: metric.score(Some(now)),
                    label: metric.score_label(Some(now)),
                })
            };
            register.insert(name.to_string(), entry);
        }

        // ── working micro subset ─────────────────────────────────────────
        for name in ["mode", "feedback"] {
            let field = match name {
                "mode"     => &profile.working.mode,
                "feedback" => &profile.working.feedback,
                _          => unreachable!(),
            };
            working.insert(name.to_string(), active_text(field, FieldClass::Working));
        }
    }

    if tier == Tier::Standard || tier == Tier::Rich {
        // ── domains, values, reasoning (standard+) ───────────────────────
        domains = profile.domains.iter()
            .filter_map(|f| f.active(FieldClass::Domain))
            .map(|v| match v {
                ObservationValue::Domain(d) => DomainJson {
                    label: d.label.clone(),
                    weight_pct: (d.weight * 100.0).round() as u32,
                },
                ObservationValue::Number(n) => DomainJson { label: n.to_string(), weight_pct: 0 },
                ObservationValue::Text(s) => DomainJson { label: s.clone(), weight_pct: 0 },
            })
            .collect();

        values = profile.values.iter()
            .filter_map(|f| active_text(f, FieldClass::Value))
            .collect();

        let r = &profile.identity.reasoning;
        for (name, field) in [
            ("style",   &r.style),
            ("pattern", &r.pattern),
            ("intake",  &r.intake),
            ("stance",  &r.stance),
        ] {
            reasoning.insert(name.to_string(), active_text(field, FieldClass::Identity));
        }

        // ── full working (replaces micro subset) ─────────────────────────
        working.clear();
        for name in ["mode", "pace", "feedback", "pattern"] {
            let field = match name {
                "mode"     => &profile.working.mode,
                "pace"     => &profile.working.pace,
                "feedback" => &profile.working.feedback,
                "pattern"  => &profile.working.pattern,
                _          => unreachable!(),
            };
            working.insert(name.to_string(), active_text(field, FieldClass::Working));
        }
    }

    if tier == Tier::Rich {
        // ── signals (rich only) ──────────────────────────────────────────
        let s = &profile.signals;
        for (cat, fields) in [
            ("phrases",    s.phrases.as_slice()),
            ("avoidances", s.avoidances.as_slice()),
            ("rhythms",    s.rhythms.as_slice()),
            ("framings",   s.framings.as_slice()),
        ] {
            let items: Vec<String> = fields.iter()
                .filter_map(|f| active_text(f, FieldClass::Signal))
                .collect();
            if !items.is_empty() {
                signals.insert(cat.to_string(), items);
            }
        }

        // ── annotations (rich only) ──────────────────────────────────────
        annotations = profile.annotations.iter()
            .filter(|a| a.pinned)
            .map(|a| AnnotationJson {
                id: a.id.clone(),
                field: a.field.clone(),
                note: a.note.clone(),
                author: a.author.clone(),
                created_at: a.created_at.clone(),
                pinned: a.pinned,
            })
            .collect();

        // ── unresolved conflicts (rich only) ─────────────────────────────
        unresolved_conflicts = profile.delta_queue.iter()
            .filter(|d| !d.resolved)
            .map(|d| ConflictJson {
                id:    d.id.clone(),
                field: d.field.clone(),
                a: SideJson { orientation: d.a.source.orientation.clone(), value: val_str(&d.a.value) },
                b: SideJson { orientation: d.b.source.orientation.clone(), value: val_str(&d.b.value) },
            })
            .collect();
    }

    ShowJson {
        tier: tier.to_string(),
        version: profile.meta.version.clone(),
        overall_confidence: profile.meta.overall_confidence,
        core,
        register,
        domains,
        values,
        reasoning,
        working,
        signals,
        annotations,
        unresolved_conflicts,
    }
}

fn cmd_status(user_id: &str, format: Format) -> Result<()> {
    let store = ProfileStore::new(ProfileStore::default_dir());
    let mut profile = store.load_or_create(user_id)?;
    profile.recompute_overall_confidence();

    if format == Format::Json {
        let mut field_summaries: Vec<FieldSummaryJson> = Vec::new();
        let mut total_confirmed = 0usize;
        let mut total_proposed  = 0usize;
        let mut total_delta     = 0usize;

        for (path, field) in all_fields(&profile) {
            if field.observations.is_empty() { continue; }
            let mut c = 0; let mut p = 0; let mut d = 0;
            for obs in &field.observations {
                match obs.status {
                    ObservationStatus::Confirmed => c += 1,
                    ObservationStatus::Proposed  => p += 1,
                    ObservationStatus::Delta     => d += 1,
                    _                            => {}
                }
            }
            total_confirmed += c;
            total_proposed  += p;
            total_delta     += d;

            let preview = field.observations.iter()
                .find(|o| matches!(o.status, ObservationStatus::Proposed | ObservationStatus::Confirmed))
                .map(|o| truncate(&val_str(&o.value), 80));

            let pc = if field.proposal_count > 1 { Some(field.proposal_count) } else { None };
            field_summaries.push(FieldSummaryJson { path, confirmed: c, proposed: p, delta: d, proposal_count: pc, preview });
        }

        let out = StatusJson {
            user_id,
            version: &profile.meta().version,
            overall_confidence: profile.meta().overall_confidence,
            updated: &profile.meta().updated,
            fields: field_summaries,
            totals: TotalsJson { confirmed: total_confirmed, proposed: total_proposed, delta: total_delta },
            delta_queue_open:       profile.delta_queue().iter().filter(|d| !d.resolved).count(),
            review_queue_pending:   profile.review_queue().iter().filter(|r| !r.resolved).count(),
            bridge_log_processed:   profile.bridge_log().processed.len(),
        };
        println!("{}", serde_json::to_string_pretty(&out)?);
        return Ok(());
    }

    // ── human mode ───────────────────────────────────────────────────────────
    eprintln!(
        "{user_id}  v{}  conf:{:.2}  updated: {}",
        profile.meta().version, profile.meta().overall_confidence, profile.meta().updated,
    );
    eprintln!();

    let mut total_confirmed = 0usize;
    let mut total_proposed  = 0usize;
    let mut total_delta     = 0usize;
    let mut printed_any     = false;

    let summaries: Vec<(String, Vec<String>)> = all_fields(&profile)
        .into_iter()
        .filter(|(_, f)| !f.observations.is_empty())
        .map(|(path, field)| {
            let mut tags = Vec::new();
            let mut c = 0; let mut p = 0; let mut d = 0;
            for obs in &field.observations {
                match obs.status {
                    ObservationStatus::Confirmed => c += 1,
                    ObservationStatus::Proposed  => p += 1,
                    ObservationStatus::Delta     => d += 1,
                    _                            => {}
                }
            }
            total_confirmed += c;
            total_proposed  += p;
            total_delta     += d;
            if c > 0 { tags.push(format!("confirmed:{c}")); }
            if p > 0 { tags.push(format!("proposed:{p}")); }
            if d > 0 { tags.push(format!("delta:{d}")); }

            let preview = field.observations.iter()
                .find(|o| matches!(o.status, ObservationStatus::Proposed | ObservationStatus::Confirmed))
                .map(|o| {
                    let s = val_str(&o.value);
                    let count_tag = if field.proposal_count > 1 {
                        format!(" (×{})", field.proposal_count)
                    } else {
                        String::new()
                    };
                    format!("\"{}\"{}",  truncate(&s, 55), count_tag)
                });

            let mut lines = vec![format!("  {path:<38} {}", tags.join("  "))];
            if let Some(p) = preview {
                lines.push(format!("  {:<38} {}", "", p));
            }
            (path, lines)
        })
        .collect();

    for (_, lines) in &summaries {
        for line in lines { eprintln!("{line}"); }
        printed_any = true;
    }

    if !printed_any {
        eprintln!("  (no observations yet)");
    } else {
        eprintln!();
        eprintln!("  total: {total_confirmed} confirmed, {total_proposed} proposed, {total_delta} delta");
    }
    eprintln!();
    let open_deltas = profile.delta_queue().iter().filter(|d| !d.resolved).count();
    let open_review = profile.review_queue().iter().filter(|r| !r.resolved).count();
    let bridge_done = profile.bridge_log().processed.len();
    eprintln!(
        "  delta_queue: {open_deltas} open  |  review_queue: {open_review} pending  |  bridge_log: {bridge_done} processed"
    );
    Ok(())
}

/// Shared logic for confirm and reject.
fn cmd_flip_status(
    user_id: &str,
    field: &str,
    index: usize,
    new_status: ObservationStatus,
    from_status: ObservationStatus,
    format: Format,
) -> Result<()> {
    let store = ProfileStore::new(ProfileStore::default_dir());
    let mut profile = store.load_or_create(user_id)?;

    let preview = {
        let obs_field = resolve_field_mut(&mut profile, field)
            .ok_or_else(|| anyhow::anyhow!("unknown field path '{field}'"))?;

        let len = obs_field.observations.len();
        let obs = obs_field.observations.get_mut(index)
            .ok_or_else(|| anyhow::anyhow!("index {index} out of range (field has {len} obs)"))?;

        if obs.status != from_status {
            anyhow::bail!(
                "observation is '{:?}', not '{:?}' — can only {} observations in '{:?}' status",
                obs.status, from_status,
                if new_status == ObservationStatus::Confirmed { "confirm" } else { "reject" },
                from_status,
            );
        }
        obs.status = new_status;
        val_str(&obs.value)
    };

    store.save(&mut profile)?;

    let verb      = if new_status == ObservationStatus::Confirmed { "confirmed" } else { "rejected" };
    let preview_s = truncate(&preview, 60);

    if format == Format::Json {
        println!("{}", serde_json::to_string_pretty(&ActionResult {
            ok: true,
            error: None,
            field: Some(field.to_string()),
            index: Some(index),
            value: Some(preview_s),
            new_status: Some(verb.to_string()),
            observations_proposed: None,
            overall_confidence: None,
        })?);
    } else {
        eprintln!("{verb} {field}[{index}]: \"{preview_s}\"");
    }
    Ok(())
}

fn cmd_confirm(user_id: &str, field: &str, index: usize, format: Format) -> Result<()> {
    cmd_flip_status(user_id, field, index, ObservationStatus::Confirmed, ObservationStatus::Proposed, format)
}

fn cmd_reject(user_id: &str, field: &str, index: usize, format: Format) -> Result<()> {
    cmd_flip_status(user_id, field, index, ObservationStatus::Rejected, ObservationStatus::Proposed, format)
}

fn cmd_confirm_all(user_id: &str, field_prefix: &str, format: Format) -> Result<()> {
    let store = ProfileStore::new(ProfileStore::default_dir());
    let mut profile = store.load_or_create(user_id)?;

    let confirmed_fields = ingestion::confirm_all_proposed(&mut profile, field_prefix);
    let confirmed: usize = confirmed_fields.len();

    if confirmed > 0 {
        profile.recompute_overall_confidence();
        store.save(&mut profile)?;
    }

    let result = ConfirmAllResult { ok: true, confirmed_count: confirmed, fields: confirmed_fields, error: None };

    if format == Format::Json {
        println!("{}", serde_json::to_string_pretty(&result)?);
    } else if confirmed == 0 {
        eprintln!("No proposed observations found under '{field_prefix}'.");
    } else {
        eprintln!("Confirmed {confirmed} observation(s) across {confirmed} field(s).");
        for f in &result.fields { eprintln!("  {f}"); }
    }
    Ok(())
}

fn cmd_reject_all(user_id: &str, field_prefix: &str, format: Format) -> Result<()> {
    let store = ProfileStore::new(ProfileStore::default_dir());
    let mut profile = store.load_or_create(user_id)?;

    let rejected_fields = ingestion::reject_all_proposed(&mut profile, field_prefix);
    let rejected: usize = rejected_fields.len();

    if rejected > 0 {
        store.save(&mut profile)?;
    }

    let result = ConfirmAllResult { ok: true, confirmed_count: rejected, fields: rejected_fields, error: None };

    if format == Format::Json {
        println!("{}", serde_json::to_string_pretty(&result)?);
    } else if rejected == 0 {
        eprintln!("No proposed observations found under '{field_prefix}'.");
    } else {
        eprintln!("Rejected {rejected} observation(s) across {rejected} field(s).");
        for f in &result.fields { eprintln!("  {f}"); }
    }
    Ok(())
}

fn cmd_clear(user_id: &str, target: &str, format: Format) -> Result<()> {
    let store = ProfileStore::new(ProfileStore::default_dir());
    let mut profile = store.load_or_create(user_id)?;

    let mut cleared_count = 0;

    if target == "deltas" || target == "all" {
        cleared_count += profile.delta_queue().len();
        profile.delta_queue_mut().clear();
    }
    if target == "reviews" || target == "all" {
        cleared_count += profile.review_queue().len();
        profile.review_queue_mut().clear();
    }
    if target == "proposed" || target == "all" {
        let matching = ingestion::reject_all_proposed(&mut profile, "");
        cleared_count += matching.len();
    }

    if cleared_count > 0 {
        store.save(&mut profile)?;
    }

    if format == Format::Json {
        println!("{}", serde_json::to_string_pretty(&serde_json::json!({
            "ok": true,
            "target": target,
            "cleared_count": cleared_count,
        }))?);
    } else {
        eprintln!("Cleared {} items for target '{}'.", cleared_count, target);
    }
    Ok(())
}

fn cmd_delta_list(user_id: &str, format: Format) -> Result<()> {
    let store = ProfileStore::new(ProfileStore::default_dir());
    let profile = store.load_or_create(user_id)?;

    let open: Vec<&DeltaItem> = profile.delta_queue().iter().filter(|d| !d.resolved).collect();

    if format == Format::Json {
        let json_deltas: Vec<ConflictJson> = open.iter().map(|d| ConflictJson {
            id:    d.id.clone(),
            field: d.field.clone(),
            a: SideJson { orientation: d.a.source.orientation.clone(), value: val_str(&d.a.value) },
            b: SideJson { orientation: d.b.source.orientation.clone(), value: val_str(&d.b.value) },
        }).collect();
        println!("{}", serde_json::to_string_pretty(&serde_json::json!({
            "user_id": user_id,
            "open_count": open.len(),
            "deltas": json_deltas,
        }))?);
        return Ok(());
    }

    if open.is_empty() {
        eprintln!("No open deltas for '{user_id}'.");
        return Ok(());
    }
    eprintln!("{} open delta(s) for '{user_id}':\n", open.len());
    for d in &open {
        eprintln!("  id:    {}", d.id);
        eprintln!("  field: {}", d.field);
        eprintln!("  a:     [{}] \"{}\"", d.a.source.orientation, val_str(&d.a.value));
        eprintln!("  b:     [{}] \"{}\"", d.b.source.orientation, val_str(&d.b.value));
        eprintln!("  since: {}", d.created_at);
        eprintln!();
    }
    eprintln!("Resolve with: pidx delta resolve {user_id} <id> --keep a|b");
    Ok(())
}

fn cmd_delta_resolve(user_id: &str, delta_id: &str, keep: &str, format: Format) -> Result<()> {
    let store = ProfileStore::new(ProfileStore::default_dir());
    let mut profile = store.load_or_create(user_id)?;

    let (field_path, keep_session, reject_session) = {
        let d = profile.delta_queue().iter()
            .find(|d| d.id == delta_id && !d.resolved)
            .ok_or_else(|| anyhow::anyhow!("no open delta with id '{delta_id}'"))?;
        let (keep_obs, reject_obs) = if keep == "a" { (&d.a, &d.b) } else { (&d.b, &d.a) };
        (d.field.clone(), keep_obs.source.session_ref.clone(), reject_obs.source.session_ref.clone())
    };

    for d in profile.delta_queue_mut().iter_mut() {
        if d.id == delta_id { d.resolved = true; break; }
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

    store.save(&mut profile)?;

    if format == Format::Json {
        println!("{}", serde_json::to_string_pretty(&serde_json::json!({
            "ok": true,
            "delta_id": delta_id,
            "kept": keep,
            "field": field_path,
        }))?);
    } else {
        eprintln!("Resolved delta {delta_id}: kept '{keep}', rejected the other.");
    }
    Ok(())
}

fn cmd_ingest(user_id: &str, packet_path: &std::path::Path, format: Format) -> Result<()> {
    use crate::models::bridge::BridgePacket;

    let store = ProfileStore::new(ProfileStore::default_dir());
    let mut profile = store.load_or_create(user_id)?;

    let raw = std::fs::read_to_string(packet_path)
        .map_err(|e| anyhow::anyhow!("cannot read {}: {e}", packet_path.display()))?;
    let packet: BridgePacket = serde_json::from_str(&raw)
        .map_err(|e| anyhow::anyhow!("invalid bridge packet: {e}"))?;

    // Timestamp validation
    if chrono::DateTime::parse_from_rfc3339(&packet.timestamp).is_err() 
       && chrono::DateTime::parse_from_rfc2822(&packet.timestamp).is_err() {
        anyhow::bail!("invalid bridge packet: invalid timestamp '{}'", packet.timestamp);
    }

    let n_obs   = packet.observations.len();
    let session = packet.session_ref.clone();
    let filename = packet_path.file_name().and_then(|n| n.to_str()).unwrap_or("unknown.bridge.json");

    ingestion::ingest_bridge_packet(&mut profile, &packet, filename);
    store.save(&mut profile)?;

    if format == Format::Json {
        println!("{}", serde_json::to_string_pretty(&ActionResult {
            ok: true,
            error: None,
            field: None,
            index: None,
            value: None,
            new_status: None,
            observations_proposed: Some(n_obs),
            overall_confidence: Some(profile.meta().overall_confidence),
        })?);
    } else {
        eprintln!("Ingested {n_obs} observations from session '{session}' into '{user_id}'.");
        eprintln!("Saved. Run `pidx status {user_id}` to review proposed observations.");
    }
    Ok(())
}

fn cmd_annotate(user_id: &str, field: &str, note: &str, pinned: bool, format: Format) -> Result<()> {
    let store = ProfileStore::new(ProfileStore::default_dir());
    let mut profile = store.load_or_create(user_id)?;

    if resolve_field_mut(&mut profile, field).is_none() {
        anyhow::bail!("unknown field path '{field}'");
    }

    let id = uuid::Uuid::new_v4().to_string();
    let annotation = Annotation {
        id: id.clone(),
        field: field.to_string(),
        note: note.to_string(),
        author: "user".to_string(),
        created_at: ProfileMeta::now_utc(),
        pinned,
    };

    profile.annotations_mut().push(annotation.clone());
    store.save(&mut profile)?;

    if format == Format::Json {
        println!("{}", serde_json::to_string_pretty(&serde_json::json!({
            "ok": true,
            "annotation": {
                "id": annotation.id,
                "field": annotation.field,
                "note": annotation.note,
                "pinned": annotation.pinned,
            }
        }))?);
    } else {
        eprintln!("Added annotation to {field}: \"{}\"", truncate(note, 60));
    }
    Ok(())
}

fn cmd_review_list(user_id: &str, format: Format) -> Result<()> {
    let store = ProfileStore::new(ProfileStore::default_dir());
    let profile = store.load_or_create(user_id)?;

    let pending: Vec<&ReviewItem> = profile.review_queue().iter().filter(|r| !r.resolved).collect();

    if format == Format::Json {
        let json_items: Vec<serde_json::Value> = pending.iter().map(|r| serde_json::json!({
            "id": r.id,
            "field": r.field,
            "index": r.observation_index,
            "confidence": r.effective_confidence,
            "flagged_at": r.flagged_at,
        })).collect();
        println!("{}", serde_json::to_string_pretty(&serde_json::json!({
            "user_id": user_id,
            "pending_count": pending.len(),
            "items": json_items,
        }))?);
        return Ok(());
    }

    if pending.is_empty() {
        eprintln!("No pending review items for '{user_id}'.");
        return Ok(());
    }
    eprintln!("{} pending review item(s) for '{user_id}':\n", pending.len());
    for r in &pending {
        eprintln!("  id:         {}", r.id);
        eprintln!("  field:      {}[{}]", r.field, r.observation_index);
        eprintln!("  confidence: {:.2}", r.effective_confidence);
        eprintln!("  flagged at: {}", r.flagged_at);
        eprintln!();
    }
    eprintln!("Process with: pidx review process {user_id} <id> --action solidify|discard");
    Ok(())
}

fn cmd_review_process(user_id: &str, review_id: &str, action: &str, format: Format) -> Result<()> {
    let store = ProfileStore::new(ProfileStore::default_dir());
    let mut profile = store.load_or_create(user_id)?;

    let (field_path, obs_idx) = {
        let r = profile.review_queue_mut().iter_mut()
            .find(|r| r.id == review_id && !r.resolved)
            .ok_or_else(|| anyhow::anyhow!("no pending review item with id '{review_id}'"))?;
        r.resolved = true;
        (r.field.clone(), r.observation_index)
    };

    let preview = if let Some(field) = resolve_field_mut(&mut profile, &field_path) {
        if let Some(obs) = field.observations.get_mut(obs_idx) {
            if action == "solidify" {
                obs.decay_exempt = true;
                obs.weight = 1.0;
                obs.status = ObservationStatus::Confirmed;
            } else if action == "discard" {
                obs.status = ObservationStatus::Archived;
            }
            Some(val_str(&obs.value))
        } else {
            None
        }
    } else {
        None
    };

    store.save(&mut profile)?;

    if format == Format::Json {
        println!("{}", serde_json::to_string_pretty(&serde_json::json!({
            "ok": true,
            "review_id": review_id,
            "action": action,
            "field": field_path,
        }))?);
    } else {
        eprintln!("Processed review {review_id}: '{action}' applied to {field_path}[{obs_idx}]");
        if let Some(p) = preview {
            eprintln!("  Value: \"{}\"", truncate(&p, 60));
        }
    }
    Ok(())
}

fn cmd_diff(user_a: &str, user_b: &str, format: Format) -> Result<()> {
    let store = ProfileStore::new(ProfileStore::default_dir());
    let mut pa = match store.load_or_create(user_a)? {
        ProfileWrapper::Human(h) => h,
        ProfileWrapper::Npc(_) => anyhow::bail!("Diff not supported for NPCs"),
    };
    let mut pb = match store.load_or_create(user_b)? {
        ProfileWrapper::Human(h) => h,
        ProfileWrapper::Npc(_) => anyhow::bail!("Diff not supported for NPCs"),
    };

    pa.recompute_overall_confidence();
    pb.recompute_overall_confidence();

    let mut diffs: Vec<serde_json::Value> = Vec::new();
    
    let core_a: Vec<String> = pa.identity.core.iter().filter_map(|f| active_text(f, FieldClass::Identity)).collect();
    let core_b: Vec<String> = pb.identity.core.iter().filter_map(|f| active_text(f, FieldClass::Identity)).collect();
    diffs.push(serde_json::json!({ "field": "identity.core", "a": core_a, "b": core_b }));

    let mode_a = active_text(&pa.working.mode, FieldClass::Working);
    let mode_b = active_text(&pb.working.mode, FieldClass::Working);
    if mode_a != mode_b {
        diffs.push(serde_json::json!({ "field": "working.mode", "a": mode_a, "b": mode_b }));
    }

    let val_a: Vec<String> = pa.values.iter().filter_map(|f| active_text(f, FieldClass::Value)).collect();
    let val_b: Vec<String> = pb.values.iter().filter_map(|f| active_text(f, FieldClass::Value)).collect();
    diffs.push(serde_json::json!({ "field": "values", "a": val_a, "b": val_b }));

    let mut reg_diffs = Vec::new();
    let now = chrono::Utc::now();
    for (name, a_metric, b_metric) in [
        ("formality", &pa.comm.formality, &pb.comm.formality),
        ("directness", &pa.comm.directness, &pb.comm.directness),
        ("hedging", &pa.comm.hedging, &pb.comm.hedging),
        ("humor", &pa.comm.humor, &pb.comm.humor),
        ("abstraction", &pa.comm.abstraction, &pb.comm.abstraction),
    ] {
        let score_a = if a_metric.evidence.is_empty() { 0.0 } else { a_metric.score(Some(now)) };
        let score_b = if b_metric.evidence.is_empty() { 0.0 } else { b_metric.score(Some(now)) };
        let diff = score_a - score_b;
        if diff.abs() > 0.1 {
            reg_diffs.push(serde_json::json!({ "metric": name, "a": score_a, "b": score_b, "diff": diff }));
        }
    }
    
    if !reg_diffs.is_empty() {
        diffs.push(serde_json::json!({ "field": "register", "differences": reg_diffs }));
    }

    if format == Format::Json {
        println!("{}", serde_json::to_string_pretty(&serde_json::json!({
            "user_a": user_a,
            "user_b": user_b,
            "confidence_a": pa.meta.overall_confidence,
            "confidence_b": pb.meta.overall_confidence,
            "diffs": diffs,
        }))?);
        return Ok(());
    }

    eprintln!("Diff between {} and {}", user_a, user_b);
    eprintln!("  Confidence: {:.2} vs {:.2}", pa.meta.overall_confidence, pb.meta.overall_confidence);
    eprintln!("  Identity Core:");
    eprintln!("    A: {:?}", core_a);
    eprintln!("    B: {:?}", core_b);
    if mode_a != mode_b {
        eprintln!("  Working Mode:");
        eprintln!("    A: {:?}", mode_a.unwrap_or_default());
        eprintln!("    B: {:?}", mode_b.unwrap_or_default());
    }
    if !reg_diffs.is_empty() {
        eprintln!("  Register Differences:");
        for rd in reg_diffs {
            let m = rd["metric"].as_str().unwrap();
            let a = rd["a"].as_f64().unwrap();
            let b = rd["b"].as_f64().unwrap();
            let d = rd["diff"].as_f64().unwrap();
            eprintln!("    {m}: A={:.2}, B={:.2} (diff: {:+.2})", a, b, d);
        }
    }
    
    Ok(())
}

fn cmd_list_users(format: Format) -> Result<()> {
    use std::fs;
    let dir = ProfileStore::default_dir();

    // Collect (user_id, profile_meta) for every *.pidx.json file in the dir.
    // Missing or unreadable files are silently skipped — best-effort listing.
    #[derive(Serialize)]
    struct UserSummary {
        user_id: String,
        version: String,
        updated: String,
        overall_confidence: f64,
    }

    let mut users: Vec<UserSummary> = Vec::new();

    if dir.exists() {
        for entry in fs::read_dir(&dir)
            .with_context(|| format!("reading profiles dir {}", dir.display()))?
        {
            let entry = entry?;
            let path = entry.path();
            let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
                continue;
            };
            let Some(user_id) = name.strip_suffix(".pidx.json") else {
                continue;
            };
            // Load just enough to surface meta — ignore parse errors silently.
            if let Ok(Some(profile)) = {
                let store = ProfileStore::new(&dir);
                store.load(user_id)
            } {
                users.push(UserSummary {
                    user_id: user_id.to_string(),
                    version: profile.meta().version.clone(),
                    updated: profile.meta().updated.clone(),
                    overall_confidence: profile.meta().overall_confidence,
                });
            }
        }
    }

    users.sort_by(|a, b| a.user_id.cmp(&b.user_id));

    if format == Format::Json {
        println!("{}", serde_json::to_string_pretty(&serde_json::json!({
            "count": users.len(),
            "users": users,
        }))?);
    } else {
        if users.is_empty() {
            eprintln!("No profiles found in {}.", dir.display());
        } else {
            eprintln!("{} profile(s) in {}:\n", users.len(), dir.display());
            for u in &users {
                eprintln!(
                    "  {:<32}  v{}  conf:{:.2}  updated: {}",
                    u.user_id, u.version, u.overall_confidence, u.updated,
                );
            }
        }
    }
    Ok(())
}

fn cmd_decay(user_id: &str, threshold: f64, format: Format) -> Result<()> {
    let store = ProfileStore::new(ProfileStore::default_dir());
    let mut profile = store.load_or_create(user_id)?;

    let newly_flagged = ingestion::run_decay_pass(&mut profile, threshold);

    if newly_flagged > 0 {
        profile.recompute_overall_confidence();
        store.save(&mut profile)?;
    }

    let pending = profile.review_queue().iter().filter(|r| !r.resolved).count();

    if format == Format::Json {
        println!("{}", serde_json::to_string_pretty(&serde_json::json!({
            "ok": true,
            "threshold": threshold,
            "newly_flagged": newly_flagged,
            "review_queue_pending": pending,
        }))?);
    } else if newly_flagged == 0 {
        eprintln!("No observations below threshold {threshold:.2} for '{user_id}'.");
    } else {
        eprintln!(
            "Flagged {newly_flagged} observation(s) → review queue ({pending} pending total)."
        );
        eprintln!("Review with: pidx review list {user_id}");
    }
    Ok(())
}

fn cmd_watch(user_id: &str, dir: Option<std::path::PathBuf>, format: Format) -> Result<()> {
    use notify::{EventKind, RecursiveMode, Watcher};
    use notify::event::ModifyKind;
    use std::sync::mpsc;

    let mailbox = dir.unwrap_or_else(ProfileStore::default_mailbox_dir);
    std::fs::create_dir_all(&mailbox)
        .with_context(|| format!("creating mailbox dir {}", mailbox.display()))?;

    let processed_dir = mailbox.join(".processed");
    std::fs::create_dir_all(&processed_dir)
        .with_context(|| format!("creating .processed dir {}", processed_dir.display()))?;

    let (tx, rx) = mpsc::channel();
    let mut watcher = notify::recommended_watcher(move |res| {
        let _ = tx.send(res);
    })?;
    watcher.watch(&mailbox, RecursiveMode::NonRecursive)?;

    eprintln!("watching {}  →  {user_id}  (ctrl-c to stop)", mailbox.display());

    for result in rx {
        let event = match result {
            Ok(e) => e,
            Err(e) => { eprintln!("watch error: {e}"); continue; }
        };

        // Only act on file creation or data-write events.
        let relevant = matches!(
            event.kind,
            EventKind::Create(_) | EventKind::Modify(ModifyKind::Data(_))
        );
        if !relevant { continue; }

        for path in &event.paths {
            let name = match path.file_name().and_then(|n| n.to_str()) {
                Some(n) => n,
                None => continue,
            };
            // Only process .bridge.json files; skip hidden/temp files.
            if !name.ends_with(".bridge.json") || name.starts_with('.') { continue; }

            // Try to read + parse; skip silently if file isn't fully written yet.
            let content = match std::fs::read_to_string(path) {
                Ok(c) => c,
                Err(_) => continue,
            };
            let packet: BridgePacket = match serde_json::from_str(&content) {
                Ok(p) => p,
                Err(e) => {
                    eprintln!("  skip {name}: parse error: {e}");
                    continue;
                }
            };

            // Timestamp validation
            if chrono::DateTime::parse_from_rfc3339(&packet.timestamp).is_err() 
               && chrono::DateTime::parse_from_rfc2822(&packet.timestamp).is_err() {
                eprintln!("  skip {name}: invalid timestamp '{}'", packet.timestamp);
                continue;
            }

            let store = ProfileStore::new(ProfileStore::default_dir());
            let mut profile = store.load_or_create(user_id)?;
            let (proposed, deltas) = ingestion::ingest_bridge_packet(&mut profile, &packet, name);
            store.save(&mut profile)?;

            // Move to .processed/ — if rename fails (e.g. cross-device), delete instead.
            let dest = processed_dir.join(name);
            if std::fs::rename(path, &dest).is_err() {
                let _ = std::fs::remove_file(path);
            }

            if format == Format::Json {
                println!("{}", serde_json::to_string_pretty(&serde_json::json!({
                    "ok": true,
                    "file": name,
                    "observations_proposed": proposed,
                    "deltas_flagged": deltas,
                    "overall_confidence": profile.meta().overall_confidence,
                }))?);
            } else {
                eprintln!(
                    "  [{}] proposed:{proposed}  deltas:{deltas}  conf:{:.2}",
                    name, profile.meta().overall_confidence,
                );
            }
        }
    }
    Ok(())
}

// ── Entry point ───────────────────────────────────────────────────────────────

fn main() {
    // Initialise tracing. Reads RUST_LOG (e.g. `RUST_LOG=info pidx status usr`).
    // Defaults to showing warnings and above when RUST_LOG is unset.
    // In JSON mode, tracing output intentionally goes to stderr so stdout stays
    // machine-parseable — agents read stdout only.
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("warn")),
        )
        .with_writer(std::io::stderr)
        .init();

    let cli = Cli::parse();
    let fmt = cli.format;

    let result = match cli.command {
        Command::Show   { user_id, tier }    => cmd_show(&user_id, tier, fmt),
        Command::Status { user_id }          => cmd_status(&user_id, fmt),
        Command::Confirm    { user_id, field, index } => cmd_confirm(&user_id, &field, index, fmt),
        Command::Reject     { user_id, field, index } => cmd_reject(&user_id, &field, index, fmt),
        Command::ConfirmAll { user_id, field }         => cmd_confirm_all(&user_id, &field, fmt),
        Command::RejectAll  { user_id, field }         => cmd_reject_all(&user_id, &field, fmt),
        Command::Clear      { user_id, target }        => cmd_clear(&user_id, &target, fmt),
        Command::Delta(DeltaCmd::List    { user_id })            => cmd_delta_list(&user_id, fmt),
        Command::Delta(DeltaCmd::Resolve { user_id, id, keep }) => cmd_delta_resolve(&user_id, &id, &keep, fmt),
        Command::Review(ReviewCmd::List    { user_id })            => cmd_review_list(&user_id, fmt),
        Command::Review(ReviewCmd::Process { user_id, id, action }) => cmd_review_process(&user_id, &id, &action, fmt),
        Command::Annotate { user_id, field, note, pinned } => cmd_annotate(&user_id, &field, &note, pinned, fmt),
        Command::Diff { user_a, user_b } => cmd_diff(&user_a, &user_b, fmt),
        Command::Ingest { user_id, packet }  => cmd_ingest(&user_id, &packet, fmt),
        Command::ListUsers => cmd_list_users(fmt),
        Command::Decay { user_id, threshold } => cmd_decay(&user_id, threshold, fmt),
        Command::Watch { user_id, dir } => cmd_watch(&user_id, dir, fmt),
    };

    if let Err(e) = result {
        if fmt == Format::Json {
            println!("{}", serde_json::to_string_pretty(&ActionResult::err(e)).unwrap_or_else(|_| {
                r#"{"ok":false,"error":"serialization failure"}"#.to_string()
            }));
        } else {
            error!("{e}");
        }
        std::process::exit(1);
    }
}

fn build_npc_show_json(p: &mut crate::models::miin_profile::MiinProfileDocument, _tier: Tier) -> ShowJson {
    ShowJson {
        tier: _tier.to_string(),
        version: p.meta.version.clone(),
        overall_confidence: p.meta.overall_confidence,
        core: vec![],
        register: HashMap::new(),
        working: HashMap::new(),
        domains: vec![],
        values: vec![],
        reasoning: HashMap::new(),
        signals: HashMap::new(),
        annotations: vec![],
        unresolved_conflicts: vec![],
    }
}
