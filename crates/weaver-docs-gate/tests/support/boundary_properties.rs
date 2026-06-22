//! Property tests for boundary-manifest validation helpers.
//!
//! These tests exercise the pure validation rules used by the integration
//! gate. They keep generated ISO date shapes and state/evidence combinations
//! separate from filesystem and Markdown rendering concerns.

use std::collections::BTreeSet;

use proptest::prelude::*;
use weaver_docs_gate::{BoundaryManifest, BoundaryState, BoundaryTask, UpstreamRef, UpstreamRole};

use super::{
    is_iso_date,
    markdown_anchor,
    validate_divergent_adr_anchor,
    validate_manifest_registry,
    validate_state_evidence,
};

/// Generated state-specific evidence for one manifest row.
struct Evidence {
    shipped_in: Option<String>,
    removal_gate: Option<String>,
    adr_anchor: Option<String>,
    next_review_by: Option<String>,
}

/// Generated non-evidence fields for one manifest row.
#[derive(Debug)]
struct TaskFields {
    id: String,
    gist: String,
    upstream_task: String,
    last_reviewed: String,
}

/// Presence flags used to generate and check state evidence.
#[derive(Clone, Copy)]
struct EvidenceFlags(u8);

const SHIPPED_IN: u8 = 0b0001;
const REMOVAL_GATE: u8 = 0b0010;
const ADR_ANCHOR: u8 = 0b0100;
const NEXT_REVIEW_BY: u8 = 0b1000;

impl Evidence {
    /// Build optional evidence fields from generated presence flags.
    fn from_flags(flags: EvidenceFlags) -> Self {
        Self {
            shipped_in: flags.has(SHIPPED_IN).then(|| "4339a6f3".into()),
            removal_gate: flags
                .has(REMOVAL_GATE)
                .then(|| "remove once upstream ships".into()),
            adr_anchor: flags
                .has(ADR_ANCHOR)
                .then(|| "intentional-divergence".into()),
            next_review_by: flags.has(NEXT_REVIEW_BY).then(|| "2026-06-30".into()),
        }
    }
}

impl TaskFields {
    /// Build representative fields for properties that vary only evidence.
    fn representative() -> Self {
        Self {
            id: "12.1.1".into(),
            gist: "Track the downstream consumer boundary.".into(),
            upstream_task: "renderer-contract".into(),
            last_reviewed: "2026-06-20".into(),
        }
    }
}

impl EvidenceFlags {
    /// Return whether the named evidence bit is present.
    const fn has(self, mask: u8) -> bool { self.0 & mask != 0 }
}

proptest! {
    #![proptest_config(ProptestConfig {
        failure_persistence: None,
        ..ProptestConfig::default()
    })]

    #[test]
    /// Prove arbitrary registry IDs must exactly match row IDs and be unique.
    fn manifest_referential_integrity_matches_generated_task_ids(
        managed_tasks in proptest::collection::vec(task_id_text(), 0..24),
        row_ids in proptest::collection::vec(task_id_text(), 0..24),
    ) {
        let expected = managed_tasks == row_ids && ids_are_unique(&managed_tasks);
        let manifest = manifest_with_ids(managed_tasks, row_ids);

        prop_assert_eq!(validate_manifest_registry(&manifest).is_ok(), expected);
    }

    #[test]
    /// Prove formatted calendar dates satisfy the ISO date-shape check.
    fn iso_date_accepts_generated_yyyy_mm_dd(
        year in 0u32..=9999,
        month in 1u32..=12,
        day in 1u32..=28,
    ) {
        let date = format!("{year:04}-{month:02}-{day:02}");

        prop_assert!(is_iso_date(&date));
    }

    #[test]
    /// Prove non-ten-character strings cannot satisfy the ISO date-shape check.
    fn iso_date_rejects_invalid_lengths(value in invalid_length_date_text()) {
        prop_assert!(!is_iso_date(&value));
    }

    #[test]
    /// Prove malformed separators and non-digits are rejected.
    fn iso_date_rejects_invalid_formats(value in invalid_format_date_text()) {
        prop_assert!(!is_iso_date(&value));
    }

    #[test]
    /// Prove state/evidence validation accepts exactly the required field set.
    fn state_evidence_constraints_match_boundary_state(
        state in boundary_state(),
        evidence_bits in 0u8..16,
        fields in task_fields(),
    ) {
        let flags = EvidenceFlags(evidence_bits);
        let task = task_with_evidence(state, Evidence::from_flags(flags), fields);
        let expected = expected_state_evidence(state, flags);

        prop_assert_eq!(validate_state_evidence(&task).is_ok(), expected);
    }

    #[test]
    /// Prove divergent rows are accepted exactly when their ADR anchor exists.
    fn adr_007_anchor_validation_matches_generated_anchor_sets(
        anchor in anchor_text(),
        is_present in any::<bool>(),
    ) {
        let mut anchors = BTreeSet::new();
        if is_present {
            anchors.insert(anchor.clone());
        } else {
            anchors.insert(format!("{anchor}-other"));
        }
        let task = task_with_evidence(
            BoundaryState::Divergent,
            Evidence {
                shipped_in: None,
                removal_gate: None,
                adr_anchor: Some(anchor),
                next_review_by: None,
            },
            TaskFields::representative(),
        );

        prop_assert_eq!(
            validate_divergent_adr_anchor(&task, &anchors).is_ok(),
            is_present
        );
    }

    #[test]
    /// Prove generated Markdown heading anchors stay GitHub-safe.
    fn markdown_heading_anchors_have_stable_slug_shape(heading in markdown_heading_text()) {
        let anchor = markdown_anchor(&heading);

        prop_assert!(!anchor.is_empty());
        prop_assert!(!anchor.starts_with('-'));
        prop_assert!(!anchor.ends_with('-'));
        prop_assert!(!anchor.contains("--"));
        prop_assert!(anchor
            .chars()
            .all(|char| char.is_ascii_lowercase() || char.is_ascii_digit() || char == '-'));
    }
}

