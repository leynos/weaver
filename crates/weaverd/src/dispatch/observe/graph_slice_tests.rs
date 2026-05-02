//! Unit tests for the `observe graph-slice` dispatch handler.
use std::{fs, path::PathBuf};

use rstest::{fixture, rstest};
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
#[fixture]
fn backends_fixture() -> Result<(FusionBackends<SemanticBackendProvider>, TempDir), String> {
    let dir = TempDir::new().map_err(|error| format!("create temp dir: {error}"))?;
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
    Ok((FusionBackends::new(config, provider), dir))
}
fn write_source(temp_dir: &TempDir, name: &str, content: &str) -> Result<PathBuf, std::io::Error> {
    let path = temp_dir.path().join(name);
    fs::write(&path, content)?;
    Ok(path)
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

fn response_payload(output: Vec<u8>) -> Result<serde_json::Value, String> {
    let response = String::from_utf8(output).map_err(|error| format!("utf8: {error}"))?;
    let stream_line = response
        .lines()
        .next()
        .ok_or_else(|| "stream line".to_string())?;
    let envelope: serde_json::Value =
        serde_json::from_str(stream_line).map_err(|error| format!("envelope: {error}"))?;
    let data = envelope
        .get("data")
        .and_then(|value| value.as_str())
        .ok_or_else(|| "stdout data".to_string())?;
    serde_json::from_str(data).map_err(|error| format!("payload: {error}"))
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
) -> Result<(i32, serde_json::Value), String> {
    let mut output = Vec::new();
    let mut writer = ResponseWriter::new(&mut output);
    let result = match handle(request, &mut writer, backends) {
        Ok(result) => result,
        Err(error) => return Err(format!("dispatch fails: {error}")),
    };
    response_payload(output).map(|payload| (result.status, payload))
}
fn assert_success_response(status: i32, payload: &serde_json::Value) {
    assert_eq!(status, 0, "expected success exit status");
    assert_eq!(
        payload["status"], "success",
        "expected success payload status"
    );
    assert_eq!(payload["schema_version"], "graph_slice.v1");
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
    assert_eq!(
        payload["spillover"]["truncated"], true,
        "expected truncated"
    );
    let frontier = match payload["spillover"]["frontier"].as_array() {
        Some(frontier) => frontier,
        None => panic!("frontier array"),
    };
    assert!(!frontier.is_empty(), "expected non-empty frontier");
    for (index, entry) in frontier.iter().enumerate() {
        assert!(
            entry["symbol_id"].as_str().is_some_and(|s| !s.is_empty()),
            "frontier entry {index} should have a non-empty symbol_id"
        );
        assert_eq!(
            entry["depth"],
            serde_json::json!(1),
            "frontier entry {index} depth should be 1"
        );
    }
}

fn assert_refusal(status: i32, payload: &serde_json::Value, reason: &str) {
    let expected_status = match reason {
        "unsupported_language" => 10,
        "no_symbol_at_position" => 11,
        "position_out_of_range" => 12,
        "not_yet_implemented" => 13,
        "backend_unavailable" => 14,
        other => panic!("unknown refusal reason {other}"),
    };
    assert_eq!(status, expected_status);
    assert_eq!(payload["status"], "refusal");
    assert_eq!(payload["schema_version"], "graph_slice.v1");
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
fn valid_request_returns_success_and_echoed_constraints(
    backends_fixture: Result<(FusionBackends<SemanticBackendProvider>, TempDir), String>,
) -> Result<(), String> {
    let (mut backends, temp_dir) = backends_fixture?;
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
    )
    .map_err(|error| error.to_string())?;
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

    let (status, payload) = dispatch_payload(&request, &mut backends)?;

    assert_success_response(status, &payload);
    assert_default_graph_slice_shape(&payload);
    assert_eq!(payload["spillover"]["truncated"], false);
    assert_eq!(payload["cards"][0]["symbol"]["ref"]["name"], "increment");
    assert!(payload["cards"].as_array().expect("cards array").len() >= 2);
    Ok(())
}

#[rstest]
fn max_cards_budget_truncates_same_file_symbol_inventory(
    backends_fixture: Result<(FusionBackends<SemanticBackendProvider>, TempDir), String>,
) -> Result<(), String> {
    let (mut backends, temp_dir) = backends_fixture?;
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
    )
    .map_err(|error| error.to_string())?;
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

    let (status, payload) = dispatch_payload(&request, &mut backends)?;

    assert_success_response(status, &payload);
    assert_eq!(payload["cards"].as_array().expect("cards array").len(), 1);
    assert_spillover_truncated_with_frontier(&payload);
    Ok(())
}

mod coverage_tests;
struct RefusalCase<'a> {
    filename: &'a str,
    content: &'a str,
    position: &'a str,
    expected_reason: &'a str,
    expected_message: Option<&'a str>,
}

fn assert_structured_refusal(
    backends: &mut FusionBackends<SemanticBackendProvider>,
    temp_dir: &TempDir,
    case: &RefusalCase<'_>,
) -> Result<(), String> {
    let path =
        write_source(temp_dir, case.filename, case.content).map_err(|error| error.to_string())?;
    let uri = Url::from_file_path(&path)
        .map_err(|()| "file uri".to_string())?
        .to_string();
    let request = make_request(&["--uri", &uri, "--position", case.position]);

    let (status, payload) = dispatch_payload(&request, backends)?;

    match case.expected_message {
        Some(message) => {
            assert_refusal_with_message(status, &payload, case.expected_reason, message);
        }
        None => assert_refusal(status, &payload, case.expected_reason),
    }
    Ok(())
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
fn structured_refusal_cases(
    backends_fixture: Result<(FusionBackends<SemanticBackendProvider>, TempDir), String>,
    #[case] case: (&str, &str, &str, &str, Option<&str>),
) -> Result<(), String> {
    let (mut backends, temp_dir) = backends_fixture?;
    let (filename, content, position, expected_reason, expected_message) = case;
    assert_structured_refusal(
        &mut backends,
        &temp_dir,
        &RefusalCase {
            filename,
            content,
            position,
            expected_reason,
            expected_message,
        },
    )
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
#[case(
    &["--uri", "file:///src/main.rs", "--position", "1:1", "--max-cards", "0"],
    "--max-cards must be >= 1"
)]
#[case(&["--uri", "file://%zz", "--position", "1:1"], "invalid URI")]
fn invalid_arguments_return_dispatch_error(
    backends_fixture: Result<(FusionBackends<SemanticBackendProvider>, TempDir), String>,
    #[case] arguments: &[&str],
    #[case] expected_substring: &str,
) -> Result<(), String> {
    let (mut backends, _temp_dir) = backends_fixture?;
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
    Ok(())
}

#[rstest]
fn missing_source_file_returns_invalid_arguments(
    backends_fixture: Result<(FusionBackends<SemanticBackendProvider>, TempDir), String>,
) -> Result<(), String> {
    let (mut backends, temp_dir) = backends_fixture?;
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
    Ok(())
}

#[test]
fn first_non_whitespace_column_uses_character_offsets() {
    assert_eq!(
        first_non_whitespace_column("\u{2003}\u{2003}fn main() {}"),
        Some(3)
    );
}
