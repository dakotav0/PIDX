//! PIDX-type — calibration seed as personality code.
//!
//! The calibration seed IS the personality type. Decay rates, hardening
//! multipliers, and review thresholds encode how someone's patterns evolve.
//! A model reads this and attributes a qualitative type; the engine reads it
//! and adjusts its quantitative behavior.
//!
//! # Mapping
//!
//! | Signal | Meaning |
//! |--------|---------|
//! | identity_decay | Core stability (lower = more stable) |
//! | signal_decay | Verbal pattern volatility (higher = faster change) |
//! | working_decay | Work style stability |
//! | hardening_multiplier | Reinforcement sensitivity |
//! | review_threshold | Rigor / freshness bar |

use serde::{Deserialize, Serialize};

use chrono::Utc;

use super::calibration::CalibrationSeed;
use super::decay::FieldClass;
use super::profile::Register;

// ── PidxType ─────────────────────────────────────────────────────────────────

/// Derived personality type from a calibration seed.
///
/// This is the quantitative profile that surfaces from decay curves, hardening
/// rates, and domain volatility. MBTI compatibility is an optional overlay.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PidxType {
    /// Version of the calibration seed that produced this type.
    pub calibration_version: u32,
    /// Core identity stability (λ). Lower = more stable.
    pub identity_decay: f64,
    /// Verbal pattern volatility (λ). Higher = faster language shift.
    pub signal_decay: f64,
    /// Work style stability (λ).
    pub working_decay: f64,
    /// Value stability (λ).
    pub value_decay: f64,
    /// Domain expertise stability (λ).
    pub domain_decay: f64,
    /// Reinforcement amplification. Higher = patterns harden faster.
    pub hardening_multiplier: f64,
    /// Confidence threshold for review flagging. Higher = stricter.
    pub review_threshold: f64,
    /// Number of domains with explicit calibration overrides.
    pub domain_overrides: usize,
    /// MBTI-compatible type hint (optional, model-attributed).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mbti_hint: Option<String>,
}

impl PidxType {
    /// Derive a PIDX-type from a calibration seed.
    pub fn from_calibration(seed: &CalibrationSeed) -> Self {
        Self {
            calibration_version: seed.version,
            identity_decay: seed.lambda_for(FieldClass::Identity),
            signal_decay: seed.lambda_for(FieldClass::Signal),
            working_decay: seed.lambda_for(FieldClass::Working),
            value_decay: seed.lambda_for(FieldClass::Value),
            domain_decay: seed.lambda_for(FieldClass::Domain),
            hardening_multiplier: seed.hardening_multiplier,
            review_threshold: seed.review_threshold,
            domain_overrides: seed.domains.len(),
            mbti_hint: None,
        }
    }

    /// Human-readable summary.
    pub fn summary(&self) -> String {
        let stability = if self.identity_decay < 0.001 {
            "very stable"
        } else if self.identity_decay < 0.002 {
            "stable"
        } else {
            "fluid"
        };
        let signal = if self.signal_decay > 0.01 {
            "adaptive"
        } else {
            "consistent"
        };
        let hardening = if self.hardening_multiplier > 1.5 {
            "strong reinforcement"
        } else if self.hardening_multiplier > 1.1 {
            "moderate reinforcement"
        } else {
            "light reinforcement"
        };

        let mut s = format!(
            "PIDX-type v{} | core: {stability} | signals: {signal} | hardening: {hardening}",
            self.calibration_version
        );
        if let Some(mbti) = &self.mbti_hint {
            s.push_str(&format!(" | hint: {mbti}"));
        }
        s
    }
}

// ── MBTI compatibility ──────────────────────────────────────────────────────

