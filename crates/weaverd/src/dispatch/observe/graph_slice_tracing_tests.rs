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
    support::{SourceRequest, backends_fixture, detail_value, dispatch_payload, prepare_request},
};
use crate::{
    dispatch::{
        observe::test_support::{
            StubLanguageServer,
            markdown_hover,
            semantic_backends_with_server,
        },
        request::CommandRequest,
        router::DISPATCH_TARGET,
        trace_capture::{CapturedEvent, capture_events},
    },
    test_helpers::{ensure_same, ensure_same_ref, ensure_true},
};

// The message text is the operator-facing part of the contract, so these tests
// pin it literally rather than importing it from the instrumentation module. A
// reworded event then fails a test instead of silently changing the contract.
/// Message attached to the handler-entry event.
const REQUEST_MESSAGE: &str = "handling graph-slice request";
/// Message attached to the discovery-cap event.
const DISCOVERY_CAP_MESSAGE: &str = "same-file discovery capped at maximum positions";
/// Message attached to the budget-truncation event.
const BUDGET_MESSAGE: &str = "budget truncation produced spillover frontier";
/// Message attached to the enrichment-skip event.
const ENRICHMENT_SKIP_MESSAGE: &str = "enrichment skipped due to detail level";
/// Message attached to the enrichment-completion event.
const ENRICHMENT_DONE_MESSAGE: &str = "card enrichment completed";

/// Returns the captured `message` field, which the recording layer stores as
/// the rendered format string.
fn message_of(event: &CapturedEvent) -> Option<&str> {
    event.fields.get("message").map(String::as_str)
}

/// Returns the dispatch-target events carrying `message`.
fn events_with_message<'a>(events: &'a [CapturedEvent], message: &str) -> Vec<&'a CapturedEvent> {
    events
        .iter()
        .filter(|event| event.target == DISPATCH_TARGET && message_of(event) == Some(message))
        .collect()
}

/// Returns the single dispatch event carrying `message`.
fn sole_event<'a>(events: &'a [CapturedEvent], message: &str) -> Result<&'a CapturedEvent, String> {
    let matches = events_with_message(events, message);
    ensure_same(matches.len(), 1, message)?;
    matches
        .first()
        .copied()
        .ok_or_else(|| format!("expected exactly one {message} event"))
}

/// Returns the value of `field` on `event`, reporting a missing field.
fn field_of<'a>(event: &'a CapturedEvent, field: &str) -> Result<&'a str, String> {
    event.fields.get(field).map(String::as_str).ok_or_else(|| {
        format!(
            "event `{}` should carry `{field}`",
            message_of(event).unwrap_or("")
        )
    })
}

/// Runs `action`, returning its value with the tracing events it emitted.
fn with_captured_events<T>(action: impl FnOnce() -> T) -> (T, Vec<CapturedEvent>) {
    let mut slot = None;
    let events = capture_events(|| slot = Some(action()));
    let value = slot.expect("capture action should run exactly once");
    (value, events)
}

/// Renders `count` distinct single-symbol functions as Rust source.
fn render_functions(count: usize) -> String {
    (0..count).fold(String::new(), |mut source, index| {
        source.push_str("fn item_");
        source.push_str(&index.to_string());
        source.push_str("() {}\n");
        source
    })
}

/// Builds a request against a fixture file, delegating to the shared support
/// helper so these tests use the same request shape as the behaviour tests.
fn request_for(
    temp_dir: &TempDir,
    filename: &str,
    content: &str,
    arguments: &[&str],
) -> Result<CommandRequest, String> {
    prepare_request(&SourceRequest {
        temp_dir,
        filename,
        content,
        arguments,
    })
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
    let cards = payload
        .get("cards")
        .and_then(Value::as_array)
        .ok_or_else(|| "cards should be an array".to_owned())?;
    ensure_same(cards.len(), 2, "emitted card count")?;
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
    let card = payload
        .get("cards")
        .and_then(Value::as_array)
        .and_then(|cards| cards.first())
        .ok_or_else(|| "payload should carry an entry card".to_owned())?;
    let expected_symbol_id = card
        .get("symbol")
        .and_then(|symbol| symbol.get("symbol_id"))
        .and_then(Value::as_str)
        .ok_or_else(|| "entry card should carry a symbol_id".to_owned())?;
    ensure_same_ref(
        field_of(skip_event, "symbol_id")?,
        expected_symbol_id,
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
    ensure_true(
        !field_of(done_event, "symbol_id")?.is_empty(),
        "completion event to carry a symbol id",
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
