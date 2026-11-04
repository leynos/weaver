//! Unit tests for command dispatch and request handling.

use std::{
    io::Write,
    net::{TcpListener, TcpStream},
    thread,
};

use rstest::rstest;
use weaver_daemon_types::JSONL_REQUEST_MAX_LINE_BYTES;

#[path = "read_error_event_tests.rs"]
mod read_error_event_tests;
#[path = "receive_request_tests.rs"]
mod receive_request_tests;
#[path = "structured_event_tests.rs"]
mod structured_event_tests;
#[path = "tests_helpers.rs"]
mod tests_helpers;

use tests_helpers::{
    BackendManagerFixture,
    HandlerTestHarness,
    backend_manager,
    capture_events,
    ensure,
    harness,
};

use super::{structured_event::serialize_structured_event, *};
use crate::dispatch::{UNKNOWN_OPERATION_TYPE, parse_stderr_json_payload};

/// Reads a nested JSON field, returning `None` when any level is absent.
fn json_at<'a>(value: &'a serde_json::Value, path: &[&str]) -> Option<&'a serde_json::Value> {
    path.iter().try_fold(value, |node, key| node.get(*key))
}

#[rstest]
fn handler_responds_to_get_definition_without_args(
    harness: Result<HandlerTestHarness, String>,
) -> Result<(), String> {
    let mut test_harness = harness?;
    let lines = test_harness.send_and_collect(
        b"{\"command\":{\"domain\":\"observe\",\"operation\":\"get-definition\"}}\n",
    )?;

    // Should have error about missing arguments and exit message.
    ensure(
        lines.iter().any(|line| line.contains("invalid arguments")),
        format!("expected an invalid arguments error; got {lines:?}"),
    )?;
    ensure(
        lines.iter().any(|line| line.contains(r#""kind":"exit""#)),
        format!("expected an exit event; got {lines:?}"),
    )?;

    test_harness.join()?;
    Ok(())
}

#[rstest]
fn handler_rejects_malformed_json(
    harness: Result<HandlerTestHarness, String>,
) -> Result<(), String> {
    let mut test_harness = harness?;
    let lines = test_harness.send_and_collect(b"not valid json\n")?;

    // Should have error message.
    ensure(
        lines.iter().any(|line| line.contains("error:")),
        format!("expected an error message; got {lines:?}"),
    )?;
    ensure(
        lines.iter().any(|line| line.contains(r#""status":1"#)),
        format!("expected a failure status; got {lines:?}"),
    )?;

    test_harness.join()?;
    Ok(())
}

#[rstest]
fn handler_rejects_unknown_domain(
    harness: Result<HandlerTestHarness, String>,
) -> Result<(), String> {
    let mut test_harness = harness?;
    let lines = test_harness
        .send_and_collect(b"{\"command\":{\"domain\":\"bogus\",\"operation\":\"test\"}}\n")?;

    ensure(
        lines.iter().any(|line| line.contains("unknown domain")),
        format!("expected an unknown domain error; got {lines:?}"),
    )?;
    ensure(
        lines.iter().any(|line| line.contains(r#""status":1"#)),
        format!("expected a failure status; got {lines:?}"),
    )?;

    test_harness.join()?;
    Ok(())
}

#[rstest]
fn handler_responds_to_not_implemented_operation(
    harness: Result<HandlerTestHarness, String>,
) -> Result<(), String> {
    let mut test_harness = harness?;
    let lines = test_harness.send_and_collect(
        b"{\"command\":{\"domain\":\"observe\",\"operation\":\"find-references\"}}\n",
    )?;

    // find-references is not yet implemented.
    ensure(
        lines
            .iter()
            .any(|line| line.contains("not yet implemented")),
        format!("expected a not-yet-implemented error; got {lines:?}"),
    )?;
    ensure(
        lines.iter().any(|line| line.contains(r#""kind":"exit""#)),
        format!("expected an exit event; got {lines:?}"),
    )?;

    test_harness.join()?;
    Ok(())
}

#[rstest]
fn handler_emits_known_operations_for_unknown_operation(
    harness: Result<HandlerTestHarness, String>,
) -> Result<(), String> {
    let mut test_harness = harness?;
    let lines = test_harness
        .send_and_collect(b"{\"command\":{\"domain\":\"observe\",\"operation\":\"bogus\"}}\n")?;

    let payload = lines
        .iter()
        .find_map(|line| parse_stderr_json_payload::<serde_json::Value>(line))
        .expect("unknown-operation payload should be present");

    ensure(
        json_at(&payload, &["status"]).and_then(serde_json::Value::as_str) == Some("error"),
        format!("expected an error payload status; got {payload}"),
    )?;
    ensure(
        json_at(&payload, &["type"]).and_then(serde_json::Value::as_str)
            == Some(UNKNOWN_OPERATION_TYPE),
        format!("expected an unknown-operation payload type; got {payload}"),
    )?;
    ensure(
        json_at(&payload, &["details", "domain"]).and_then(serde_json::Value::as_str)
            == Some("observe"),
        format!("expected the request domain to be echoed; got {payload}"),
    )?;
    ensure(
        json_at(&payload, &["details", "operation"]).and_then(serde_json::Value::as_str)
            == Some("bogus"),
        format!("expected the request operation to be echoed; got {payload}"),
    )?;
    ensure(
        json_at(&payload, &["details", "known_operations"]).cloned()
            == Some(serde_json::json!([
                "get-definition",
                "find-references",
                "grep",
                "diagnostics",
                "call-hierarchy",
                "get-card",
                "graph-slice"
            ])),
        format!("expected the complete known-operation list; got {payload}"),
    )?;
    ensure(
        lines.iter().any(|line| line.contains(r#""status":1"#)),
        format!("expected a failure status; got {lines:?}"),
    )?;

    test_harness.join()?;
    Ok(())
}

#[rstest]
fn capture_events_records_server_thread_events(
    backend_manager: Result<BackendManagerFixture, String>,
) -> Result<(), String> {
    let mut outcome: Result<(), String> = Ok(());
    let events = capture_events(|| {
        outcome = (|| {
            let mut harness = tests_helpers::harness(backend_manager)?;
            harness.send_and_collect(
                b"{\"command\":{\"domain\":\"observe\",\"operation\":\"get-definition\"}}\n",
            )?;
            harness.join()
        })();
    });
    outcome?;

    ensure(
        events.iter().any(|event| {
            event.target == DISPATCH_TARGET
                && event
                    .fields
                    .get("event")
                    .is_some_and(|value| value == "dispatching_request")
        }),
        format!("expected a server-emitted dispatching_request event; captured: {events:?}"),
    )?;
    Ok(())
}

#[test]
fn request_too_large_serialization_maps_to_request_too_large_event() {
    let temp_dir = std::env::temp_dir();
    let socket_path = temp_dir.join("weaverd.sock");
    let endpoint = socket_path.to_string_lossy().into_owned();
    let event = read_error_event(
        &DispatchError::request_too_large(
            JSONL_REQUEST_MAX_LINE_BYTES + 1,
            JSONL_REQUEST_MAX_LINE_BYTES,
        ),
        &endpoint,
        &temp_dir,
    );
    let value = serialize_structured_event(&event);

    assert_eq!(
        value.get("event").and_then(serde_json::Value::as_str),
        Some("request_too_large")
    );
    assert_eq!(
        value.get("size"),
        Some(&serde_json::json!(JSONL_REQUEST_MAX_LINE_BYTES + 1))
    );
    assert_eq!(
        value.get("max_size"),
        Some(&serde_json::json!(JSONL_REQUEST_MAX_LINE_BYTES))
    );
}

#[test]
fn read_request_line_returns_request_too_large_for_large_payload() {
    let max_plus_one = JSONL_REQUEST_MAX_LINE_BYTES + 1;
    let listener = TcpListener::bind(("127.0.0.1", 0)).expect("bind listener");
    let addr = listener.local_addr().expect("listener addr");

    let payload = vec![b'a'; max_plus_one];
    let sender = thread::spawn(move || {
        let mut stream = TcpStream::connect(addr).expect("connect sender");
        stream.write_all(&payload).expect("write request");
        stream.flush().expect("flush sender");
    });

    let (stream, _) = listener.accept().expect("accept");
    let mut connection_stream = ConnectionStream::Tcp(stream);
    let error =
        read_request_line(&mut connection_stream).expect_err("expected request too large error");

    assert!(matches!(error, DispatchError::RequestTooLarge { .. }));
    assert_eq!(
        match error {
            DispatchError::RequestTooLarge { size, .. } => size,
            _ => 0,
        },
        max_plus_one
    );

    sender.join().expect("join sender");
}