/// Generate roadmap-like task IDs with enough variation to find registry drift.
fn task_id_text() -> impl Strategy<Value = String> { "[0-9]{1,3}(\\.[0-9]{1,3}){1,3}" }

/// Generate valid ADR/GitHub anchor slugs.
fn anchor_text() -> impl Strategy<Value = String> { "[a-z0-9]{1,16}(-[a-z0-9]{1,16}){0,4}" }

/// Generate headings that always yield a non-empty Markdown anchor.
fn markdown_heading_text() -> impl Strategy<Value = String> {
    "[A-Za-z0-9][A-Za-z0-9 ._|!:-]{0,64}"
}

/// Generate arbitrary non-evidence fields for state/evidence rows.
fn task_fields() -> impl Strategy<Value = TaskFields> {
    (
        task_id_text(),
        "[A-Za-z0-9 .|\\n]{0,64}",
        "[A-Za-z0-9 .|\\n]{0,64}",
        iso_date_text(),
    )
        .prop_map(|(id, gist, upstream_task, last_reviewed)| TaskFields {
            id,
            gist,
            upstream_task,
            last_reviewed,
        })
}

/// Generate formatted calendar dates for manifest row metadata.
fn iso_date_text() -> impl Strategy<Value = String> {
    (0u32..=9999, 1u32..=12, 1u32..=28)
        .prop_map(|(year, month, day)| format!("{year:04}-{month:02}-{day:02}"))
}

/// Build a manifest from generated registry IDs and row IDs.
fn manifest_with_ids(managed_tasks: Vec<String>, row_ids: Vec<String>) -> BoundaryManifest {
    BoundaryManifest {
        schema_version: 1,
        managed_tasks,
        tasks: row_ids
            .into_iter()
            .map(|id| {
                let mut fields = TaskFields::representative();
                fields.id = id;
                task_with_evidence(
                    BoundaryState::Consumes,
                    Evidence {
                        shipped_in: Some("4339a6f3".into()),
                        removal_gate: None,
                        adr_anchor: None,
                        next_review_by: None,
                    },
                    fields,
                )
            })
            .collect(),
    }
}

/// Return whether generated IDs contain no duplicates.
fn ids_are_unique(ids: &[String]) -> bool {
    let mut seen = BTreeSet::new();
    ids.iter().all(|id| seen.insert(id))
}

/// Generate strings whose length alone rules out an ISO `YYYY-MM-DD` shape.
fn invalid_length_date_text() -> impl Strategy<Value = String> {
    prop_oneof!["[0-9-]{0,9}", "[0-9-]{11,20}",]
}

/// Generate ten-character strings that violate separator or digit positions.
fn invalid_format_date_text() -> impl Strategy<Value = String> {
    prop_oneof![
        (0u32..=9999, 1u32..=12, 1u32..=28)
            .prop_map(|(year, month, day)| format!("{year:04}/{month:02}-{day:02}")),
        (0u32..=9999, 1u32..=12, 1u32..=28)
            .prop_map(|(year, month, day)| format!("{year:04}-{month:02}/{day:02}")),
        (0u32..=9999, 1u32..=12, 1u32..=28)
            .prop_map(|(year, month, day)| format!("A{year:03}-{month:02}-{day:02}")),
    ]
}

/// Generate each boundary state with equal intent.
fn boundary_state() -> impl Strategy<Value = BoundaryState> {
    prop_oneof![
        Just(BoundaryState::Consumes),
        Just(BoundaryState::Wraps),
        Just(BoundaryState::Pending),
        Just(BoundaryState::Divergent),
    ]
}

/// Build one task with generated fields and state-specific evidence.
fn task_with_evidence(
    state: BoundaryState,
    evidence: Evidence,
    fields: TaskFields,
) -> BoundaryTask {
    BoundaryTask {
        id: fields.id,
        gist: fields.gist,
        state,
        upstream: vec![UpstreamRef {
            task: fields.upstream_task,
            role: UpstreamRole::Renderer,
        }],
        shipped_in: evidence.shipped_in,
        removal_gate: evidence.removal_gate,
        adr_anchor: evidence.adr_anchor,
        next_review_by: evidence.next_review_by,
        last_reviewed: fields.last_reviewed,
    }
}

/// Return whether the generated evidence set is valid for a state.
const fn expected_state_evidence(state: BoundaryState, flags: EvidenceFlags) -> bool {
    let required = match state {
        BoundaryState::Consumes => SHIPPED_IN,
        BoundaryState::Wraps => REMOVAL_GATE,
        BoundaryState::Pending => NEXT_REVIEW_BY,
        BoundaryState::Divergent => ADR_ANCHOR,
    };
    flags.0 == required
}
