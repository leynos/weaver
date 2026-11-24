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
    trace_capture::{CapturedEvent, capture_events},
};

/// Message attached to the handler-entry event.
pub(crate) const REQUEST_MESSAGE: &str = "handling graph-slice request";
/// Message attached to the discovery-cap event.
pub(crate) const DISCOVERY_CAP_MESSAGE: &str = "same-file discovery capped at maximum positions";
/// Message attached to the budget-truncation event.
pub(crate) const BUDGET_MESSAGE: &str = "budget truncation produced spillover frontier";
/// Message attached to the enrichment-skip event.
pub(crate) const ENRICHMENT_SKIP_MESSAGE: &str = "enrichment skipped due to detail level";
/// Message attached to the enrichment-completion event.
pub(crate) const ENRICHMENT_DONE_MESSAGE: &str = "card enrichment completed";

/// Returns the captured `message` field, which the recording layer stores as
/// the rendered format string.
fn message_of(event: &CapturedEvent) -> Option<&str> {
    event.fields.get("message").map(String::as_str)
}

/// Returns the dispatch-target events carrying `message`.
pub(crate) fn events_with_message<'a>(
    events: &'a [CapturedEvent],
    message: &str,
) -> Vec<&'a CapturedEvent> {
    events
        .iter()
        .filter(|event| event.target == DISPATCH_TARGET && message_of(event) == Some(message))
        .collect()
}

/// Returns the single dispatch event carrying `message`.
///
/// Zero or several matches both fail, because every caller knows which single
/// boundary it expected to observe.
pub(crate) fn sole_event<'a>(
    events: &'a [CapturedEvent],
    message: &str,
) -> Result<&'a CapturedEvent, String> {
    let matches = events_with_message(events, message);
    if matches.len() != 1 {
        return Err(format!(
            "expected exactly one `{message}` event, captured {}",
            matches.len()
        ));
    }
    matches
        .first()
        .copied()
        .ok_or_else(|| format!("expected exactly one `{message}` event"))
}

/// Returns the value of `field` on `event`, reporting a missing field.
pub(crate) fn field_of<'a>(event: &'a CapturedEvent, field: &str) -> Result<&'a str, String> {
    event.fields.get(field).map(String::as_str).ok_or_else(|| {
        format!(
            "event `{}` should carry `{field}`",
            message_of(event).unwrap_or("<unnamed>")
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
