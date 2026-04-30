//! Unit tests for the `observe graph-slice` dispatch handler.

use std::{fs, path::PathBuf};

use rstest::rstest;
use tempfile::TempDir;
use url::Url;
use weaver_cards::{DEFAULT_CACHE_CAPACITY, DetailLevel};
use weaver_config::{CapabilityMatrix, Config, SocketEndpoint};

use super::{MAX_SAME_FILE_DISCOVERY_POSITIONS, first_non_whitespace_column, handle};
use crate::{
    backends::FusionBackends,
    dispatch::{errors::DispatchError, request::CommandRequest, response::ResponseWriter},
    semantic_provider::SemanticBackendProvider,
};

fn make_backends() -> (FusionBackends<SemanticBackendProvider>, TempDir) {
    let dir = match TempDir::new() {
        Ok(dir) => dir,
        Err(error) => panic!("create temp dir: {error}"),
    };
    let socket_path = dir
        .path()
        .join("socket.sock")
        .to_string_lossy()
        .into_owned();
    let config = Config {
        daemon_socket: SocketEndpoint::unix(socket_path),
        ..Config::default()
    };
    let provider =
        SemanticBackendProvider::new(CapabilityMatrix::default(), DEFAULT_CACHE_CAPACITY);
    (FusionBackends::new(config, provider), dir)
}

fn write_source(temp_dir: &TempDir, name: &str, content: &str) -> PathBuf {
    let path = temp_dir.path().join(name);
    if let Err(error) = fs::write(&path, content) {
        panic!("write source: {error}");
    }
    path
}

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

fn response_payload(output: Vec<u8>) -> serde_json::Value {
    let response = match String::from_utf8(output) {
        Ok(response) => response,
        Err(error) => panic!("utf8: {error}"),
    };
    let Some(stream_line) = response.lines().next() else {
        panic!("stream line");
    };
    let envelope: serde_json::Value = match serde_json::from_str(stream_line) {
        Ok(envelope) => envelope,
        Err(error) => panic!("envelope: {error}"),
    };
    let Some(data) = envelope.get("data").and_then(|value| value.as_str()) else {
        panic!("stdout data");
    };
    match serde_json::from_str(data) {
        Ok(payload) => payload,
        Err(error) => panic!("payload: {error}"),
    }
}

fn detail_value(detail: DetailLevel) -> &'static str {
    match detail {
        DetailLevel::Minimal => "minimal",
        DetailLevel::Signature => "signature",
        DetailLevel::Structure => "structure",
        DetailLevel::Semantic => "semantic",
        DetailLevel::Full => "full",
        _ => "full",
    }
}

fn dispatch_payload(
    request: &CommandRequest,
    backends: &mut FusionBackends<SemanticBackendProvider>,
) -> (i32, serde_json::Value) {
    let mut output = Vec::new();
    let mut writer = ResponseWriter::new(&mut output);
    let result = match handle(request, &mut writer, backends) {
        Ok(result) => result,
        Err(error) => panic!("dispatch fails: {error}"),
    };
    (result.status, response_payload(output))
}

fn assert_success_response(status: i32, payload: &serde_json::Value) {
    assert_eq!(status, 0);
    assert_eq!(payload["status"], "success");
}

fn assert_default_graph_slice_shape(payload: &serde_json::Value) {
    assert_eq!(payload["constraints"]["direction"], "both");
    assert_eq!(
        payload["constraints"]["budget"]["max_cards"],
        serde_json::json!(30)
    );
    assert_eq!(payload["edges"], serde_json::json!([]));
}

fn assert_spillover_truncated_with_frontier(payload: &serde_json::Value) {
    assert_eq!(payload["spillover"]["truncated"], true);
    let frontier = match payload["spillover"]["frontier"].as_array() {
        Some(frontier) => frontier,
        None => panic!("frontier array"),
    };
    assert!(!frontier.is_empty(), "expected non-empty frontier");
}

