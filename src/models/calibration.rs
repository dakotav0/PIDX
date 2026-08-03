//! Calibration seed — configurable decay, hardening, and threshold parameters.
//!
//! Stored as `~/.config/pidx/calibration.yml`. When absent, the engine uses
//! hardcoded defaults that match the pre-calibration behavior.
//!
//! # Inheritance
//!
//! Domain-level overrides follow inheritance: only deviations from the default
//! are stored. A domain without an override block inherits all top-level values.
//!
//! ```yaml
//! # Full calibration with one domain override
//! version: 1
//! decay:
//!   signal: 0.0200
//! domains:
//!   signal:
//!     decay: 0.0100   # override — this domain decays slower
//! ```
//!
//! # CLI
//!
//! `pidx calibrate --domain signal --decay 0.01` writes the override.
//! `pidx calibrate --domain signal --reset` removes it (back to inheritance).

use std::collections::HashMap;
use std::path::PathBuf;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::decay::FieldClass;

// ── CalibrationSeed ──────────────────────────────────────────────────────────

/// The full calibration configuration for the PIDX engine.
///
/// Serializes to/from `~/.config/pidx/calibration.yml`. Designed to be
/// versioned: each time the user adjusts a parameter, `version` increments
/// and a snapshot is kept.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct CalibrationSeed {
    /// Monotonically increasing version — bump on every adjustment.
    pub version: u32,
    /// Per-field-class decay rates (λ). Keyed by lowercase field class name.
    pub decay: HashMap<String, f64>,
    /// Multiplier applied to weight on each reinforcement match.
    #[serde(default = "default_hardening_multiplier")]
    pub hardening_multiplier: f64,
    /// Cap for observation weight (prevents runaway hardening).
    #[serde(default = "default_max_weight")]
    pub max_weight: f64,
    /// Confidence threshold below which observations are flagged for review.
    #[serde(default = "default_review_threshold")]
    pub review_threshold: f64,
    /// Per-domain overrides. Only domains with deviations from the default
    /// appear here. An empty map means all domains inherit top-level values.
    #[serde(default)]
    pub domains: HashMap<String, DomainCalibration>,
}

/// Per-domain calibration override. Only fields that differ from the
/// top-level default are set — everything else inherits.
#[derive(Debug, Clone, Serialize, Deserialize, Default, JsonSchema)]
pub struct DomainCalibration {
    /// Override for the decay rate (λ). None = inherit from top-level.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub decay: Option<f64>,
    /// Override for hardening multiplier.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hardening_multiplier: Option<f64>,
    /// Override for max weight cap.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_weight: Option<f64>,
    /// Override for review threshold (confidence floor before flagging).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub review_threshold: Option<f64>,
}

// ── Default generators (serde) ─────────────────────────────────────────────

fn default_hardening_multiplier() -> f64 {
    1.2
}
fn default_max_weight() -> f64 {
    5.0
}
fn default_review_threshold() -> f64 {
    0.30
}

// ── Construction ────────────────────────────────────────────────────────────

impl CalibrationSeed {
    /// Create the default calibration seed (matching pre-calibration hardcoded values).
    pub fn default_seed() -> Self {
        let mut decay = HashMap::new();
        decay.insert("identity".into(), 0.0005);
        decay.insert("value".into(), 0.0008);
        decay.insert("register".into(), 0.0100);
        decay.insert("domain".into(), 0.0080);
        decay.insert("working".into(), 0.0070);
        decay.insert("signal".into(), 0.0200);
        decay.insert("annotation".into(), 0.0);

        Self {
            version: 1,
            decay,
            hardening_multiplier: default_hardening_multiplier(),
            max_weight: default_max_weight(),
            review_threshold: default_review_threshold(),
            domains: HashMap::new(),
        }
    }

