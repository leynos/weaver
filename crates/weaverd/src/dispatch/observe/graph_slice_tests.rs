//! Unit tests for the `observe graph-slice` dispatch handler.

use std::fs;
use std::path::PathBuf;

use rstest::{fixture, rstest};
use tempfile::TempDir;
use url::Url;
use weaver_cards::{DEFAULT_CACHE_CAPACITY, DetailLevel};
use weaver_config::{CapabilityMatrix, Config, SocketEndpoint};

use super::handle;
use crate::backends::FusionBackends;
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
        detail => unreachable!("unexpected detail level: {detail:?}"),
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
#[case(&["--position", "10:5"])]
#[case(&["--uri", "file:///src/main.rs", "--position", "bad"])]
fn invalid_arguments_return_dispatch_error(
    #[case] arguments: &[&str],
    backends: (FusionBackends<SemanticBackendProvider>, TempDir),
) {
    let (mut backends, _temp_dir) = backends;
    let request = make_request(arguments);
    let mut buffer = Vec::new();
    let mut writer = ResponseWriter::new(&mut buffer);
    let result = handle(&request, &mut writer, &mut backends);
    assert!(result.is_err());
}