/// Coarse MBTI axis mapping from calibration+register signals.
///
/// Uses **register scores** as the primary signal when available:
///
/// | Axis | Primary signal | Fallback (no register) |
/// |------|---------------|----------------------|
/// | I/E | signal_decay (social responsiveness) | — |
/// | S/N | abstraction score (>5→N) | working+domain decay (<0.005→S) |
/// | T/F | affect score (>5→F) | value decay + threshold (→T) |
/// | J/P | formality score (>5→J) | identity decay + hardening (→J) |
///
/// The fallback path preserves backward compatibility for profiles
/// without register evidence (e.g. newly seeded profiles).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MbtiAxis {
    /// Introversion (I) vs Extraversion (E) — derived from signal decay rate.
    /// Fast signal decay → more extraverted (responsive to social context).
    I,
    E,
    /// Sensing (S) vs Intuition (N) — derived from abstraction register score.
    /// High abstraction (>5) → intuition. Fallback: stable working+domain → S.
    S,
    N,
    /// Thinking (T) vs Feeling (F) — derived from affect register score.
    /// High affect (>5) → feeling. Fallback: stable values + high threshold → T.
    T,
    F,
    /// Judging (J) vs Perceiving (P) — derived from formality register score.
    /// High formality (>5) → judging. Fallback: stable identity + strong hardening → J.
    J,
    P,
}

impl PidxType {
    /// Derive coarse MBTI axes from calibration values and optional register data.
    ///
    /// Pass `register: Some(&profile.comm)` when a profile with register evidence
    /// is available. Pass `None` for the pure calibration-based fallback.
    pub fn mbti_axes(&self, register: Option<&Register>) -> [MbtiAxis; 4] {
        let now = Utc::now();

        // I/E: signal decay based — this measures social responsiveness well.
        let ie = if self.signal_decay > 0.015 {
            MbtiAxis::E
        } else {
            MbtiAxis::I
        };

        // S/N: register abstraction first, fallback to decay
        let sn = if let Some(reg) = register {
            let score = reg.abstraction.score(Some(now));
            if score > 5.0 {
                MbtiAxis::N
            } else {
                MbtiAxis::S
            }
        } else {
            if self.working_decay < 0.005 && self.domain_decay < 0.005 {
                MbtiAxis::S
            } else {
                MbtiAxis::N
            }
        };

        // T/F: register affect first, fallback to decay
        let tf = if let Some(reg) = register {
            let score = reg.affect.score(Some(now));
            if score > 5.0 {
                MbtiAxis::F
            } else {
                MbtiAxis::T
            }
        } else {
            if self.value_decay < 0.001 && self.review_threshold > 0.25 {
                MbtiAxis::T
            } else {
                MbtiAxis::F
            }
        };

        // J/P: register formality first, fallback to decay
        let jp = if let Some(reg) = register {
            let score = reg.formality.score(Some(now));
            if score > 5.0 {
                MbtiAxis::J
            } else {
                MbtiAxis::P
            }
        } else {
            if self.identity_decay < 0.001 && self.hardening_multiplier > 1.3 {
                MbtiAxis::J
            } else {
                MbtiAxis::P
            }
        };

        [ie, sn, tf, jp]
    }

    /// Format MBTI axes as a 4-letter code.
    ///
    /// Pass `register` when register data is available for more accurate
    /// S/N, T/F, and J/P axis resolution.
    pub fn mbti_code(&self, register: Option<&Register>) -> String {
        self.mbti_axes(register)
            .iter()
            .map(|a| format!("{a:?}"))
            .collect()
    }
}

// ── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_seed_produces_type() {
        let seed = CalibrationSeed::default_seed();
        let pt = PidxType::from_calibration(&seed);
        assert_eq!(pt.calibration_version, 1);
        assert!((pt.identity_decay - 0.0005).abs() < 1e-9);
        assert!((pt.signal_decay - 0.02).abs() < 1e-9);
        assert!(!pt.summary().is_empty());
    }

    #[test]
    fn mbti_code_is_four_letters() {
        let seed = CalibrationSeed::default_seed();
        let pt = PidxType::from_calibration(&seed);
        let code = pt.mbti_code(None);
        assert_eq!(code.len(), 4);
        assert!(code.contains('I') || code.contains('E'));
    }

    #[test]
    fn signal_decay_drives_ie_axis() {
        let mut seed = CalibrationSeed::default_seed();
        // Fast signal decay → E
        seed.decay.insert("signal".into(), 0.03);
        let pt = PidxType::from_calibration(&seed);
        assert_eq!(pt.mbti_axes(None)[0], MbtiAxis::E);

        // Slow signal decay → I
        seed.decay.insert("signal".into(), 0.005);
        let pt = PidxType::from_calibration(&seed);
        assert_eq!(pt.mbti_axes(None)[0], MbtiAxis::I);
    }
}
