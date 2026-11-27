//! Tracing assertions for the `observe graph-slice` decision boundaries.
//!
//! The handler emits `debug` events when a request arrives, when same-file
//! discovery hits its cap, when the card budget truncates, and when each card
//! finishes enrichment. These tests pin the emitted field set so an operator
//! can rely on those events when diagnosing a production slice.

use rstest::rstest;
use serde_json::Value;
use tempfile::TempDir;
use weaver_cards::DetailLevel;
use weaver_lsp_host::{Language, ServerCapabilitySet};

use super::{
    FusionBackends,
    MAX_SAME_FILE_DISCOVERY_POSITIONS,
    SemanticBackendProvider,
    assert_success_response,
    support::{backends_fixture, detail_value, dispatch_payload, render_padded_functions},
    tracing_support::{
        BUDGET_EVENT,
        DISCOVERY_CAP_EVENT,
        DecisionEvent,
        ENRICHMENT_DONE_EVENT,
        ENRICHMENT_SKIP_EVENT,
        REQUEST_EVENT,
        events_matching,
        expected_path,
        expected_uri,
        field_of,
        request_for,
        sole_event,
        with_captured_events,
    },
};
use crate::{
    dispatch::observe::test_support::{
        StubLanguageServer,
        markdown_hover,
        semantic_backends_with_server,
    },
    test_helpers::{ensure_same, ensure_same_ref, ensure_true},
};

/// Reads the `cards` array from a success payload.
fn cards_of(payload: &Value) -> Result<&Vec<Value>, String> {
    payload
        .get("cards")
        .and_then(Value::as_array)
        .ok_or_else(|| "cards should be an array".to_owned())
}

/// Reads the `symbol.symbol_id` of the first card in a success payload.
fn entry_symbol_id(payload: &Value) -> Result<&str, String> {
    cards_of(payload)?
        .first()
        .and_then(|card| card.get("symbol"))
        .and_then(|symbol| symbol.get("symbol_id"))
        .and_then(Value::as_str)
        .ok_or_else(|| "entry card should carry a symbol_id".to_owned())
}

/// Blank lines interleaved with each rendered function in the discovery-cap
/// fixture, so its total line count differs from its non-blank count.
const BLANK_LINES_PER_FUNCTION: usize = 2;

/// Cards the truncation fixture keeps: the entry card plus one sibling.
const EXPECTED_INCLUDED: usize = 2;

/// Candidates the truncation fixture leaves out.
///
/// Deliberately different from [`EXPECTED_INCLUDED`], so a test cannot pass by
/// reading one count where the other belongs.
const EXPECTED_FRONTIER: usize = 5;

#[rstest]
fn handler_entry_logs_request_fields(
    backends_fixture: Result<(FusionBackends<SemanticBackendProvider>, TempDir), String>,
) -> Result<(), String> {
    let (mut backends, temp_dir) = backends_fixture?;
    let request = request_for(
        &temp_dir,
        "entry.rs",
        "struct Counter(u32);\n\nimpl Counter {\n    fn increment(&mut self) {}\n}\n",
        &[
            "--position",
            "4:8",
            // A non-default budget: asserting the default would pass even if
            // the event logged a hard-coded constant.
            "--max-cards",
            "7",
            "--entry-detail",
            detail_value(DetailLevel::Structure),
            "--node-detail",
            detail_value(DetailLevel::Semantic),
        ],
    )?;

    let (outcome, events) = with_captured_events(|| dispatch_payload(&request, &mut backends));
    let (status, payload) = outcome?;
    assert_success_response(status, &payload);

    let entry_event = sole_event(&events, REQUEST_EVENT)?;
    ensure_same(
        entry_event.level,
        tracing::Level::DEBUG,
        "entry event level",
    )?;
    ensure_same_ref(
        field_of(entry_event, "uri")?,
        expected_uri(&temp_dir, "entry.rs")?.as_str(),
        "entry event request URI",
    )?;
    ensure_same_ref(field_of(entry_event, "line")?, "4", "entry line")?;
    ensure_same_ref(field_of(entry_event, "column")?, "8", "entry column")?;
    ensure_same_ref(
        field_of(entry_event, "entry_detail")?,
        "Structure",
        "entry detail",
    )?;
    ensure_same_ref(
        field_of(entry_event, "node_detail")?,
        "Semantic",
        "node detail",
    )?;
    ensure_same_ref(field_of(entry_event, "max_cards")?, "7", "max_cards budget")
}

