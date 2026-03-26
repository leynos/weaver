//! Unit tests for `observe::get_card`.

use std::fs;

use rstest::{fixture, rstest};
use tempfile::TempDir;
use url::Url;
use weaver_cards::{DetailLevel, RefusalReason};
use weaver_config::{CapabilityMatrix, Config, SocketEndpoint};
use weaver_lsp_host::{Language, ServerCapabilitySet};

use super::*;
use crate::backends::FusionBackends;
use crate::dispatch::observe::test_support::{
    StubLanguageServer, markdown_hover, semantic_backends_with_server,
};
use crate::dispatch::request::CommandRequest;
use crate::semantic_provider::SemanticBackendProvider;

#[fixture]
fn temp_dir() -> TempDir {
    TempDir::new().expect("temp dir")
}

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
    let provider = SemanticBackendProvider::new(CapabilityMatrix::default());
    (FusionBackends::new(config, provider), dir)
}

#[derive(Clone, Copy)]
struct SourceFile<'a> {
    name: &'a str,
    content: &'a str,
}

#[derive(Clone)]
struct RefusalCase<'a> {
    file: SourceFile<'a>,
    line: u32,
    column: u32,
    expected_reason: RefusalReason,
    expected_message_substring: &'a str,
}

fn write_source(temp_dir: &TempDir, file: SourceFile<'_>) -> PathBuf {
    let path = temp_dir.path().join(file.name);
    fs::write(&path, file.content).expect("write source");
    path
}

fn make_request(uri: &str, line: u32, column: u32, detail: DetailLevel) -> CommandRequest {
    let detail_str = match detail {
        DetailLevel::Minimal => "minimal",
        DetailLevel::Signature => "signature",
        DetailLevel::Structure => "structure",
        DetailLevel::Semantic => "semantic",
        DetailLevel::Full => "full",
        detail => unreachable!("unexpected DetailLevel variant: {:?}", detail),
    };
    CommandRequest::parse(
        format!(
            concat!(
                "{{\"command\":{{\"domain\":\"observe\",\"operation\":\"get-card\"}},",
                "\"arguments\":[\"--uri\",\"{uri}\",\"--position\",\"{line}:{column}\",",
                "\"--detail\",\"{detail}\"]}}"
            ),
            uri = uri,
            line = line,
            column = column,
            detail = detail_str,
        )
        .as_bytes(),
    )
    .expect("request")
}

fn response_text(output: Vec<u8>) -> String {
    String::from_utf8(output).expect("utf8")
}

fn response_payload(output: Vec<u8>) -> serde_json::Value {
    let response = response_text(output);
    let stream_line = response.lines().next().expect("stream line");
    let envelope: serde_json::Value = serde_json::from_str(stream_line).expect("envelope");
    let data = envelope["data"].as_str().expect("stdout data");
    serde_json::from_str(data).expect("payload")
}

fn dispatch_payload(
    request: &CommandRequest,
    backends: &mut FusionBackends<SemanticBackendProvider>,
) -> (DispatchResult, serde_json::Value) {
    let mut output = Vec::new();
    let mut writer = ResponseWriter::new(&mut output);
    let result = handle(request, &mut writer, backends).expect("handler should succeed");
    (result, response_payload(output))
}

fn assert_refusal_response(
    temp_dir: TempDir,
    case: RefusalCase<'_>,
    backends: &mut FusionBackends<SemanticBackendProvider>,
) {
    let path = write_source(&temp_dir, case.file);
    let uri = Url::from_file_path(&path).expect("file uri").to_string();
    let request = make_request(&uri, case.line, case.column, DetailLevel::Structure);
    let mut output = Vec::new();
    let mut writer = ResponseWriter::new(&mut output);

    let result = handle(&request, &mut writer, backends).expect("handler should succeed");

    assert_eq!(result.status, 1);
    let payload = response_payload(output);
    assert_eq!(payload["status"], "refusal");
    assert_eq!(
        payload["refusal"]["reason"],
        serde_json::to_value(&case.expected_reason).expect("serialise reason")
    );
    let message = payload["refusal"]["message"]
        .as_str()
        .expect("refusal message");
    assert!(
        message.contains(case.expected_message_substring),
        "expected message '{message}' to contain '{}'",
        case.expected_message_substring
    );
}

