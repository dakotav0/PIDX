//! Tier-scaled output generation.
//!
//! Produces a plain-markdown context block for injection at the start of a
//! model session. Only `confirmed` observations contribute — delta/proposed/
//! rejected/archived are invisible to the output layer.
//!
//! Tiers (approximate token budgets):
//!   Nano     ~180t   identity.core (top 3)
//!   Micro    ~550t   + register scores, working.mode + working.feedback
//!   Standard ~1400t  + PIDX-type, domains, values, identity.reasoning, full working
//!   Rich     ~3200t  + signals, pinned annotations, delta_queue summary

use std::fmt;

use chrono::Utc;

use crate::models::{
    decay::FieldClass,
    evidence::RegisterMetric,
    observation::{ObservationField, ObservationStatus, ObservationValue},
    profile::ProfileDocument,
};

// ── Tier enum ─────────────────────────────────────────────────────────────────

/// Output resolution tier.
///
/// Each tier is a strict superset of the one below it. The right choice depends
/// on the token budget available at the injection site.
#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub enum Tier {
    Nano,
    Micro,
    Standard,
    Rich,
}

impl fmt::Display for Tier {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Tier::Nano => write!(f, "nano"),
            Tier::Micro => write!(f, "micro"),
            Tier::Standard => write!(f, "standard"),
            Tier::Rich => write!(f, "rich"),
        }
    }
}

impl std::str::FromStr for Tier {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_ascii_lowercase().as_str() {
            "nano" => Ok(Tier::Nano),
            "micro" => Ok(Tier::Micro),
            "standard" => Ok(Tier::Standard),
            "rich" => Ok(Tier::Rich),
            other => Err(format!(
                "unknown tier: {other}; expected nano|micro|standard|rich"
            )),
        }
    }
}

// ── Helpers ───────────────────────────────────────────────────────────────────

/// Returns the display string for the active observation in a field, or None.
///
/// For Text values, this is just the string. For Domain values (which carry a
/// label + weight), this returns the label alone — the domain section has its
/// own formatter that also shows the weight percentage.
fn active_text(field: &ObservationField, fc: FieldClass) -> Option<String> {
    field.active(fc, None).map(|v| match v {
        ObservationValue::Text(s) => s.clone(),
        ObservationValue::Domain(d) => d.label.clone(),
        ObservationValue::Number(n) => n.to_string(),
    })
}

/// Display representation of any ObservationValue. Used in delta listings.
fn value_display(v: &ObservationValue) -> String {
    match v {
        ObservationValue::Text(s) => s.clone(),
        ObservationValue::Domain(d) => d.label.clone(),
        ObservationValue::Number(n) => n.to_string(),
    }
}

// ── Section builders ──────────────────────────────────────────────────────────

fn section_core(profile: &ProfileDocument, top_n: usize) -> Option<String> {
    let mut lines: Vec<String> = Vec::new();
    for field in &profile.identity.core {
        if let Some(text) = active_text(field, FieldClass::Identity) {
            lines.push(format!("- {text}"));
        }
        if lines.len() >= top_n {
            break;
        }
    }
    if lines.is_empty() {
        return None;
    }
    Some(format!("### CORE\n{}", lines.join("\n")))
}