#[rstest]
fn discovery_cap_logs_path_and_non_blank_line_count(
    backends_fixture: Result<(FusionBackends<SemanticBackendProvider>, TempDir), String>,
) -> Result<(), String> {
    let (mut backends, temp_dir) = backends_fixture?;
    let non_blank_lines = MAX_SAME_FILE_DISCOVERY_POSITIONS + 1;
    let source = render_padded_functions(non_blank_lines, BLANK_LINES_PER_FUNCTION);
    let request = request_for(
        &temp_dir,
        "large.rs",
        &source,
        &[
            "--position",
            "1:4",
            "--max-cards",
            "300",
            "--entry-detail",
            detail_value(DetailLevel::Structure),
            "--node-detail",
            detail_value(DetailLevel::Structure),
        ],
    )?;

    let (outcome, events) = with_captured_events(|| dispatch_payload(&request, &mut backends));
    let (status, payload) = outcome?;
    assert_success_response(status, &payload);

    let cap_event = sole_event(&events, DISCOVERY_CAP_EVENT)?;
    ensure_same_ref(
        field_of(cap_event, "path")?,
        expected_path(&temp_dir, "large.rs").as_str(),
        "cap event file path",
    )?;
    // The fixture interleaves blank lines, so the file's total line count and
    // its non-blank count differ. Asserting both catches an implementation that
    // reports either one for the other.
    let expected_non_blank = non_blank_lines.to_string();
    ensure_same_ref(
        field_of(cap_event, "total_non_blank_lines")?,
        expected_non_blank.as_str(),
        "non-blank line count",
    )?;
    let expected_total = (non_blank_lines * (1 + BLANK_LINES_PER_FUNCTION)).to_string();
    ensure_same_ref(
        field_of(cap_event, "total_lines")?,
        expected_total.as_str(),
        "total line count",
    )?;
    ensure_true(
        expected_total != expected_non_blank,
        "fixture must distinguish total from non-blank line counts",
    )?;
    let expected_cap = MAX_SAME_FILE_DISCOVERY_POSITIONS.to_string();
    ensure_same_ref(
        field_of(cap_event, "cap")?,
        expected_cap.as_str(),
        "discovery cap",
    )
}

#[rstest]
fn budget_truncation_logs_included_and_frontier_counts(
    backends_fixture: Result<(FusionBackends<SemanticBackendProvider>, TempDir), String>,
) -> Result<(), String> {
    let (mut backends, temp_dir) = backends_fixture?;
    let request = request_for(
        &temp_dir,
        "truncate.rs",
        // Seven symbols so the included and frontier counts differ: two cards
        // fit the budget, leaving five candidates out.
        concat!(
            "struct Alpha(u32);\n\n",
            "impl Alpha {\n",
            "    fn first(&self) -> u32 { self.0 }\n",
            "    fn second(&self) -> u32 { self.0 + 1 }\n",
            "    fn third(&self) -> u32 { self.0 + 2 }\n",
            "    fn fourth(&self) -> u32 { self.0 + 3 }\n",
            "    fn fifth(&self) -> u32 { self.0 + 4 }\n",
            "    fn sixth(&self) -> u32 { self.0 + 5 }\n",
            "}\n"
        ),
        &["--position", "4:8", "--max-cards", "2"],
    )?;

    let (outcome, events) = with_captured_events(|| dispatch_payload(&request, &mut backends));
    let (status, payload) = outcome?;
    assert_success_response(status, &payload);

    let truncation_event = sole_event(&events, BUDGET_EVENT)?;
    ensure_same_ref(
        field_of(truncation_event, "included_cards")?,
        EXPECTED_INCLUDED.to_string().as_str(),
        "included card count",
    )?;
    ensure_same(
        cards_of(&payload)?.len(),
        EXPECTED_INCLUDED,
        "emitted card count",
    )?;
    // Deriving the expectation from the response would accept an event that
    // reported `included_cards` where `frontier_candidates` belongs.
    ensure_true(
        EXPECTED_FRONTIER != EXPECTED_INCLUDED,
        "fixture must not make the two counts indistinguishable",
    )?;
    ensure_same_ref(
        field_of(truncation_event, "frontier_candidates")?,
        EXPECTED_FRONTIER.to_string().as_str(),
        "frontier candidate count",
    )
}

/// A file that fits both the discovery cap and the card budget.
///
/// The two conditional events are the only instrumentation that would explain
/// missing cards, so their absence is the contract for a complete slice.
#[rstest]
#[case::discovery_cap(DISCOVERY_CAP_EVENT, "discovery cap events")]
#[case::budget_truncation(BUDGET_EVENT, "budget truncation events")]
fn complete_slice_emits_no_truncation_event(
    backends_fixture: Result<(FusionBackends<SemanticBackendProvider>, TempDir), String>,
    #[case] expected: DecisionEvent,
    #[case] what: &str,
) -> Result<(), String> {
    let (mut backends, temp_dir) = backends_fixture?;
    let request = request_for(
        &temp_dir,
        "complete.rs",
        "fn alpha() {}\n\nfn beta() {}\n",
        &["--position", "1:4"],
    )?;

    let (outcome, events) = with_captured_events(|| dispatch_payload(&request, &mut backends));
    let (status, payload) = outcome?;
    assert_success_response(status, &payload);

    ensure_same(events_matching(&events, expected).len(), 0, what)
}

