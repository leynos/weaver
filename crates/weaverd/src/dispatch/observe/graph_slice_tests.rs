//! Unit tests for the `observe graph-slice` dispatch handler.

use rstest::rstest;

use super::handle;
use crate::dispatch::{request::CommandRequest, response::ResponseWriter};

fn make_request(arguments: &[&str]) -> CommandRequest {
    let args_json: Vec<String> = arguments
        .iter()
        .map(|s| format!("\"{}\"", s.replace('"', "\\\"")))
        .collect();
    let json = format!(
        concat!(
            "{{\"command\":{{\"domain\":\"observe\",",
            "\"operation\":\"graph-slice\"}},",
            "\"arguments\":[{}]}}"
        ),
        args_json.join(",")
    );
    match CommandRequest::parse(json.as_bytes()) {
        Ok(request) => request,
        Err(error) => panic!("request: {error}"),
    }
}

fn dispatch_graph_slice(arguments: &[&str]) -> (i32, String) {
    let request = make_request(arguments);
    let mut buffer = Vec::new();
    let mut writer = ResponseWriter::new(&mut buffer);
    let result = match handle(&request, &mut writer) {
        Ok(result) => result,
        Err(error) => panic!("dispatch succeeds: {error}"),
    };
    let output = match String::from_utf8(buffer) {
        Ok(output) => output,
        Err(error) => panic!("valid UTF-8: {error}"),
    };
    (result.status, output)
}

fn parse_stdout_data(line: &str) -> Option<String> {
    let envelope = serde_json::from_str::<serde_json::Value>(line).ok()?;
    if envelope.get("stream").and_then(|v| v.as_str()) != Some("stdout") {
        return None;
    }
    envelope
        .get("data")
        .and_then(|v| v.as_str())
        .map(String::from)
}

/// Extracts the stdout payload from the JSONL stream envelope.
///
/// The `ResponseWriter` wraps output in `{"stream":"stdout","data":"…"}`
/// envelopes. This helper extracts just the data content.
fn extract_stdout(raw: &str) -> String {
    raw.lines()
        .find_map(parse_stdout_data)
        .unwrap_or_else(|| String::from(raw))
}

#[test]
fn valid_request_returns_not_yet_implemented_refusal() {
    let (status, output) =
        dispatch_graph_slice(&["--uri", "file:///src/main.rs", "--position", "10:5"]);

    assert_eq!(status, 1);
    let parsed: serde_json::Value =
        serde_json::from_str(&extract_stdout(&output)).expect("valid JSON");
    assert_eq!(
        parsed.get("status").and_then(|v| v.as_str()),
        Some("refusal")
    );
    assert_eq!(
        parsed.pointer("/refusal/reason").and_then(|v| v.as_str()),
        Some("not_yet_implemented")
    );
}

#[rstest]
#[case(&["--position", "10:5"])]
#[case(&["--uri", "file:///src/main.rs", "--position", "bad"])]
fn invalid_arguments_return_dispatch_error(#[case] arguments: &[&str]) {
    let request = make_request(arguments);
    let mut buffer = Vec::new();
    let mut writer = ResponseWriter::new(&mut buffer);
    let result = handle(&request, &mut writer);
    assert!(result.is_err());
}