/// PIDX-type section: derived personality type from calibration seed.
fn section_type(profile: &ProfileDocument) -> Option<String> {
    let seed = profile.meta.calibration.as_ref()?;
    let pt = crate::models::pidx_type::PidxType::from_calibration(seed);

    let stability = if pt.identity_decay < 0.001 {
        "very stable"
    } else if pt.identity_decay < 0.002 {
        "stable"
    } else {
        "fluid"
    };
    let signal = if pt.signal_decay > 0.01 {
        "adaptive"
    } else {
        "consistent"
    };

    let mut lines = vec![
        format!("**{}**", pt.summary()),
        String::new(),
        format!("| dimension | λ | trait |"),
        format!("|-----------|------|-------|"),
        format!("| identity  | {:.4} | {stability} |", pt.identity_decay),
        format!("| signal    | {:.4} | {signal} |", pt.signal_decay),
        format!("| working   | {:.4} | — |", pt.working_decay),
        format!("| value     | {:.4} | — |", pt.value_decay),
        format!("| domain    | {:.4} | — |", pt.domain_decay),
        format!("| hardening | {:.1}× | — |", pt.hardening_multiplier),
        format!("| threshold | {:.2} | — |", pt.review_threshold),
    ];

    let mbti = pt.mbti_code(Some(&profile.comm));
    if !mbti.is_empty() {
        lines.push(String::new());
        lines.push(format!("MBTI axes: **{}**", mbti));
    }

    let body = lines.join("\n");
    Some(format!("### PIDX-TYPE\n{}", body))
}

fn draw_markdown_register_bar(score: f64) -> String {
    let filled = (score.clamp(0.0, 10.0).round()) as usize;
    let mut bar = String::from("[");
    for i in 0..10 {
        if i < filled {
            bar.push('█');
        } else {
            bar.push('░');
        }
    }
    bar.push(']');
    bar
}

fn section_register(profile: &ProfileDocument) -> Option<String> {
    let now = Utc::now();
    let reg = &profile.comm;

    // Array of (&name, &metric) — Rust's answer to Python's getattr() iteration.
    // Since Register has named fields (not a Vec), we enumerate them explicitly.
    let metrics: [(&str, &RegisterMetric); 6] = [
        ("formality", &reg.formality),
        ("directness", &reg.directness),
        ("hedging", &reg.hedging),
        ("humor", &reg.humor),
        ("abstraction", &reg.abstraction),
        ("affect", &reg.affect),
    ];

    let mut lines: Vec<String> = Vec::new();
    for (name, metric) in &metrics {
        if metric.evidence.is_empty() {
            continue;
        }
        let score = metric.score(Some(now));
        let label = metric.score_label(Some(now));
        let bar = draw_markdown_register_bar(score);
        lines.push(format!("- {name}: {score:.1}/10 {bar} ({label})"));
    }
    if lines.is_empty() {
        return None;
    }
    Some(format!("### REGISTER\n{}", lines.join("\n")))
}

/// `fields` is a slice of field name strings to include, in order.
/// Passing `&["mode", "feedback"]` gives the Micro subset;
/// passing all four gives the Standard/Rich full view.
fn section_working(profile: &ProfileDocument, fields: &[&str]) -> Option<String> {
    let w = &profile.working;
    let mut lines: Vec<String> = Vec::new();
    for &name in fields {
        // Match the string name to the actual struct field.
        // This is the idiomatic Rust substitute for Python's getattr().
        let field = match name {
            "mode" => &w.mode,
            "pace" => &w.pace,
            "feedback" => &w.feedback,
            "pattern" => &w.pattern,
            _ => continue,
        };
        if let Some(text) = active_text(field, FieldClass::Working) {
            lines.push(format!("- {name}: {text}"));
        }
    }
    if lines.is_empty() {
        return None;
    }
    Some(format!("### WORKING STYLE\n{}", lines.join("\n")))
}

fn section_domains(profile: &ProfileDocument) -> Option<String> {
    let mut entries: Vec<String> = Vec::new();
    for field in &profile.domains {
        if let Some(v) = field.active(FieldClass::Domain, None) {
            let line = match v {
                // Domain entries carry a weight — show it as a percentage.
                ObservationValue::Domain(d) => {
                    format!("- {} ({:.0}%)", d.label, d.weight * 100.0)
                }
                ObservationValue::Text(s) => format!("- {s}"),
                ObservationValue::Number(n) => format!("- {n}"),
            };
            entries.push(line);
        }
    }
    if entries.is_empty() {
        return None;
    }
    Some(format!("### DOMAINS\n{}", entries.join("\n")))
}