    /// Resolve the decay rate (λ) for a field class, respecting domain overrides.
    ///
    /// Checks: domain override → top-level decay → hardcoded default.
    pub fn lambda_for(&self, field_class: FieldClass) -> f64 {
        let key = field_class.lowercase_key();
        // Domain override
        if let Some(domain_cal) = self.domains.get(key) {
            if let Some(decay) = domain_cal.decay {
                return decay;
            }
        }
        // Top-level
        self.decay
            .get(key)
            .copied()
            .unwrap_or_else(|| field_class.default_lambda())
    }

    /// Resolve hardening multiplier for a field class.
    pub fn hardening_for(&self, field_class: FieldClass) -> f64 {
        let key = field_class.lowercase_key();
        if let Some(domain_cal) = self.domains.get(key) {
            if let Some(hm) = domain_cal.hardening_multiplier {
                return hm;
            }
        }
        self.hardening_multiplier
    }

    /// Resolve max weight for a field class.
    pub fn max_weight_for(&self, field_class: FieldClass) -> f64 {
        let key = field_class.lowercase_key();
        if let Some(domain_cal) = self.domains.get(key) {
            if let Some(mw) = domain_cal.max_weight {
                return mw;
            }
        }
        self.max_weight
    }

    /// Resolve review threshold for a field class.
    pub fn review_threshold_for(&self, field_class: FieldClass) -> f64 {
        let key = field_class.lowercase_key();
        if let Some(domain_cal) = self.domains.get(key) {
            if let Some(rt) = domain_cal.review_threshold {
                return rt;
            }
        }
        self.review_threshold
    }
}

// ── File I/O ────────────────────────────────────────────────────────────────

/// Path to the calibration file: `~/.config/pidx/calibration.yml`.
pub fn calibration_path() -> PathBuf {
    let base = dirs::config_dir().unwrap_or_else(|| PathBuf::from("."));
    base.join("pidx").join("calibration.yml")
}

/// Load the calibration seed from disk. Returns `None` if the file doesn't exist
/// or can't be parsed (falls back to hardcoded defaults).
pub fn load_calibration() -> Option<CalibrationSeed> {
    let path = calibration_path();
    if !path.exists() {
        return None;
    }
    let content = std::fs::read_to_string(&path).ok()?;
    serde_yaml::from_str(&content).ok()
}

/// Write a calibration seed to disk, creating parent directories as needed.
pub fn save_calibration(seed: &CalibrationSeed) -> anyhow::Result<()> {
    let path = calibration_path();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let yaml = serde_yaml::to_string(seed)?;
    std::fs::write(&path, yaml)?;
    Ok(())
}

/// Get or create the calibration seed. If the file exists and parses, return it.
/// Otherwise, create a default seed, write it to disk, and return it.
pub fn get_or_create_calibration() -> CalibrationSeed {
    if let Some(seed) = load_calibration() {
        return seed;
    }
    let seed = CalibrationSeed::default_seed();
    let _ = save_calibration(&seed);
    seed
}

// ── Calibrate command helpers ───────────────────────────────────────────────

/// Set a domain-level decay override. If `decay` is `None`, removes the override
/// (back to inheritance). Returns the updated seed.
pub fn calibrate_domain(
    seed: &mut CalibrationSeed,
    domain: &str,
    decay: Option<f64>,
    hardening: Option<f64>,
    max_weight: Option<f64>,
    review_threshold: Option<f64>,
) {
    // Remove override entirely if all values are None and no existing override
    let has_override = seed.domains.contains_key(domain);

    if decay.is_none() && hardening.is_none() && max_weight.is_none() && review_threshold.is_none()
    {
        if has_override {
            seed.domains.remove(domain);
            seed.version += 1;
        }
        return;
    }

    let entry = seed.domains.entry(domain.to_string()).or_default();
    if let Some(d) = decay {
        entry.decay = Some(d);
    }
    if let Some(h) = hardening {
        entry.hardening_multiplier = Some(h);
    }
    if let Some(mw) = max_weight {
        entry.max_weight = Some(mw);
    }
    if let Some(rt) = review_threshold {
        entry.review_threshold = Some(rt);
    }
    seed.version += 1;
}