fn assert_refusal(status: i32, payload: &serde_json::Value, reason: &str) {
    assert_eq!(status, 1);
    assert_eq!(payload["status"], "refusal");
    assert_eq!(payload["refusal"]["reason"], reason);
}

fn assert_refusal_with_message(
    status: i32,
    payload: &serde_json::Value,
    reason: &str,
    message: &str,
) {
    assert_refusal(status, payload, reason);
    assert_eq!(payload["refusal"]["message"], message);
}

#[rstest]
fn valid_request_returns_success_and_echoed_constraints() {
    let (mut backends, temp_dir) = make_backends();
    let path = write_source(
        &temp_dir,
        "slice.rs",
        concat!(
            "struct Counter(u32);\n\n",
            "impl Counter {\n",
            "    fn increment(&mut self) {\n",
            "        self.0 += 1;\n",
            "    }\n",
            "}\n"
        ),
    );
    let uri = Url::from_file_path(&path).expect("file uri").to_string();
    let request = make_request(&[
        "--uri",
        &uri,
        "--position",
        "4:8",
        "--entry-detail",
        detail_value(DetailLevel::Structure),
        "--node-detail",
        detail_value(DetailLevel::Semantic),
    ]);

    let (status, payload) = dispatch_payload(&request, &mut backends);

    assert_success_response(status, &payload);
    assert_default_graph_slice_shape(&payload);
    assert_eq!(payload["spillover"]["truncated"], false);
    assert_eq!(payload["cards"][0]["symbol"]["ref"]["name"], "increment");
    assert!(payload["cards"].as_array().expect("cards array").len() >= 2);
}

#[rstest]
fn max_cards_budget_truncates_same_file_symbol_inventory() {
    let (mut backends, temp_dir) = make_backends();
    let path = write_source(
        &temp_dir,
        "slice.py",
        concat!(
            "class Factory:\n",
            "    @classmethod\n",
            "    def build(cls) -> \"Factory\":\n",
            "        return cls()\n\n",
            "    @staticmethod\n",
            "    def version() -> str:\n",
            "        return \"1.0\"\n"
        ),
    );
    let uri = Url::from_file_path(&path).expect("file uri").to_string();
    let request = make_request(&[
        "--uri",
        &uri,
        "--position",
        "3:9",
        "--max-cards",
        "1",
        "--entry-detail",
        detail_value(DetailLevel::Semantic),
        "--node-detail",
        detail_value(DetailLevel::Semantic),
    ]);

    let (status, payload) = dispatch_payload(&request, &mut backends);

    assert_success_response(status, &payload);
    assert_eq!(payload["cards"].as_array().expect("cards array").len(), 1);
    assert_spillover_truncated_with_frontier(&payload);
}

#[rstest]
fn discovery_cap_marks_spillover_truncated_when_card_budget_remains() {
    let (mut backends, temp_dir) = make_backends();
    let source = (0..=MAX_SAME_FILE_DISCOVERY_POSITIONS)
        .map(|index| format!("fn item_{index}() {{}}\n"))
        .collect::<String>();
    let path = write_source(&temp_dir, "large.rs", &source);
    let uri = Url::from_file_path(&path).expect("file uri").to_string();
    let request = make_request(&[
        "--uri",
        &uri,
        "--position",
        "1:4",
        "--max-cards",
        "300",
        "--entry-detail",
        detail_value(DetailLevel::Structure),
        "--node-detail",
        detail_value(DetailLevel::Structure),
    ]);

    let (status, payload) = dispatch_payload(&request, &mut backends);

    assert_success_response(status, &payload);
    assert_eq!(payload["spillover"]["truncated"], true);
    assert_eq!(
        payload["spillover"]["frontier"]
            .as_array()
            .expect("frontier array")
            .len(),
        0
    );
}