fn section_values(profile: &ProfileDocument) -> Option<String> {
    let mut items: Vec<String> = Vec::new();
    for field in &profile.values {
        if let Some(text) = active_text(field, FieldClass::Value) {
            items.push(format!("- {text}"));
        }
    }
    if items.is_empty() {
        return None;
    }
    Some(format!("### VALUES\n{}", items.join("\n")))
}

fn section_reasoning(profile: &ProfileDocument) -> Option<String> {
    let r = &profile.identity.reasoning;
    let mut lines: Vec<String> = Vec::new();
    for (name, field) in [
        ("style", &r.style),
        ("pattern", &r.pattern),
        ("intake", &r.intake),
    ] {
        if let Some(text) = active_text(field, FieldClass::Identity) {
            lines.push(format!("- {name}: {text}"));
        }
    }
    if lines.is_empty() {
        return None;
    }
    Some(format!("### REASONING\n{}", lines.join("\n")))
}

fn section_signals(profile: &ProfileDocument) -> Option<String> {
    let s = &profile.signals;
    let mut parts: Vec<String> = Vec::new();

    for (cat, fields) in [
        ("phrases", s.phrases.as_slice()),
        ("avoidances", s.avoidances.as_slice()),
        ("rhythms", s.rhythms.as_slice()),
        ("framings", s.framings.as_slice()),
    ] {
        let mut items: Vec<String> = Vec::new();
        for field in fields {
            if let Some(text) = active_text(field, FieldClass::Signal) {
                items.push(format!("  - {text}"));
            }
        }
        if !items.is_empty() {
            parts.push(format!("**{cat}**\n{}", items.join("\n")));
        }
    }
    if parts.is_empty() {
        return None;
    }
    Some(format!("### SIGNALS\n{}", parts.join("\n\n")))
}

fn section_annotations(profile: &ProfileDocument) -> Option<String> {
    let pinned: Vec<_> = profile.annotations.iter().filter(|a| a.pinned).collect();
    if pinned.is_empty() {
        return None;
    }
    let lines: Vec<String> = pinned
        .iter()
        .map(|a| format!("- [{}] {}", a.author, a.note))
        .collect();
    Some(format!("### ANNOTATIONS (pinned)\n{}", lines.join("\n")))
}

fn section_extra(profile: &ProfileDocument) -> Option<String> {
    let mut lines: Vec<String> = Vec::new();
    for (key, fields) in &profile.extra {
        for field in fields {
            if let Some(text) = active_text(field, FieldClass::Extra) {
                lines.push(format!("- {key}: {text}"));
            }
        }
    }
    if lines.is_empty() {
        return None;
    }
    Some(format!("### EXTRA\n{}", lines.join("\n")))
}

fn section_deltas(profile: &ProfileDocument) -> Option<String> {
    let open: Vec<_> = profile.delta_queue.iter().filter(|d| !d.resolved).collect();
    if open.is_empty() {
        return None;
    }
    let lines: Vec<String> = open
        .iter()
        .map(|d| {
            format!(
                "- {}: [{}] \"{}\" vs [{}] \"{}\"",
                d.field,
                d.a.source.orientation,
                value_display(&d.a.value),
                d.b.source.orientation,
                value_display(&d.b.value),
            )
        })
        .collect();
    Some(format!(
        "### UNRESOLVED CONFLICTS ({} items)\n{}",
        open.len(),
        lines.join("\n")
    ))
}

// ── Public API ────────────────────────────────────────────────────────────────

