//! Assertion helpers for the `observe graph-slice` tracing tests.
//!
//! The handler writes its decision-boundary events to the dispatch tracing
//! target. These helpers turn a captured event list into the single event a
//! test expects, and read individual fields back as strings, so each test body
//! states the contract it checks rather than the mechanics of searching for it.
//!
//! The message text is the operator-facing part of that contract, so the
//! constants below are literals rather than imports from the instrumentation.
//! Rewording an event therefore fails a test instead of silently changing what
//! operators' log filters match.

use tempfile::TempDir;

use super::support::{SourceRequest, prepare_request};
use crate::dispatch::{
    request::CommandRequest,
    router::DISPATCH_TARGET,
    trace_capture::{CapturedEvent, CapturedTrace, capture_events, capture_trace},
};

/// One decision-boundary event the tests can name.
///
/// The message text *is* the operator-facing contract, so it is a domain value
/// rather than a bare string: a test selects an event by [`DecisionEvent`], and
/// the constant definitions below are the single place the text is written.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct DecisionEvent {
    message: &'static str,
}

impl DecisionEvent {
    /// Names the event whose `message` field is `message`.
    pub(crate) const fn new(message: &'static str) -> Self { Self { message } }

    /// Returns the message text this event is recorded under.
    pub(crate) const fn message(self) -> &'static str { self.message }

    /// Returns whether `event` is this event, emitted on the dispatch target.
    fn is_recorded_as(self, event: &CapturedEvent) -> bool {
        event.target == DISPATCH_TARGET && event_message(event) == Some(self.message)
    }
}

/// Message attached to the handler-entry event.
pub(crate) const REQUEST_EVENT: DecisionEvent = DecisionEvent::new("handling graph-slice request");
/// Message attached to the discovery-cap event.
pub(crate) const DISCOVERY_CAP_EVENT: DecisionEvent =
    DecisionEvent::new("same-file discovery capped at maximum positions");
/// Message attached to the budget-truncation event.
pub(crate) const BUDGET_EVENT: DecisionEvent =
    DecisionEvent::new("budget truncation produced spillover frontier");
/// Message attached to the enrichment-skip event.
pub(crate) const ENRICHMENT_SKIP_EVENT: DecisionEvent =
    DecisionEvent::new("enrichment skipped due to detail level");
/// Message attached to the enrichment-completion event.
pub(crate) const ENRICHMENT_DONE_EVENT: DecisionEvent =
    DecisionEvent::new("card enrichment completed");

/// Returns the captured `message` field, which the recording layer stores as
/// the rendered format string.
fn event_message(event: &CapturedEvent) -> Option<&str> {
    event.fields.get("message").map(String::as_str)
}

/// Returns the dispatch-target events recording `expected`.
pub(crate) fn events_matching<'a>(
    events: &'a [CapturedEvent],
    expected: DecisionEvent,
) -> Vec<&'a CapturedEvent> {
    events
        .iter()
        .filter(|event| expected.is_recorded_as(event))
        .collect()
}

/// Returns the single dispatch event recording `expected`.
///
/// Zero or several matches both fail, because every caller knows which single
/// boundary it expected to observe.
pub(crate) fn sole_event<'a>(
    events: &'a [CapturedEvent],
    expected: DecisionEvent,
) -> Result<&'a CapturedEvent, String> {
    let matches = events_matching(events, expected);
    if matches.len() != 1 {
        return Err(format!(
            "expected exactly one `{}` event, captured {}",
            expected.message(),
            matches.len()
        ));
    }
    matches
        .first()
        .copied()
        .ok_or_else(|| format!("expected exactly one `{}` event", expected.message()))
}

/// Returns the value of `field` on `event`, reporting a missing field.
pub(crate) fn field_of<'a>(event: &'a CapturedEvent, field: &str) -> Result<&'a str, String> {
    event.fields.get(field).map(String::as_str).ok_or_else(|| {
        format!(
            "event `{}` should carry `{field}`",
            event_message(event).unwrap_or("<unnamed>")
        )
    })
}

/// Runs `action`, returning its value alongside the tracing events it emitted.
///
/// The value is stashed rather than returned directly because the capture
/// closure must return `()`, which would otherwise discard a fallible dispatch
/// result the caller needs to inspect.
pub(crate) fn with_captured_events<T>(action: impl FnOnce() -> T) -> (T, Vec<CapturedEvent>) {
    let mut slot = None;
    let events = capture_events(|| slot = Some(action()));
    let value = slot.expect("capture action should run exactly once");
    (value, events)
}

/// Runs `action`, returning its value alongside every event and span created.
///
/// Use this when the assertion is about request-scoped instrumentation rather
/// than about one event; the span a request creates is visible here even
/// though it emits no event of its own.
pub(crate) fn with_captured_trace<T>(action: impl FnOnce() -> T) -> (T, CapturedTrace) {
    let mut slot = None;
    let trace = capture_trace(|| slot = Some(action()));
    let value = slot.expect("capture action should run exactly once");
    (value, trace)
}

/// Builds a request against a fixture file, using the shared support helper so
/// these tests exercise the same request shape as the behaviour tests.
pub(crate) fn request_for(
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