#[expect(
    clippy::too_many_arguments,
    reason = "review requested this exact helper signature for the refusal table"
)]
fn assert_structured_refusal(
    backends: &mut FusionBackends<SemanticBackendProvider>,
    temp_dir: &TempDir,
    filename: &str,
    content: &str,
    position: &str,
    expected_reason: &str,
    expected_message: Option<&str>,
) {
    let path = write_source(temp_dir, filename, content);
    let uri = match Url::from_file_path(&path) {
        Ok(uri) => uri.to_string(),
        Err(()) => panic!("file uri"),
    };
    let request = make_request(&["--uri", &uri, "--position", position]);

    let (status, payload) = dispatch_payload(&request, backends);

    match expected_message {
        Some(message) => assert_refusal_with_message(status, &payload, expected_reason, message),
        None => assert_refusal(status, &payload, expected_reason),
    }
}

#[rstest]
#[case(("notes.txt", "plain text\n", "1:1", "unsupported_language", None))]
#[case(
    (
        "main.rs",
        "fn main() {}\n \n",
        "2:1",
        "no_symbol_at_position",
        Some("observe graph-slice: no symbol found at 2:1"),
    )
)]
#[case(
    (
        "main.rs",
        "fn main() {}\n",
        "10:1",
        "position_out_of_range",
        Some("observe graph-slice: position 10:1 is outside the bounds of the file"),
    )
)]
fn structured_refusal_cases(#[case] case: (&str, &str, &str, &str, Option<&str>)) {
    let (mut backends, temp_dir) = make_backends();
    let (filename, content, position, expected_reason, expected_message) = case;
    assert_structured_refusal(
        &mut backends,
        &temp_dir,
        filename,
        content,
        position,
        expected_reason,
        expected_message,
    );
}

#[rstest]
#[case(&["--position", "10:5"], "missing required argument: --uri")]
#[case(
    &["--uri", "file:///src/main.rs", "--position", "bad"],
    "invalid argument value for --position"
)]
#[case(
    &["--uri", "https://example.com/main.rs", "--position", "1:1"],
    "expected a file URI"
)]
#[case(&["--uri", "file://%zz", "--position", "1:1"], "invalid URI")]
fn invalid_arguments_return_dispatch_error(
    #[case] arguments: &[&str],
    #[case] expected_substring: &str,
) {
    let (mut backends, _temp_dir) = make_backends();
    let request = make_request(arguments);
    let mut buffer = Vec::new();
    let mut writer = ResponseWriter::new(&mut buffer);
    let result = handle(&request, &mut writer, &mut backends);
    match result {
        Ok(_) => panic!("expected invalid arguments error, dispatch succeeded"),
        Err(error) => match error {
            DispatchError::InvalidArguments { message } => {
                assert!(
                    message.contains(expected_substring),
                    "expected invalid-arguments message to contain {expected_substring:?}, got: \
                     {message}"
                );
            }
            _ => panic!("expected invalid arguments error"),
        },
    }
}

#[rstest]
fn missing_source_file_returns_invalid_arguments() {
    let (mut backends, temp_dir) = make_backends();
    let path = temp_dir.path().join("missing.rs");
    let uri = Url::from_file_path(&path).expect("file uri").to_string();
    let request = make_request(&["--uri", &uri, "--position", "1:1"]);
    let mut buffer = Vec::new();
    let mut writer = ResponseWriter::new(&mut buffer);
    match handle(&request, &mut writer, &mut backends) {
        Ok(_) => panic!("expected invalid arguments error, dispatch succeeded"),
        Err(error) => match error {
            DispatchError::InvalidArguments { message } => {
                assert!(message.contains("unable to read source file"));
                assert!(message.contains("missing.rs"));
            }
            _ => panic!("expected invalid arguments error"),
        },
    }
}

#[test]
fn first_non_whitespace_column_uses_character_offsets() {
    assert_eq!(
        first_non_whitespace_column("\u{2003}\u{2003}fn main() {}"),
        Some(3)
    );
}