/// Reset a domain back to inheritance (remove its override block).
pub fn reset_domain(seed: &mut CalibrationSeed, domain: &str) -> bool {
    if seed.domains.remove(domain).is_some() {
        seed.version += 1;
        true
    } else {
        false
    }
}

// ── FieldClass extensions ───────────────────────────────────────────────────

impl FieldClass {
    /// Lowercase key for calibration lookup ("identity", "signal", etc.).
    pub fn lowercase_key(self) -> &'static str {
        match self {
            FieldClass::Identity => "identity",
            FieldClass::Value => "value",
            FieldClass::Register => "register",
            FieldClass::Domain => "domain",
            FieldClass::Working => "working",
            FieldClass::Signal => "signal",
            FieldClass::Annotation => "annotation",
            FieldClass::Extra => "extra",
        }
    }

    /// Default λ when no calibration seed is loaded (matches pre-calibration behavior).
    pub fn default_lambda(self) -> f64 {
        self.lambda()
    }
}

// ── Derivation ───────────────────────────────────────────────────────────────

use crate::models::observation::{ObservationField, ObservationStatus};
use crate::models::profile::ProfileDocument;

/// Statistics collected for a single FieldClass during calibration derivation.
struct FcDerivationStats {
    /// Total confirmed observations across all fields of this class.
    confirmed_count: usize,
    /// Sum of base confidence across confirmed observations.
    total_confidence: f64,
    /// Sum of weight across confirmed observations.
    total_weight: f64,
    /// Sum of field-level proposal counts.
    total_proposal: u32,
    /// Number of fields that have any observations (confirmed or not).
    field_count: usize,
}

impl FcDerivationStats {
    fn new() -> Self {
        Self {
            confirmed_count: 0,
            total_confidence: 0.0,
            total_weight: 0.0,
            total_proposal: 0,
            field_count: 0,
        }
    }

    fn add_field(&mut self, field: &ObservationField) {
        self.field_count += 1;
        self.total_proposal += field.proposal_count;
        for obs in &field.observations {
            if obs.status == ObservationStatus::Confirmed {
                self.confirmed_count += 1;
                self.total_confidence += obs.confidence;
                self.total_weight += obs.weight;
            }
        }
    }

    fn avg_confidence(&self) -> f64 {
        if self.confirmed_count == 0 {
            0.0
        } else {
            self.total_confidence / self.confirmed_count as f64
        }
    }

    fn avg_weight(&self) -> f64 {
        if self.confirmed_count == 0 {
            0.0
        } else {
            self.total_weight / self.confirmed_count as f64
        }
    }

    fn avg_proposal_count(&self) -> f64 {
        if self.field_count == 0 {
            0.0
        } else {
            self.total_proposal as f64 / self.field_count as f64
        }
    }
}

/// Collect stats for identity class: core traits + reasoning fields.
fn collect_identity_stats(profile: &ProfileDocument) -> FcDerivationStats {
    let mut s = FcDerivationStats::new();
    for field in &profile.identity.core {
        s.add_field(field);
    }
    s.add_field(&profile.identity.reasoning.style);
    s.add_field(&profile.identity.reasoning.pattern);
    s.add_field(&profile.identity.reasoning.intake);
    s.add_field(&profile.identity.reasoning.stance);
    s
}

/// Collect stats for register: evidence count across all metrics.
fn collect_register_stats(profile: &ProfileDocument) -> FcDerivationStats {
    let mut s = FcDerivationStats::new();
    let metrics = [
        &profile.comm.formality,
        &profile.comm.directness,
        &profile.comm.hedging,
        &profile.comm.humor,
        &profile.comm.abstraction,
        &profile.comm.affect,
    ];
    for metric in &metrics {
        let count = metric.evidence.len();
        if count > 0 {
            s.field_count += 1;
            s.confirmed_count += count;
            // Use average evidence weight as proxy for confidence
            let avg_w: f64 = metric.evidence.iter().map(|e| e.weight).sum::<f64>() / count as f64;
            s.total_confidence += avg_w * count as f64;
            s.total_weight += avg_w * count as f64;
        }
    }
    s
}