#[rstest]
fn enrichment_skip_event_is_absent_at_semantic_detail(
    backends_fixture: Result<(FusionBackends<SemanticBackendProvider>, TempDir), String>,
) -> Result<(), String> {
    let (mut backends, temp_dir) = backends_fixture?;
    let request = request_for(
        &temp_dir,
        "semantic.rs",
        "fn solo() {}\n",
        &[
            "--position",
            "1:4",
            "--entry-detail",
            detail_value(DetailLevel::Semantic),
            "--node-detail",
            detail_value(DetailLevel::Semantic),
        ],
    )?;

    let (outcome, events) = with_captured_events(|| dispatch_payload(&request, &mut backends));
    let (status, payload) = outcome?;
    assert_success_response(status, &payload);

    // Semantic detail must attempt enrichment, so an implementation that logged
    // the skip event unconditionally would fail here. Exactly one completion
    // event proves the alternative path actually ran, rather than neither
    // event firing.
    ensure_same(
        events_matching(&events, ENRICHMENT_SKIP_EVENT).len(),
        0,
        "enrichment skip events",
    )?;
    ensure_same(
        events_matching(&events, ENRICHMENT_DONE_EVENT).len(),
        1,
        "enrichment completion events",
    )?;
    ensure_same_ref(
        field_of(sole_event(&events, ENRICHMENT_DONE_EVENT)?, "outcome")?,
        "Degraded",
        "semantic-detail outcome without an LSP server",
    )
}

#[rstest]
fn enrichment_skip_logs_symbol_id_and_outcome(
    backends_fixture: Result<(FusionBackends<SemanticBackendProvider>, TempDir), String>,
) -> Result<(), String> {
    let (mut backends, temp_dir) = backends_fixture?;
    let request = request_for(
        &temp_dir,
        "skip.rs",
        "fn solo() {}\n",
        &["--position", "1:4"],
    )?;

    let (outcome, events) = with_captured_events(|| dispatch_payload(&request, &mut backends));
    let (status, payload) = outcome?;
    assert_success_response(status, &payload);

    let skip_event = sole_event(&events, ENRICHMENT_SKIP_EVENT)?;
    ensure_same_ref(
        field_of(skip_event, "symbol_id")?,
        entry_symbol_id(&payload)?,
        "skipped card symbol id",
    )?;
    ensure_same_ref(field_of(skip_event, "outcome")?, "skipped", "skip outcome")
}

#[rstest]
fn enrichment_completion_logs_degraded_outcome_without_lsp_server(
    backends_fixture: Result<(FusionBackends<SemanticBackendProvider>, TempDir), String>,
) -> Result<(), String> {
    let (mut backends, temp_dir) = backends_fixture?;
    let request = request_for(
        &temp_dir,
        "degraded.rs",
        "fn solo() {}\n",
        &[
            "--position",
            "1:4",
            "--entry-detail",
            detail_value(DetailLevel::Semantic),
        ],
    )?;

    let (outcome, events) = with_captured_events(|| dispatch_payload(&request, &mut backends));
    let (status, payload) = outcome?;
    assert_success_response(status, &payload);

    let done_event = sole_event(&events, ENRICHMENT_DONE_EVENT)?;
    ensure_same_ref(
        field_of(done_event, "outcome")?,
        "Degraded",
        "degraded outcome",
    )?;
    ensure_same_ref(
        field_of(done_event, "symbol_id")?,
        entry_symbol_id(&payload)?,
        "degraded card symbol id",
    )
}

#[rstest]
fn enrichment_completion_logs_enriched_outcome_with_lsp_server() -> Result<(), String> {
    let (server, _hover_params) = StubLanguageServer::with_hover(
        ServerCapabilitySet::new(false, false, false).with_hover(true),
        markdown_hover("```rust\nfn solo()\n```"),
    );
    let (mut backends, temp_dir) = semantic_backends_with_server(Language::Rust, server)?;
    let request = request_for(
        &temp_dir,
        "enriched.rs",
        "fn solo() {}\n",
        &[
            "--position",
            "1:4",
            "--entry-detail",
            detail_value(DetailLevel::Semantic),
        ],
    )?;

    let (outcome, events) = with_captured_events(|| dispatch_payload(&request, &mut backends));
    let (status, payload) = outcome?;
    assert_success_response(status, &payload);

    let done_event = sole_event(&events, ENRICHMENT_DONE_EVENT)?;
    ensure_same_ref(
        field_of(done_event, "outcome")?,
        "Enriched",
        "enriched outcome",
    )
}