#[rstest]
fn handle_returns_success_for_supported_rust_symbol(
    temp_dir: TempDir,
    backends: (FusionBackends<SemanticBackendProvider>, TempDir),
) {
    let (mut backends, _dir) = backends;
    let path = write_source(
        &temp_dir,
        SourceFile {
            name: "card.rs",
            content: "/// Greets callers.\nfn greet(name: &str) -> usize {\n    let count = name.len();\n    count\n}\n",
        },
    );
    let uri = Url::from_file_path(&path).expect("file uri").to_string();
    let request = make_request(&uri, 2, 4, DetailLevel::Structure);
    let mut output = Vec::new();
    let mut writer = ResponseWriter::new(&mut output);

    let result = handle(&request, &mut writer, &mut backends).expect("handler should succeed");

    assert_eq!(result.status, 0);
    let payload = response_payload(output);
    assert_eq!(payload["status"], "success");
    assert_eq!(payload["card"]["symbol"]["ref"]["name"], "greet");
}

#[rstest]
fn handle_returns_semantic_success_with_enrichment_and_rewritten_provenance(temp_dir: TempDir) {
    let path = write_source(
        &temp_dir,
        SourceFile {
            name: "card.rs",
            content: "/// Greets callers.\nfn greet(name: &str) -> usize {\n    let count = name.len();\n    count\n}\n",
        },
    );
    let uri = Url::from_file_path(&path).expect("file uri").to_string();
    let request = make_request(&uri, 2, 4, DetailLevel::Semantic);
    let (server, _hover_params) = StubLanguageServer::with_hover(
        ServerCapabilitySet::new(false, false, false).with_hover(true),
        markdown_hover(concat!(
            "```rust\nfn greet(name: &str) -> usize\n```\n",
            "**Deprecated**: use `welcome` instead"
        )),
    );
    let (mut backends, _dir) = semantic_backends_with_server(Language::Rust, server);
    let mut output = Vec::new();
    let mut writer = ResponseWriter::new(&mut output);

    let result = handle(&request, &mut writer, &mut backends).expect("handler should succeed");

    assert_eq!(result.status, 0);
    let payload = response_payload(output);
    assert_eq!(payload["status"], "success");
    assert_eq!(payload["card"]["lsp"]["source"], "lsp_hover");
    assert_eq!(
        payload["card"]["lsp"]["type"],
        "fn greet(name: &str) -> usize"
    );
    assert_eq!(payload["card"]["lsp"]["deprecated"], true);
    assert_eq!(
        payload["card"]["provenance"]["sources"],
        serde_json::json!(["tree_sitter", "lsp_hover"])
    );
}

#[rstest]
fn handle_returns_semantic_success_with_degraded_provenance_when_hover_is_unavailable(
    temp_dir: TempDir,
) {
    let path = write_source(
        &temp_dir,
        SourceFile {
            name: "card.rs",
            content: "/// Greets callers.\nfn greet(name: &str) -> usize {\n    let count = name.len();\n    count\n}\n",
        },
    );
    let uri = Url::from_file_path(&path).expect("file uri").to_string();
    let request = make_request(&uri, 2, 4, DetailLevel::Semantic);
    let (server, _hover_params) =
        StubLanguageServer::missing_hover(ServerCapabilitySet::new(false, false, false));
    let (mut backends, _dir) = semantic_backends_with_server(Language::Rust, server);
    let mut output = Vec::new();
    let mut writer = ResponseWriter::new(&mut output);

    let result = handle(&request, &mut writer, &mut backends).expect("handler should succeed");

    assert_eq!(result.status, 0);
    let payload = response_payload(output);
    assert_eq!(payload["status"], "success");
    assert!(payload["card"]["lsp"].is_null());
    assert_eq!(
        payload["card"]["provenance"]["sources"],
        serde_json::json!(["tree_sitter", "tree_sitter_degraded_semantic"])
    );
}