/// Derive a decay lambda for a field class from its observation stats.
///
/// Formula:
///   maturity = avg_confidence × avg_weight × ln(1 + confirmed_count)
///   adjusted = base_lambda / (1.0 + maturity_scale × maturity)
///
/// More observations with higher confidence → more stable → lower decay.
/// Clamped to [base_lambda × 0.2, base_lambda × 2.0] to prevent extremes.
fn derive_lambda(base: f64, stats: &FcDerivationStats, maturity_scale: f64) -> f64 {
    if stats.confirmed_count == 0 || stats.field_count == 0 {
        return base; // No data → use default
    }
    let maturity = stats.avg_confidence()
        * stats.avg_weight().clamp(0.5, 5.0)
        * (1.0 + stats.confirmed_count as f64).ln();
    let adjusted = base / (1.0 + maturity_scale * maturity);
    adjusted.clamp(base * 0.2, base * 2.0)
}

/// Derive a complete calibration seed from a profile's observations.
///
/// Analyzes each FieldClass independently to set appropriate decay rates,
/// then computes hardening sensitivity and review strictness from the
/// overall profile maturity.
///
/// Returns a `CalibrationSeed` that can be stored in `ProfileMeta.calibration`.
pub fn derive_calibration(profile: &ProfileDocument) -> CalibrationSeed {
    let mut decay = std::collections::HashMap::new();

    // ── Per-class analysis ────────────────────────────────────────────
    let identity_stats = collect_identity_stats(profile);
    decay.insert(
        "identity".into(),
        derive_lambda(FieldClass::Identity.lambda(), &identity_stats, 5.0),
    );

    let value_stats = {
        let mut s = FcDerivationStats::new();
        for field in &profile.values {
            s.add_field(field);
        }
        s
    };
    decay.insert(
        "value".into(),
        derive_lambda(FieldClass::Value.lambda(), &value_stats, 4.0),
    );

    let register_stats = collect_register_stats(profile);
    decay.insert(
        "register".into(),
        derive_lambda(FieldClass::Register.lambda(), &register_stats, 3.0),
    );

    let domain_stats = {
        let mut s = FcDerivationStats::new();
        for field in &profile.domains {
            s.add_field(field);
        }
        s
    };
    decay.insert(
        "domain".into(),
        derive_lambda(FieldClass::Domain.lambda(), &domain_stats, 3.0),
    );

    let working_stats = {
        let mut s = FcDerivationStats::new();
        s.add_field(&profile.working.mode);
        s.add_field(&profile.working.pace);
        s.add_field(&profile.working.feedback);
        s.add_field(&profile.working.pattern);
        s
    };
    decay.insert(
        "working".into(),
        derive_lambda(FieldClass::Working.lambda(), &working_stats, 4.0),
    );

    let signal_stats = {
        let mut s = FcDerivationStats::new();
        for field in &profile.signals.phrases {
            s.add_field(field);
        }
        for field in &profile.signals.avoidances {
            s.add_field(field);
        }
        for field in &profile.signals.rhythms {
            s.add_field(field);
        }
        for field in &profile.signals.framings {
            s.add_field(field);
        }
        s
    };
    decay.insert(
        "signal".into(),
        derive_lambda(FieldClass::Signal.lambda(), &signal_stats, 3.0),
    );

    // Annotations never decay — always λ = 0.0
    decay.insert("annotation".into(), 0.0);

    // ── Global parameters ─────────────────────────────────────────────

    // Hardening multiplier: how much reinforcement accelerates weight growth.
    // Higher avg proposal count across all fields → stronger hardening.
    let all_stats = [
        &identity_stats,
        &value_stats,
        &register_stats,
        &domain_stats,
        &working_stats,
        &signal_stats,
    ];
    let non_empty: Vec<_> = all_stats.iter().filter(|s| s.field_count > 0).collect();
    let all_proposal_avg = if non_empty.is_empty() {
        0.0
    } else {
        non_empty
            .iter()
            .map(|s| s.avg_proposal_count())
            .sum::<f64>()
            / non_empty.len() as f64
    };
    let hardening_multiplier = if all_proposal_avg > 1.0 {
        (1.2 + 0.3 * all_proposal_avg.log2()).clamp(1.0, 2.5)
    } else {
        1.2
    };

    // Max weight cap: scale with profile maturity.
    // More confirmed obs → higher cap makes room for reinforcement growth.
    let total_confirmed: usize = identity_stats.confirmed_count
        + value_stats.confirmed_count
        + domain_stats.confirmed_count
        + working_stats.confirmed_count
        + signal_stats.confirmed_count;
    let max_weight = (3.0 + 0.5 * (total_confirmed as f64).sqrt()).clamp(3.0, 10.0);

    // Review threshold: stricter for mature profiles.
    let maturity_factor = (total_confirmed as f64 / 50.0).clamp(0.0, 1.0);
    let review_threshold = 0.15 + 0.35 * maturity_factor;

    CalibrationSeed {
        version: 1,
        decay,
        hardening_multiplier,
        max_weight,
        review_threshold,
        domains: std::collections::HashMap::new(),
    }
}

