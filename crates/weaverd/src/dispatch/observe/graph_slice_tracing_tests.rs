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
    support::{backends_fixture, detail_value, dispatch_payload, render_functions},
    tracing_support::{
        BUDGET_MESSAGE,
        DISCOVERY_CAP_MESSAGE,
        ENRICHMENT_DONE_MESSAGE,
        ENRICHMENT_SKIP_MESSAGE,
        REQUEST_MESSAGE,
        events_with_message,
        field_of,
        request_for,
        sole_event,
        with_captured_events,
        with_captured_trace,
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
            "--entry-detail",
            detail_value(DetailLevel::Structure),
            "--node-detail",
            detail_value(DetailLevel::Semantic),
        ],
    )?;

    let (outcome, events) = with_captured_events(|| dispatch_payload(&request, &mut backends));
    let (status, payload) = outcome?;
    assert_success_response(status, &payload);

    let entry_event = sole_event(&events, REQUEST_MESSAGE)?;
    ensure_same(
        entry_event.level,
        tracing::Level::DEBUG,
        "entry event level",
    )?;
    ensure_true(
        field_of(entry_event, "uri")?.ends_with("entry.rs"),
        "entry event to carry the request URI",
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
    ensure_same_ref(
        field_of(entry_event, "max_cards")?,
        "30",
        "max_cards budget",
    )
}

#[rstest]
fn discovery_cap_logs_path_and_non_blank_line_count(
    backends_fixture: Result<(FusionBackends<SemanticBackendProvider>, TempDir), String>,
) -> Result<(), String> {
    let (mut backends, temp_dir) = backends_fixture?;
    let source = render_functions(MAX_SAME_FILE_DISCOVERY_POSITIONS + 1);
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

    let cap_event = sole_event(&events, DISCOVERY_CAP_MESSAGE)?;
    ensure_true(
        field_of(cap_event, "path")?.ends_with("large.rs"),
        "cap event to carry the file path",
    )?;
    let expected_lines = (MAX_SAME_FILE_DISCOVERY_POSITIONS + 1).to_string();
    ensure_same_ref(
        field_of(cap_event, "total_non_blank_lines")?,
        expected_lines.as_str(),
        "discovery input size",
    )?;
    let expected_cap = MAX_SAME_FILE_DISCOVERY_POSITIONS.to_string();
    ensure_same_ref(
        field_of(cap_event, "cap")?,
        expected_cap.as_str(),
        "discovery cap",
    )
}

#[rstest]
fn discovery_cap_event_is_absent_below_the_cap(
    backends_fixture: Result<(FusionBackends<SemanticBackendProvider>, TempDir), String>,
) -> Result<(), String> {
    let (mut backends, temp_dir) = backends_fixture?;
    let request = request_for(
        &temp_dir,
        "small.rs",
        "fn alpha() {}\n\nfn beta() {}\n",
        &["--position", "1:4"],
    )?;

    let (outcome, events) = with_captured_events(|| dispatch_payload(&request, &mut backends));
    let (status, payload) = outcome?;
    assert_success_response(status, &payload);

    ensure_same(
        events_with_message(&events, DISCOVERY_CAP_MESSAGE).len(),
        0,
        "discovery cap events",
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
        concat!(
            "struct Alpha(u32);\n\n",
            "impl Alpha {\n",
            "    fn first(&self) -> u32 { self.0 }\n",
            "    fn second(&self) -> u32 { self.0 + 1 }\n",
            "    fn third(&self) -> u32 { self.0 + 2 }\n",
            "}\n"
        ),
        &["--position", "4:8", "--max-cards", "2"],
    )?;

    let (outcome, events) = with_captured_events(|| dispatch_payload(&request, &mut backends));
    let (status, payload) = outcome?;
    assert_success_response(status, &payload);

    let truncation_event = sole_event(&events, BUDGET_MESSAGE)?;
    ensure_same_ref(
        field_of(truncation_event, "included_cards")?,
        "2",
        "included card count",
    )?;
    ensure_same(cards_of(&payload)?.len(), 2, "emitted card count")?;
    let frontier = payload
        .get("spillover")
        .and_then(|spillover| spillover.get("frontier"))
        .and_then(Value::as_array)
        .ok_or_else(|| "frontier should be an array".to_owned())?;
    let expected_frontier = frontier.len().to_string();
    ensure_same_ref(
        field_of(truncation_event, "frontier_candidates")?,
        expected_frontier.as_str(),
        "frontier candidate count",
    )
}

#[rstest]
fn budget_truncation_event_is_absent_without_spillover(
    backends_fixture: Result<(FusionBackends<SemanticBackendProvider>, TempDir), String>,
) -> Result<(), String> {
    let (mut backends, temp_dir) = backends_fixture?;
    let request = request_for(
        &temp_dir,
        "untruncated.rs",
        "fn solo() {}\n",
        &["--position", "1:4"],
    )?;

    let (outcome, events) = with_captured_events(|| dispatch_payload(&request, &mut backends));
    let (status, payload) = outcome?;
    assert_success_response(status, &payload);

    ensure_same(
        events_with_message(&events, BUDGET_MESSAGE).len(),
        0,
        "budget truncation events",
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

    let skip_event = sole_event(&events, ENRICHMENT_SKIP_MESSAGE)?;
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

    let done_event = sole_event(&events, ENRICHMENT_DONE_MESSAGE)?;
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

    let done_event = sole_event(&events, ENRICHMENT_DONE_MESSAGE)?;
    ensure_same_ref(
        field_of(done_event, "outcome")?,
        "Enriched",
        "enriched outcome",
    )
}

#[rstest]
fn request_span_carries_the_request_location(
    backends_fixture: Result<(FusionBackends<SemanticBackendProvider>, TempDir), String>,
) -> Result<(), String> {
    let (mut backends, temp_dir) = backends_fixture?;
    let request = request_for(
        &temp_dir,
        "spanned.rs",
        "struct Alpha(u32);\n\nimpl Alpha {\n    fn first(&self) -> u32 { self.0 }\n}\n",
        &["--position", "4:8"],
    )?;

    let (outcome, trace) = with_captured_trace(|| dispatch_payload(&request, &mut backends));
    let (status, payload) = outcome?;
    assert_success_response(status, &payload);

    // The span is what lets a reader attribute the count-only discovery-cap and
    // budget-truncation events to one concurrent request. `#[instrument]` names
    // the span after the function and targets the defining module, not
    // `DISPATCH_TARGET`, so both are asserted explicitly.
    let span = trace
        .spans
        .iter()
        .find(|span| span.name == "build_response")
        .ok_or_else(|| {
            let names = trace
                .spans
                .iter()
                .map(|span| span.name.as_str())
                .collect::<Vec<_>>();
            format!("expected a build_response span, captured {names:?}")
        })?;
    ensure_same_ref(
        span.target.as_str(),
        "weaverd::dispatch::observe::graph_slice",
        "request span target",
    )?;
    ensure_same(span.level, tracing::Level::DEBUG, "request span level")?;
    ensure_true(
        span.fields
            .get("uri")
            .is_some_and(|uri| uri.ends_with("spanned.rs")),
        "request span to carry the request URI",
    )?;
    ensure_same_ref(
        span.fields
            .get("line")
            .map(String::as_str)
            .unwrap_or_default(),
        "4",
        "request span line",
    )?;
    ensure_same_ref(
        span.fields
            .get("column")
            .map(String::as_str)
            .unwrap_or_default(),
        "8",
        "request span column",
    )
}
