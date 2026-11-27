//! Span assertions for the `observe graph-slice` handler.
//!
//! The discovery-cap and budget-truncation events are count-only. A
//! request-scoped span is what attributes them to one request when several
//! slices are handled concurrently, so this module proves the span exists,
//! carries the request location, and actually encloses the decision events.

use rstest::rstest;
use tempfile::TempDir;

use super::{
    FusionBackends,
    SemanticBackendProvider,
    assert_success_response,
    support::{backends_fixture, dispatch_payload},
    tracing_support::{REQUEST_EVENT, expected_uri, request_for, sole_event, with_captured_trace},
};
use crate::{
    dispatch::trace_capture::CapturedSpan,
    test_helpers::{ensure_same, ensure_same_ref},
};

/// Returns the value of `field` in a captured span, or an empty string.
fn span_field<'a>(span: &'a CapturedSpan, field: &str) -> &'a str {
    span.fields
        .get(field)
        .map(String::as_str)
        .unwrap_or_default()
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

    // `#[instrument]` names the span after the function and targets the
    // defining module, not `DISPATCH_TARGET`, so both are asserted explicitly.
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
    ensure_same_ref(
        span_field(span, "uri"),
        expected_uri(&temp_dir, "spanned.rs")?.as_str(),
        "request span URI",
    )?;
    ensure_same_ref(span_field(span, "line"), "4", "request span line")?;
    ensure_same_ref(span_field(span, "column"), "8", "request span column")?;

    // Creating a span is not enough: the decision events must sit inside it, or
    // the correlation promised to operators would be absent from real logs.
    let decision_event = sole_event(&trace.events, REQUEST_EVENT)?;
    ensure_same_ref(
        decision_event.span_path.as_slice(),
        [String::from("build_response")].as_slice(),
        "decision event span path",
    )
}
