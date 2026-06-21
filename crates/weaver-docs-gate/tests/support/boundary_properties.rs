//! Property tests for boundary-manifest validation helpers.
//!
//! These tests exercise the pure validation rules used by the integration
//! gate. They keep generated ISO date shapes and state/evidence combinations
//! separate from filesystem and Markdown rendering concerns.

use proptest::prelude::*;
use weaver_docs_gate::{BoundaryState, BoundaryTask, UpstreamRef, UpstreamRole};

use super::{is_iso_date, validate_state_evidence};

/// Generated state-specific evidence for one manifest row.
struct Evidence {
    shipped_in: Option<String>,
    removal_gate: Option<String>,
    adr_anchor: Option<String>,
    next_review_by: Option<String>,
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
    ) {
        let flags = EvidenceFlags(evidence_bits);
        let task = task_with_evidence(state, Evidence::from_flags(flags));
        let expected = expected_state_evidence(state, flags);

        prop_assert_eq!(validate_state_evidence(&task).is_ok(), expected);
    }
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

/// Build one task with generated state-specific evidence fields.
fn task_with_evidence(state: BoundaryState, evidence: Evidence) -> BoundaryTask {
    BoundaryTask {
        id: "12.1.1".into(),
        gist: "Track the downstream consumer boundary.".into(),
        state,
        upstream: vec![UpstreamRef {
            task: "renderer-contract".into(),
            role: UpstreamRole::Renderer,
        }],
        shipped_in: evidence.shipped_in,
        removal_gate: evidence.removal_gate,
        adr_anchor: evidence.adr_anchor,
        next_review_by: evidence.next_review_by,
        last_reviewed: "2026-06-20".into(),
    }
}

/// Return whether the generated evidence set is valid for a state.
const fn expected_state_evidence(state: BoundaryState, flags: EvidenceFlags) -> bool {
    match state {
        BoundaryState::Consumes => {
            flags.has(SHIPPED_IN)
                && !flags.has(REMOVAL_GATE)
                && !flags.has(ADR_ANCHOR)
                && !flags.has(NEXT_REVIEW_BY)
        }
        BoundaryState::Wraps => {
            flags.has(REMOVAL_GATE)
                && !flags.has(SHIPPED_IN)
                && !flags.has(ADR_ANCHOR)
                && !flags.has(NEXT_REVIEW_BY)
        }
        BoundaryState::Pending => {
            flags.has(NEXT_REVIEW_BY)
                && !flags.has(SHIPPED_IN)
                && !flags.has(REMOVAL_GATE)
                && !flags.has(ADR_ANCHOR)
        }
        BoundaryState::Divergent => {
            flags.has(ADR_ANCHOR)
                && !flags.has(SHIPPED_IN)
                && !flags.has(REMOVAL_GATE)
                && !flags.has(NEXT_REVIEW_BY)
        }
    }
}
