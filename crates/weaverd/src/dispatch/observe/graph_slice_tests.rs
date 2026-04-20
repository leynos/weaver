//! Unit tests for the `observe graph-slice` dispatch handler.

use std::fs;
use std::path::PathBuf;

use rstest::{fixture, rstest};
use tempfile::TempDir;
use url::Url;
use weaver_cards::{DEFAULT_CACHE_CAPACITY, DetailLevel};
use weaver_config::{CapabilityMatrix, Config, SocketEndpoint};

use super::{first_non_whitespace_column, handle};
use crate::backends::FusionBackends;
use crate::dispatch::errors::DispatchError;
use crate::dispatch::request::CommandRequest;
use crate::dispatch::response::ResponseWriter;
use crate::semantic_provider::SemanticBackendProvider;

#[fixture]
fn backends() -> (FusionBackends<SemanticBackendProvider>, TempDir) {
    let dir = TempDir::new().expect("create temp dir");
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
    fs::write(&path, content).expect("write source");
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
    CommandRequest::parse(json.as_bytes()).expect("request")
}

fn response_payload(output: Vec<u8>) -> serde_json::Value {
    let response = String::from_utf8(output).expect("utf8");
    let stream_line = response.lines().next().expect("stream line");
    let envelope: serde_json::Value = serde_json::from_str(stream_line).expect("envelope");
    let data = envelope["data"].as_str().expect("stdout data");
    serde_json::from_str(data).expect("payload")
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
    let result = handle(request, &mut writer, backends).expect("dispatch succeeds");
    (result.status, response_payload(output))
}

#[rstest]
fn valid_request_returns_success_and_echoed_constraints(
    backends: (FusionBackends<SemanticBackendProvider>, TempDir),
) {
    let (mut backends, temp_dir) = backends;
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

    assert_eq!(status, 0);
    assert_eq!(payload["status"], "success");
    assert_eq!(payload["constraints"]["direction"], "both");
    assert_eq!(
        payload["constraints"]["budget"]["max_cards"],
        serde_json::json!(30)
    );
    assert_eq!(payload["edges"], serde_json::json!([]));
    assert_eq!(payload["spillover"]["truncated"], false);
    assert_eq!(payload["cards"][0]["symbol"]["ref"]["name"], "increment");
    assert!(payload["cards"].as_array().expect("cards array").len() >= 2);
}

#[rstest]
fn max_cards_budget_truncates_same_file_symbol_inventory(
    backends: (FusionBackends<SemanticBackendProvider>, TempDir),
) {
    let (mut backends, temp_dir) = backends;
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

    assert_eq!(status, 0);
    assert_eq!(payload["status"], "success");
    assert_eq!(payload["cards"].as_array().expect("cards array").len(), 1);
    assert_eq!(payload["spillover"]["truncated"], true);
    assert!(
        !payload["spillover"]["frontier"]
            .as_array()
            .expect("frontier array")
            .is_empty()
    );
}

#[rstest]
fn unsupported_language_returns_structured_refusal(
    backends: (FusionBackends<SemanticBackendProvider>, TempDir),
) {
    let (mut backends, temp_dir) = backends;
    let path = write_source(&temp_dir, "notes.txt", "plain text\n");
    let uri = Url::from_file_path(&path).expect("file uri").to_string();
    let request = make_request(&["--uri", &uri, "--position", "1:1"]);

    let (status, payload) = dispatch_payload(&request, &mut backends);

    assert_eq!(status, 1);
    assert_eq!(payload["status"], "refusal");
    assert_eq!(payload["refusal"]["reason"], "unsupported_language");
}

#[rstest]
fn no_symbol_at_position_returns_structured_refusal(
    backends: (FusionBackends<SemanticBackendProvider>, TempDir),
) {
    let (mut backends, temp_dir) = backends;
    let path = write_source(&temp_dir, "main.rs", "fn main() {}\n \n");
    let uri = Url::from_file_path(&path).expect("file uri").to_string();
    let request = make_request(&["--uri", &uri, "--position", "2:1"]);

    let (status, payload) = dispatch_payload(&request, &mut backends);

    assert_eq!(status, 1);
    assert_eq!(payload["status"], "refusal");
    assert_eq!(payload["refusal"]["reason"], "no_symbol_at_position");
    assert_eq!(
        payload["refusal"]["message"],
        "observe graph-slice: no symbol found at 2:1"
    );
}

#[rstest]
fn position_out_of_range_returns_structured_refusal(
    backends: (FusionBackends<SemanticBackendProvider>, TempDir),
) {
    let (mut backends, temp_dir) = backends;
    let path = write_source(&temp_dir, "main.rs", "fn main() {}\n");
    let uri = Url::from_file_path(&path).expect("file uri").to_string();
    let request = make_request(&["--uri", &uri, "--position", "10:1"]);

    let (status, payload) = dispatch_payload(&request, &mut backends);

    assert_eq!(status, 1);
    assert_eq!(payload["status"], "refusal");
    assert_eq!(payload["refusal"]["reason"], "position_out_of_range");
    assert_eq!(
        payload["refusal"]["message"],
        "observe graph-slice: position 10:1 is outside the bounds of the file"
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
    backends: (FusionBackends<SemanticBackendProvider>, TempDir),
) {
    let (mut backends, _temp_dir) = backends;
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
                    "expected invalid-arguments message to contain {expected_substring:?}, got: {message}"
                );
            }
            _ => panic!("expected invalid arguments error"),
        },
    }
}

#[rstest]
fn missing_source_file_returns_invalid_arguments(
    backends: (FusionBackends<SemanticBackendProvider>, TempDir),
) {
    let (mut backends, temp_dir) = backends;
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