#[rstest]
#[case(
    RefusalCase {
        file: SourceFile {
            name: "notes.txt",
            content: "plain text",
        },
        line: 1,
        column: 1,
        expected_reason: RefusalReason::UnsupportedLanguage,
        expected_message_substring: "unsupported language for path",
    }
)]
#[case(
    RefusalCase {
        file: SourceFile {
            name: "empty.py",
            content: "# heading\n\ndef greet() -> None:\n    return None\n",
        },
        line: 1,
        column: 1,
        expected_reason: RefusalReason::NoSymbolAtPosition,
        expected_message_substring: "no symbol found at 1:1",
    }
)]
#[case(
    RefusalCase {
        file: SourceFile {
            name: "bounds.rs",
            content: "fn greet() {}\n",
        },
        line: 10,
        column: 100,
        expected_reason: RefusalReason::PositionOutOfRange,
        expected_message_substring: "position 10:100 is outside the bounds of the file",
    }
)]
fn handle_returns_structured_refusals(
    temp_dir: TempDir,
    #[case] case: RefusalCase<'static>,
    backends: (FusionBackends<SemanticBackendProvider>, TempDir),
) {
    let (mut backends, _dir) = backends;
    assert_refusal_response(temp_dir, case, &mut backends);
}

#[rstest]
fn handle_rejects_non_file_uri(backends: (FusionBackends<SemanticBackendProvider>, TempDir)) {
    let (mut backends, _dir) = backends;
    let request = make_request("https://example.com/demo.rs", 1, 1, DetailLevel::Minimal);
    let mut output = Vec::new();
    let mut writer = ResponseWriter::new(&mut output);

    let error = match handle(&request, &mut writer, &mut backends) {
        Ok(result) => panic!("handler unexpectedly succeeded: {}", result.status),
        Err(error) => error,
    };

    assert!(matches!(error, DispatchError::InvalidArguments { .. }));
    assert!(error.to_string().contains("unsupported URI scheme"));
}

#[rstest]
fn handle_reuses_cached_cards_for_identical_requests(
    temp_dir: TempDir,
    backends: (FusionBackends<SemanticBackendProvider>, TempDir),
) {
    let (mut backends, _dir) = backends;
    let path = write_source(
        &temp_dir,
        SourceFile {
            name: "cache.rs",
            content: "fn greet() -> usize {\n    1\n}\n",
        },
    );
    let uri = Url::from_file_path(&path).expect("file uri").to_string();
    let request = make_request(&uri, 1, 4, DetailLevel::Structure);

    let (first_result, first_payload) = dispatch_payload(&request, &mut backends);
    let (second_result, second_payload) = dispatch_payload(&request, &mut backends);
    let stats = backends.provider().card_extractor().cache_stats();

    assert_eq!(first_result.status, 0);
    assert_eq!(second_result.status, 0);
    assert_eq!(
        first_payload["card"]["provenance"]["extracted_at"],
        second_payload["card"]["provenance"]["extracted_at"]
    );
    assert_eq!(stats.hits, 1);
    assert_eq!(stats.misses, 1);
}

#[rstest]
fn handle_invalidates_stale_revisions_when_file_changes(
    temp_dir: TempDir,
    backends: (FusionBackends<SemanticBackendProvider>, TempDir),
) {
    let (mut backends, _dir) = backends;
    let path = write_source(
        &temp_dir,
        SourceFile {
            name: "cache.rs",
            content: "fn greet() -> usize {\n    1\n}\n",
        },
    );
    let uri = Url::from_file_path(&path).expect("file uri").to_string();
    let request = make_request(&uri, 1, 4, DetailLevel::Structure);

    let (_, first_payload) = dispatch_payload(&request, &mut backends);
    fs::write(&path, "fn welcome() -> usize {\n    2\n}\n").expect("rewrite source");
    let (_, second_payload) = dispatch_payload(&request, &mut backends);
    let extractor = backends.provider().card_extractor();
    let stats = extractor.cache_stats();

    assert_eq!(extractor.cache_len(), 1);
    assert_eq!(stats.hits, 0);
    assert_eq!(stats.misses, 2);
    assert_ne!(
        first_payload["card"]["symbol"]["ref"]["name"],
        second_payload["card"]["symbol"]["ref"]["name"]
    );
}
