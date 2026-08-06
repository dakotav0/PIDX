//! Read-side query surface for observations.
//!
//! The missing read half of the propose → validate → review → act loop
//! (TODO bug #2): enumerate observations with their full paths, filterable by
//! status / path prefix / term. Borrows Birchbark's list/get read shape —
//! observations are structured, so the honest analog is prefix + substring
//! filters, not semantic search (that's mRAG's lane).

use schemars::JsonSchema;
use serde::Serialize;

use crate::models::observation::{ObservationField, ObservationStatus, ObservationValue};
use crate::models::profile::ProfileDocument;

/// One observation, addressable and filterable.
#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct ObservationRow {
    /// Full dot-path: "signals.phrases.2" or "working.mode".
    pub path: String,
    /// Slot of the field within its list (0 for scalars).
    pub index: usize,
    /// Index of this observation within the field's observations.
    pub obs_index: usize,
    /// Stringified value.
    pub value: String,
    pub status: ObservationStatus,
    /// Short provenance label (orientation).
    pub source: String,
    pub confidence: f64,
    /// Observation timestamp (RFC3339).
    pub updated: String,
}

/// Filters for `list_observations`.
#[derive(Debug, Default, Clone)]
pub struct ObservationQuery {
    /// Only rows with this status. None = all statuses.
    pub status: Option<ObservationStatus>,
    /// Only rows whose path starts with this prefix (e.g. "signals.phrases").
    pub path_prefix: Option<String>,
    /// Only rows whose value contains this substring (case-insensitive).
    pub term: Option<String>,
    /// Cap on returned rows (applied after filtering).
    pub limit: Option<usize>,
}

/// Stringify an observation value for display/filtering.
pub fn value_text(v: &ObservationValue) -> String {
    match v {
        ObservationValue::Text(s) => s.clone(),
        ObservationValue::Domain(d) => d.label.clone(),
        ObservationValue::Number(n) => n.to_string(),
    }
}

/// Lowercase status label for display.
pub fn status_str(s: ObservationStatus) -> &'static str {
    match s {
        ObservationStatus::Proposed => "proposed",
        ObservationStatus::Confirmed => "confirmed",
        ObservationStatus::Rejected => "rejected",
        ObservationStatus::Delta => "delta",
        ObservationStatus::Archived => "archived",
    }
}

fn push_rows(out: &mut Vec<ObservationRow>, field: &ObservationField, path: &str, index: usize) {
    for (obs_index, o) in field.observations.iter().enumerate() {
        out.push(ObservationRow {
            path: path.to_string(),
            index,
            obs_index,
            value: value_text(&o.value),
            status: o.status,
            source: o.source.orientation.clone(),
            confidence: o.confidence,
            updated: o.source.timestamp.clone(),
        });
    }
}

/// Enumerate every observation in the profile with its full path, filtered by
/// `q`. Scalar fields resolve to `working.mode`-style paths; list slots to
/// `signals.phrases.2`-style; extra-bucket keys to `extra.<key>.<slot>`.
pub fn list_observations(profile: &ProfileDocument, q: &ObservationQuery) -> Vec<ObservationRow> {
    let mut rows = Vec::new();

    macro_rules! scalar {
        ($f:expr, $path:expr) => {
            push_rows(&mut rows, $f, $path, 0);
        };
    }
    macro_rules! list {
        ($list:expr, $prefix:expr) => {
            for (i, f) in $list.iter().enumerate() {
                push_rows(&mut rows, f, &format!("{}.{}", $prefix, i), i);
            }
        };
    }

    scalar!(&profile.working.mode, "working.mode");
    scalar!(&profile.working.pace, "working.pace");
    scalar!(&profile.working.feedback, "working.feedback");
    scalar!(&profile.working.pattern, "working.pattern");
    scalar!(
        &profile.identity.reasoning.style,
        "identity.reasoning.style"
    );
    scalar!(
        &profile.identity.reasoning.pattern,
        "identity.reasoning.pattern"
    );
    scalar!(
        &profile.identity.reasoning.intake,
        "identity.reasoning.intake"
    );
    scalar!(
        &profile.identity.reasoning.stance,
        "identity.reasoning.stance"
    );
    list!(&profile.identity.core, "identity.core");
    list!(&profile.domains, "domains");
    list!(&profile.values, "values");
    list!(&profile.signals.phrases, "signals.phrases");
    list!(&profile.signals.avoidances, "signals.avoidances");
    list!(&profile.signals.rhythms, "signals.rhythms");
    list!(&profile.signals.framings, "signals.framings");
    for (key, fields) in &profile.extra {
        for (i, f) in fields.iter().enumerate() {
            push_rows(&mut rows, f, &format!("extra.{}.{}", key, i), i);
        }
    }

    let mut out: Vec<ObservationRow> = rows
        .into_iter()
        .filter(|r| {
            if let Some(s) = &q.status {
                if r.status != *s {
                    return false;
                }
            }
            if let Some(p) = &q.path_prefix {
                if !r.path.starts_with(p.as_str()) {
                    return false;
                }
            }
            if let Some(t) = &q.term {
                if !r.value.to_lowercase().contains(&t.to_lowercase()) {
                    return false;
                }
            }
            true
        })
        .collect();

    if let Some(n) = q.limit {
        out.truncate(n);
    }
    out
}

