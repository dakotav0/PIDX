//! LSM-style compaction: reclaim tombstoned fields without deleting the audit.
//!
//! The profile is an append-only log of observations. Archived/rejected
//! observations are permanent record (never deleted), so dead fields pile up
//! as tombstones: the live lists grow monotonically and status/display shows
//! index gaps. Compaction is the amortized answer — a bounded O(n) pass that:
//!
//!   1. partitions each field list into live (has confirmed/proposed/delta)
//!      and dead (all archived/rejected),
//!   2. moves dead fields into `ProfileDocument.archive` (original path +
//!      index preserved for the audit trail),
//!   3. rebuilds the live lists dense, so indices are contiguous again.
//!
//! The archive is never compacted — it is the labeled grave. Run compaction
//! on demand (`pidx compact`) or wire it to a dead-ratio trigger later.

use super::observation::{ArchivedField, ObservationField};
use super::profile::ProfileDocument;

/// Result of a compaction pass, for reporting.
#[derive(Debug, Clone, Default, serde::Serialize)]
pub struct CompactReport {
    /// Number of dead fields moved to the archive.
    pub fields_archived: usize,
    /// Number of observations preserved in the archive.
    pub observations_archived: usize,
    /// Number of lists examined.
    pub lists_compacted: usize,
    /// Total archived-field entries after this pass.
    pub archive_total: usize,
}

/// Partition one list: live fields keep their relative order (dense), dead
/// fields move to the archive with their original index.
fn compact_list(
    list: &mut Vec<ObservationField>,
    path: &str,
    archive: &mut Vec<ArchivedField>,
    report: &mut CompactReport,
) {
    let mut live: Vec<ObservationField> = Vec::with_capacity(list.len());
    let original: Vec<ObservationField> = std::mem::take(list);
    for (i, field) in original.into_iter().enumerate() {
        if field.has_live() {
            live.push(field);
        } else {
            report.fields_archived += 1;
            report.observations_archived += field.observations.len();
            archive.push(ArchivedField::new(path, i, &field));
        }
    }
    *list = live;
}

/// Compact every field list in the profile. Returns the report.
pub fn compact_profile(profile: &mut ProfileDocument) -> CompactReport {
    let mut report = CompactReport::default();
    let mut archive = std::mem::take(&mut profile.archive);

    // Scalar fields — single ObservationFields, compacted in place.
    compact_scalar(
        &mut profile.identity.reasoning.style,
        "identity.reasoning.style",
        &mut archive,
        &mut report,
    );
    compact_scalar(
        &mut profile.identity.reasoning.pattern,
        "identity.reasoning.pattern",
        &mut archive,
        &mut report,
    );
    compact_scalar(
        &mut profile.identity.reasoning.intake,
        "identity.reasoning.intake",
        &mut archive,
        &mut report,
    );
    compact_scalar(
        &mut profile.identity.reasoning.stance,
        "identity.reasoning.stance",
        &mut archive,
        &mut report,
    );

    // List fields
    compact_list(
        &mut profile.identity.core,
        "identity.core",
        &mut archive,
        &mut report,
    );
    compact_list(&mut profile.domains, "domains", &mut archive, &mut report);
    compact_list(&mut profile.values, "values", &mut archive, &mut report);
    compact_list(
        &mut profile.signals.phrases,
        "signals.phrases",
        &mut archive,
        &mut report,
    );
    compact_list(
        &mut profile.signals.avoidances,
        "signals.avoidances",
        &mut archive,
        &mut report,
    );
    compact_list(
        &mut profile.signals.rhythms,
        "signals.rhythms",
        &mut archive,
        &mut report,
    );
    compact_list(
        &mut profile.signals.framings,
        "signals.framings",
        &mut archive,
        &mut report,
    );

    // Extra bucket — each key is its own list.
    for (key, fields) in profile.extra.iter_mut() {
        compact_list(fields, &format!("extra.{key}"), &mut archive, &mut report);
    }

    // Working scalars
    compact_scalar(
        &mut profile.working.mode,
        "working.mode",
        &mut archive,
        &mut report,
    );
    compact_scalar(
        &mut profile.working.pace,
        "working.pace",
        &mut archive,
        &mut report,
    );
    compact_scalar(
        &mut profile.working.feedback,
        "working.feedback",
        &mut archive,
        &mut report,
    );
    compact_scalar(
        &mut profile.working.pattern,
        "working.pattern",
        &mut archive,
        &mut report,
    );

    report.lists_compacted = 15 + profile.extra.len();
    report.archive_total = archive.len();
    profile.archive = archive;
    report
}