/// Derive calibration, store it in the profile, and bump the version.
///
/// Convenience wrapper that updates both the profile meta and the
/// calibration seed's version to track calibration changes.
pub fn derive_and_store_calibration(profile: &mut ProfileDocument) {
    let seed = derive_calibration(profile);
    profile.meta.calibration = Some(seed);
    profile.meta.bump_version();
}

// ── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_seed_matches_hardcoded_lambdas() {
        let seed = CalibrationSeed::default_seed();
        assert!((seed.lambda_for(FieldClass::Identity) - 0.0005).abs() < 1e-9);
        assert!((seed.lambda_for(FieldClass::Signal) - 0.0200).abs() < 1e-9);
        assert!((seed.lambda_for(FieldClass::Annotation) - 0.0).abs() < 1e-9);
    }

    #[test]
    fn domain_override_takes_priority() {
        let mut seed = CalibrationSeed::default_seed();
        seed.domains.insert(
            "signal".into(),
            DomainCalibration {
                decay: Some(0.005),
                hardening_multiplier: None,
                max_weight: None,
                review_threshold: None,
            },
        );
        assert!((seed.lambda_for(FieldClass::Signal) - 0.005).abs() < 1e-9);
        // Identity still uses top-level default
        assert!((seed.lambda_for(FieldClass::Identity) - 0.0005).abs() < 1e-9);
    }

    #[test]
    fn calibrate_domain_writes_and_resets() {
        let mut seed = CalibrationSeed::default_seed();
        let v1 = seed.version;

        calibrate_domain(&mut seed, "signal", Some(0.01), None, None, None);
        assert!(seed.version > v1);
        assert!((seed.lambda_for(FieldClass::Signal) - 0.01).abs() < 1e-9);

        // Reset
        assert!(reset_domain(&mut seed, "signal"));
        assert!((seed.lambda_for(FieldClass::Signal) - 0.0200).abs() < 1e-9);
    }

    #[test]
    fn roundtrip_yaml() {
        let seed = CalibrationSeed::default_seed();
        let yaml = serde_yaml::to_string(&seed).unwrap();
        let parsed: CalibrationSeed = serde_yaml::from_str(&yaml).unwrap();
        assert_eq!(seed.version, parsed.version);
        assert!((parsed.hardening_multiplier - 1.2).abs() < 1e-9);
    }
}