/// Get every observation of one field by its exact full path
/// ("signals.phrases.2", "working.mode", "extra.moment.0").
pub fn get_field_rows(profile: &ProfileDocument, path: &str) -> Option<Vec<ObservationRow>> {
    let q = ObservationQuery {
        path_prefix: Some(path.to_string()),
        ..Default::default()
    };
    let rows: Vec<ObservationRow> = list_observations(profile, &q)
        .into_iter()
        .filter(|r| r.path == path)
        .collect();
    if rows.is_empty() {
        None
    } else {
        Some(rows)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::confidence::Origination;
    use crate::models::observation::Observation;
    use crate::models::observation::{ObservationSource, ObservationValue};
    use crate::models::profile::ProfileDocument;

    fn obs(status: ObservationStatus, value: &str) -> Observation {
        Observation {
            value: ObservationValue::Text(value.into()),
            status,
            confidence: 0.7,
            weight: 0.7,
            decay_exempt: false,
            revision: 1,
            source: ObservationSource {
                origination: Origination::Active,
                orientation: "test".into(),
                session_ref: "s".into(),
                timestamp: "2026-08-03T00:00:00+00:00".into(),
            },
        }
    }

    fn field(observations: Vec<Observation>) -> ObservationField {
        ObservationField {
            observations,
            proposal_count: 0,
        }
    }

    #[test]
    fn lists_observations_with_paths() {
        let mut p = ProfileDocument::new("test");
        p.signals.phrases = vec![field(vec![obs(
            ObservationStatus::Confirmed,
            "micro project",
        )])];
        p.working.mode = field(vec![obs(ObservationStatus::Proposed, "deep work")]);
        p.extra.insert(
            "moment".into(),
            vec![field(vec![obs(
                ObservationStatus::Confirmed,
                "shared listening",
            )])],
        );

        let rows = list_observations(&p, &ObservationQuery::default());
        let paths: Vec<&str> = rows.iter().map(|r| r.path.as_str()).collect();
        assert!(paths.contains(&"signals.phrases.0"));
        assert!(paths.contains(&"working.mode"));
        assert!(paths.contains(&"extra.moment.0"));
    }

    #[test]
    fn filters_by_status_and_term() {
        let mut p = ProfileDocument::new("test");
        p.signals.phrases = vec![
            field(vec![obs(ObservationStatus::Confirmed, "micro project")]),
            field(vec![obs(ObservationStatus::Proposed, "maybe invisible")]),
        ];
        let proposed = list_observations(
            &p,
            &ObservationQuery {
                status: Some(ObservationStatus::Proposed),
                ..Default::default()
            },
        );
        assert_eq!(proposed.len(), 1);
        assert_eq!(proposed[0].path, "signals.phrases.1");
        assert_eq!(proposed[0].value, "maybe invisible");

        let term = list_observations(
            &p,
            &ObservationQuery {
                term: Some("MICRO".into()),
                ..Default::default()
            },
        );
        assert_eq!(term.len(), 1);
        assert_eq!(term[0].path, "signals.phrases.0");
    }

    #[test]
    fn get_field_rows_exact_path() {
        let mut p = ProfileDocument::new("test");
        p.signals.phrases = vec![field(vec![obs(ObservationStatus::Confirmed, "one")])];
        assert_eq!(get_field_rows(&p, "signals.phrases.0").unwrap().len(), 1);
        assert!(get_field_rows(&p, "signals.phrases.9").is_none());
        assert!(get_field_rows(&p, "signals.rhythms.0").is_none());
    }
}