/// Render a tier-scaled markdown context block for LLM injection.
///
/// Returns an empty string if no confirmed data exists at the requested tier.
/// Calls `recompute_overall_confidence()` internally — the header always
/// reflects the current state of the profile.
pub fn render_tier_output(profile: &mut ProfileDocument, tier: Tier) -> String {
    profile.recompute_overall_confidence();
    let conf = profile.meta.overall_confidence;
    let version = profile.meta.version.clone();
    let tier_label = tier.to_string().to_uppercase();

    let mut sections: Vec<String> = Vec::new();

    // ── Nano: identity core top 3 ──────────────────────────────────────────
    if let Some(s) = section_core(profile, 3) {
        sections.push(s);
    }

    if tier == Tier::Nano {
        return if sections.is_empty() {
            String::new()
        } else {
            format!(
                "## USER CONTEXT [v{version}] [{tier_label}] [conf:{conf:.2}]\n\n{}",
                sections.join("\n\n")
            )
        };
    }

    // ── Micro adds: register scores, working.mode + feedback ───────────────
    if let Some(s) = section_register(profile) {
        sections.push(s);
    }
    if let Some(s) = section_working(profile, &["mode", "feedback"]) {
        sections.push(s);
    }

    if tier == Tier::Micro {
        return if sections.is_empty() {
            String::new()
        } else {
            format!(
                "## USER CONTEXT [v{version}] [{tier_label}] [conf:{conf:.2}]\n\n{}",
                sections.join("\n\n")
            )
        };
    }

    // ── Standard adds: PIDX-type, domains, values, reasoning, full working ──
    if let Some(s) = section_type(profile) {
        sections.push(s);
    }
    if let Some(s) = section_domains(profile) {
        sections.push(s);
    }
    if let Some(s) = section_values(profile) {
        sections.push(s);
    }
    if let Some(s) = section_reasoning(profile) {
        sections.push(s);
    }
    // Replace the Micro working section (mode+feedback) with the full set.
    sections.retain(|s| !s.starts_with("### WORKING STYLE"));
    if let Some(s) = section_working(profile, &["mode", "pace", "feedback", "pattern"]) {
        sections.push(s);
    }

    if tier == Tier::Standard {
        return if sections.is_empty() {
            String::new()
        } else {
            format!(
                "## USER CONTEXT [v{version}] [{tier_label}] [conf:{conf:.2}]\n\n{}",
                sections.join("\n\n")
            )
        };
    }

    // ── Rich adds: signals, pinned annotations, delta summary ─────────────
    if let Some(s) = section_signals(profile) {
        sections.push(s);
    }
    if let Some(s) = section_extra(profile) {
        sections.push(s);
    }
    if let Some(s) = section_annotations(profile) {
        sections.push(s);
    }
    if let Some(s) = section_deltas(profile) {
        sections.push(s);
    }

    if sections.is_empty() {
        return String::new();
    }
    format!(
        "## USER CONTEXT [v{version}] [{tier_label}] [conf:{conf:.2}]\n\n{}",
        sections.join("\n\n")
    )
}

/// Resonance multiplier in [0.5, 2.0] based on profile register alignment
/// with content metadata.
///
/// `content_metadata` keys (all optional, 0–10 scale):
///   `complexity`, `emotional_depth`, `humor_level`, `abstractness`
///
/// A close match (<2 points apart) adds a small boost; a mismatch (>5 points)
/// subtracts the same amount. Neutral when evidence is absent or key is missing.
pub fn compute_resonance(
    profile: &ProfileDocument,
    content_metadata: &std::collections::HashMap<String, f64>,
) -> f64 {
    let now = Utc::now();
    let mut score = 1.0_f64;

    let align = |metric: &RegisterMetric, key: &str, weight: f64| -> f64 {
        if metric.evidence.is_empty() {
            return 0.0;
        }
        let user_score = metric.score(Some(now));
        let content_val = match content_metadata.get(key) {
            Some(&v) => v,
            None => return 0.0,
        };
        let diff = (user_score - content_val).abs();
        if diff < 2.0 {
            weight // close match → boost
        } else if diff > 5.0 {
            -weight // mismatch → penalty
        } else {
            0.0
        }
    };

    let reg = &profile.comm;
    score += align(&reg.abstraction, "abstractness", 0.12);
    score += align(&reg.humor, "humor_level", 0.08);
    score += align(&reg.formality, "complexity", 0.06);

    // Clamp to [0.5, 2.0] and round to 3 decimal places, matching Python.
    (score.clamp(0.5, 2.0) * 1000.0).round() / 1000.0
}

