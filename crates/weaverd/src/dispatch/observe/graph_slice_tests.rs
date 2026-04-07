//! Unit tests for the `observe graph-slice` dispatch handler.

use crate::dispatch::request::CommandRequest;
use crate::dispatch::response::ResponseWriter;

use super::handle;

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
    CommandRequest::parse(json.as_bytes()).expect("request")
}

fn dispatch_graph_slice(arguments: &[&str]) -> (i32, String) {
    let request = make_request(arguments);
    let mut buffer = Vec::new();
    let mut writer = ResponseWriter::new(&mut buffer);
    let result =
        handle(&request, &mut writer).expect("dispatch succeeds");
    let output = String::from_utf8(buffer).expect("valid UTF-8");
    (result.status, output)
}

/// Extracts the stdout payload from the JSONL stream envelope.
///
/// The `ResponseWriter` wraps output in `{"stream":"stdout","data":"…"}`
/// envelopes. This helper extracts just the data content.
fn extract_stdout(raw: &str) -> String {
    for line in raw.lines() {
        if let Ok(envelope) =
            serde_json::from_str::<serde_json::Value>(line)
        {
            if envelope.get("stream").and_then(|v| v.as_str())
                == Some("stdout")
            {
                if let Some(data) =
                    envelope.get("data").and_then(|v| v.as_str())
                {
                    return String::from(data);
                }
            }
        }
    }
    String::from(raw)
}

#[test]
fn valid_request_returns_not_yet_implemented_refusal() {
    let (status, output) = dispatch_graph_slice(&[
        "--uri",
        "file:///src/main.rs",
        "--position",
        "10:5",
    ]);

    assert_eq!(status, 1);
    let parsed: serde_json::Value =
        serde_json::from_str(&extract_stdout(&output))
            .expect("valid JSON");
    assert_eq!(
        parsed.get("status").and_then(|v| v.as_str()),
        Some("refusal")
    );
    assert_eq!(
        parsed
            .pointer("/refusal/reason")
            .and_then(|v| v.as_str()),
        Some("not_yet_implemented")
    );
}

#[test]
fn invalid_arguments_returns_dispatch_error() {
    let request = make_request(&["--position", "10:5"]);
    let mut buffer = Vec::new();
    let mut writer = ResponseWriter::new(&mut buffer);
    let result = handle(&request, &mut writer);
    assert!(result.is_err());
}

#[test]
fn bad_position_format_returns_dispatch_error() {
    let request = make_request(&[
        "--uri",
        "file:///src/main.rs",
        "--position",
        "bad",
    ]);
    let mut buffer = Vec::new();
    let mut writer = ResponseWriter::new(&mut buffer);
    let result = handle(&request, &mut writer);
    assert!(result.is_err());
}