/// Compact a single (scalar) field in place: if it has no live observations,
/// move its observations to the archive and reset it to default.
fn compact_scalar(
    field: &mut ObservationField,
    path: &str,
    archive: &mut Vec<ArchivedField>,
    report: &mut CompactReport,
) {
    if !field.has_live() && !field.observations.is_empty() {
        report.fields_archived += 1;
        report.observations_archived += field.observations.len();
        archive.push(ArchivedField::new(path, 0, field));
        *field = ObservationField::default();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::observation::{
        Observation, ObservationSource, ObservationStatus, ObservationValue,
    };
    use crate::models::profile::ProfileDocument;

    fn obs(status: ObservationStatus) -> Observation {
        Observation {
            value: ObservationValue::Text("v".into()),
            source: ObservationSource {
                origination: crate::models::confidence::Origination::System,
                orientation: "test".into(),
                session_ref: "s".into(),
                timestamp: "2026-01-01T00:00:00+00:00".into(),
            },
            confidence: 0.5,
            weight: 1.0,
            status,
            revision: 1,
            decay_exempt: false,
        }
    }

    #[test]
    fn has_live_distinguishes_live_from_dead() {
        let live = ObservationField {
            observations: vec![obs(ObservationStatus::Confirmed)],
            ..Default::default()
        };
        let dead = ObservationField {
            observations: vec![
                obs(ObservationStatus::Archived),
                obs(ObservationStatus::Rejected),
            ],
            ..Default::default()
        };
        assert!(live.has_live());
        assert!(!dead.has_live());
    }

    #[test]
    fn compaction_moves_dead_keeps_live_dense() {
        let mut profile = ProfileDocument::new("test");
        profile.signals.phrases = vec![
            ObservationField {
                observations: vec![obs(ObservationStatus::Archived)],
                ..Default::default()
            },
            ObservationField {
                observations: vec![obs(ObservationStatus::Confirmed)],
                ..Default::default()
            },
            ObservationField {
                observations: vec![obs(ObservationStatus::Rejected)],
                ..Default::default()
            },
            ObservationField {
                observations: vec![obs(ObservationStatus::Proposed)],
                ..Default::default()
            },
        ];

        let report = compact_profile(&mut profile);

        assert_eq!(report.fields_archived, 2);
        assert_eq!(report.observations_archived, 2);
        assert_eq!(profile.signals.phrases.len(), 2, "live list rebuilt dense");
        assert_eq!(profile.signals.phrases[0].live_count(), 1);
        assert_eq!(profile.signals.phrases[1].live_count(), 1);

        // Archive preserves path + original index for the audit trail.
        assert_eq!(report.archive_total, 2);
        let archived0 = profile.archive.iter().find(|a| a.index == 0).unwrap();
        assert_eq!(archived0.path, "signals.phrases");
        assert_eq!(
            archived0.observations[0].status,
            ObservationStatus::Archived
        );
        let archived2 = profile.archive.iter().find(|a| a.index == 2).unwrap();
        assert_eq!(
            archived2.observations[0].status,
            ObservationStatus::Rejected
        );
    }

    #[test]
    fn compaction_handles_extra_bucket_and_scalars() {
        let mut profile = ProfileDocument::new("test");
        profile.extra.insert(
            "moment".into(),
            vec![
                ObservationField {
                    observations: vec![obs(ObservationStatus::Archived)],
                    ..Default::default()
                },
                ObservationField {
                    observations: vec![obs(ObservationStatus::Confirmed)],
                    ..Default::default()
                },
            ],
        );
        profile.working.mode.observations = vec![obs(ObservationStatus::Archived)];

        let report = compact_profile(&mut profile);

        assert_eq!(report.fields_archived, 2); // one extra.moment dead + working.mode
        assert_eq!(profile.extra["moment"].len(), 1, "extra key compacted");
        assert!(
            profile.working.mode.observations.is_empty(),
            "scalar reset when dead"
        );
        assert!(profile
            .archive
            .iter()
            .any(|a| a.path == "extra.moment" && a.index == 0));
        assert!(profile.archive.iter().any(|a| a.path == "working.mode"));
    }

    #[test]
    fn compaction_is_noop_on_clean_profile() {
        let mut profile = ProfileDocument::new("test");
        profile.signals.phrases = vec![ObservationField {
            observations: vec![obs(ObservationStatus::Confirmed)],
            ..Default::default()
        }];
        let report = compact_profile(&mut profile);
        assert_eq!(report.fields_archived, 0);
        assert_eq!(profile.signals.phrases.len(), 1);
        assert!(profile.archive.is_empty());
    }
}