// ── Markdown export ─────────────────────────────────────────────────────────

pub fn render_markdown_export(
    profile: &mut ProfileDocument,
    path: &std::path::Path,
) -> Result<usize, std::io::Error> {
    profile.recompute_overall_confidence();
    let conf = profile.meta.overall_confidence;
    let version = &profile.meta.version;
    let id = &profile.meta.id;
    let now = chrono::Local::now().format("%Y-%m-%d %H:%M %Z").to_string();

    let mut doc = String::new();

    doc.push_str(&format!("# PIDX Profile: {id}\n\n"));
    doc.push_str(&format!(
        "**Version:** {version} | **Confidence:** {conf:.2} | **Exported:** {now}\n\n"
    ));
    doc.push_str("---\n\n");

    if let Some(cal) = &profile.meta.calibration {
        doc.push_str(&format!("## Calibration Seed (v{})\n\n", cal.version));
        doc.push_str(&format!(
            "- Hardening multiplier: {:.1}\n",
            cal.hardening_multiplier
        ));
        doc.push_str(&format!("- Max weight cap: {:.1}\n", cal.max_weight));
        doc.push_str(&format!(
            "- Review threshold: {:.2}\n",
            cal.review_threshold
        ));
        doc.push_str("\n### Decay Rates\n\n");
        let mut keys: Vec<&String> = cal.decay.keys().collect();
        keys.sort();
        for key in &keys {
            let lam = cal.decay[*key];
            doc.push_str(&format!("- **{key}**: λ = {lam}\n"));
        }
        if !cal.domains.is_empty() {
            doc.push_str("\n### Domain Overrides\n\n");
            let mut dkeys: Vec<&String> = cal.domains.keys().collect();
            dkeys.sort();
            for dk in &dkeys {
                let dc = &cal.domains[*dk];
                let mut parts = Vec::new();
                if let Some(d) = dc.decay {
                    parts.push(format!("decay={d}"));
                }
                if let Some(h) = dc.hardening_multiplier {
                    parts.push(format!("hardening={h}"));
                }
                doc.push_str(&format!("- **{dk}**: {}\n", parts.join(", ")));
            }
        }
        doc.push_str("\n---\n\n");
    }

    if !profile.identity.core.is_empty() {
        doc.push_str("## Identity Core\n\n");
        for (i, field) in profile.identity.core.iter().enumerate() {
            if let Some(v) = field.active(FieldClass::Identity, None) {
                let max_w = field
                    .observations
                    .iter()
                    .filter(|o| o.status == ObservationStatus::Confirmed)
                    .map(|o| o.weight)
                    .fold(0.0_f64, f64::max);
                let count = field.proposal_count;
                doc.push_str(&format!(
                    "{}. {}  \n   weight: {max_w:.2} | reinforced: {count}×\n\n",
                    i + 1,
                    value_display(v)
                ));
            }
        }
        doc.push('\n');
    }

    let now_utc = Utc::now();
    let reg = &profile.comm;
    let metrics: [(&str, &RegisterMetric); 6] = [
        ("formality", &reg.formality),
        ("directness", &reg.directness),
        ("hedging", &reg.hedging),
        ("humor", &reg.humor),
        ("abstraction", &reg.abstraction),
        ("affect", &reg.affect),
    ];
    if metrics.iter().any(|(_, m)| !m.evidence.is_empty()) {
        doc.push_str("## Register\n\n");
        for (name, metric) in &metrics {
            if !metric.evidence.is_empty() {
                let score = metric.score(Some(now_utc));
                let label = metric.score_label(Some(now_utc));
                doc.push_str(&format!("- **{name}**: {score:.1}/10 — {label}\n"));
            }
        }
        doc.push('\n');
    }

    let wk = &profile.working;
    for (name, field) in [
        ("mode", &wk.mode),
        ("pace", &wk.pace),
        ("feedback", &wk.feedback),
        ("pattern", &wk.pattern),
    ] {
        if let Some(v) = field.active(FieldClass::Working, None) {
            if name == "mode" {
                doc.push_str("## Working Style\n\n");
            }
            let max_w = field
                .observations
                .iter()
                .filter(|o| o.status == ObservationStatus::Confirmed)
                .map(|o| o.weight)
                .fold(0.0_f64, f64::max);
            doc.push_str(&format!(
                "- **{name}**: {} (w:{max_w:.2})\n",
                value_display(v)
            ));
        }
    }
    doc.push('\n');

    if !profile.domains.is_empty() {
        doc.push_str("## Domains\n\n");
        for field in &profile.domains {
            if let Some(v) = field.active(FieldClass::Domain, None) {
                let max_w = field
                    .observations
                    .iter()
                    .filter(|o| o.status == ObservationStatus::Confirmed)
                    .map(|o| o.weight)
                    .fold(0.0_f64, f64::max);
                let label = match v {
                    ObservationValue::Domain(d) => {
                        format!("{} ({:.0}%)", d.label, d.weight * 100.0)
                    }
                    _ => value_display(v),
                };
                doc.push_str(&format!("- {label} | weight: {max_w:.2}\n"));
            }
        }
        doc.push('\n');
    }

    if !profile.values.is_empty() {
        doc.push_str("## Values\n\n");
        for field in &profile.values {
            if let Some(v) = field.active(FieldClass::Value, None) {
                doc.push_str(&format!("- {}\n", value_display(v)));
            }
        }
        doc.push('\n');
    }

    let s = &profile.signals;
    let has_signals = [&s.phrases, &s.avoidances, &s.rhythms, &s.framings]
        .iter()
        .any(|slice| !slice.is_empty());
    if has_signals {
        doc.push_str("## Signals\n\n");
        for (cat, fields) in [
            ("phrases", s.phrases.as_slice()),
            ("avoidances", s.avoidances.as_slice()),
            ("rhythms", s.rhythms.as_slice()),
            ("framings", s.framings.as_slice()),
        ] {
            let mut items: Vec<String> = Vec::new();
            for field in fields {
                if let Some(v) = field.active(FieldClass::Signal, None) {
                    let max_w = field
                        .observations
                        .iter()
                        .filter(|o| o.status == ObservationStatus::Confirmed)
                        .map(|o| o.weight)
                        .fold(0.0_f64, f64::max);
                    let count = field.proposal_count;
                    items.push(format!("  - {} (w:{max_w:.2}, ×{count})", value_display(v)));
                }
            }
            if !items.is_empty() {
                doc.push_str(&format!("**{cat}**\n{}\n\n", items.join("\n")));
            }
        }
    }

    let pinned: Vec<_> = profile.annotations.iter().filter(|a| a.pinned).collect();
    if !pinned.is_empty() {
        doc.push_str("## Annotations\n\n");
        for a in &pinned {
            doc.push_str(&format!(
                "- [{author}] {note}\n",
                author = a.author,
                note = a.note
            ));
        }
        doc.push('\n');
    }

    if !profile.meta.ties.is_empty() {
        doc.push_str("## Ties\n\n");
        for tie in &profile.meta.ties {
            doc.push_str(&format!(
                "- **{target}** — {rel} (w:{w:.2})\n",
                target = tie.target,
                rel = tie.relationship,
                w = tie.weight
            ));
        }
        doc.push('\n');
    }

    let total_confirmed: usize = profile
        .identity
        .core
        .iter()
        .chain(profile.domains.iter())
        .chain(profile.values.iter())
        .flat_map(|f| f.observations.iter())
        .filter(|o| o.status == ObservationStatus::Confirmed)
        .count();
    doc.push_str(&format!(
        "---\n\n*{total_confirmed} confirmed observations | Profile confidence: {conf:.2}*\n"
    ));

    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(path, &doc)?;
    Ok(doc.len())
}
