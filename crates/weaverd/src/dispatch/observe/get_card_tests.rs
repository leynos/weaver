//! Unit tests for `observe::get_card`.

use std::fs;

use rstest::{fixture, rstest};
use tempfile::TempDir;
use url::Url;
use weaver_cards::{DEFAULT_CACHE_CAPACITY, DetailLevel, RefusalReason};
use weaver_config::{CapabilityMatrix, Config, SocketEndpoint};
use weaver_lsp_host::{Language, ServerCapabilitySet};
use weaver_test_macros::allow_fixture_expansion_lints;

use super::*;
use crate::{
    backends::FusionBackends,
    dispatch::{
        observe::test_support::{
            StubLanguageServer,
            markdown_hover,
            semantic_backends_with_server,
        },
        request::CommandRequest,
    },
    semantic_provider::SemanticBackendProvider,
};

#[path = "get_card_semantic_tests.rs"]
mod semantic_tests;

#[allow_fixture_expansion_lints]
#[fixture]
fn temp_dir() -> TempDir {
    match TempDir::new() {
        Ok(temp_dir) => temp_dir,
        Err(error) => panic!("temp dir: {error}"),
    }
}

#[fixture]
fn backends() -> (FusionBackends<SemanticBackendProvider>, TempDir) {
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
    if let Err(error) = fs::write(&path, file.content) {
        panic!("write source: {error}");
    }
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
    match CommandRequest::parse(
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
    ) {
        Ok(request) => request,
        Err(error) => panic!("request: {error}"),
    }
}

fn response_text(output: Vec<u8>) -> String {
    match String::from_utf8(output) {
        Ok(text) => text,
        Err(error) => panic!("utf8: {error}"),
    }
}

fn response_payload(output: Vec<u8>) -> serde_json::Value {
    let response = response_text(output);
    let Some(stream_line) = response.lines().next() else {
        panic!("stream line");
    };
    let envelope: serde_json::Value = match serde_json::from_str(stream_line) {
        Ok(envelope) => envelope,
        Err(error) => panic!("envelope: {error}"),
    };
    let Some(data) = envelope["data"].as_str() else {
        panic!("stdout data");
    };
    match serde_json::from_str(data) {
        Ok(payload) => payload,
        Err(error) => panic!("payload: {error}"),
    }
}

fn dispatch_payload(
    request: &CommandRequest,
    backends: &mut FusionBackends<SemanticBackendProvider>,
) -> (DispatchResult, serde_json::Value) {
    let mut output = Vec::new();
    let mut writer = ResponseWriter::new(&mut output);
    let result = match handle(request, &mut writer, backends) {
        Ok(result) => result,
        Err(error) => panic!("handler should succeed: {error}"),
    };
    (result, response_payload(output))
}

fn assert_refusal_response(
    temp_dir: TempDir,
    case: RefusalCase<'_>,
    backends: &mut FusionBackends<SemanticBackendProvider>,
) {
    let path = write_source(&temp_dir, case.file);
    let uri = match Url::from_file_path(&path) {
        Ok(uri) => uri,
        Err(()) => panic!("file uri"),
    }
    .to_string();
    let request = make_request(&uri, case.line, case.column, DetailLevel::Structure);
    let mut output = Vec::new();
    let mut writer = ResponseWriter::new(&mut output);

    let result = match handle(&request, &mut writer, backends) {
        Ok(result) => result,
        Err(error) => panic!("handler should succeed: {error}"),
    };

    assert_eq!(result.status, 1);
    let payload = response_payload(output);
    assert_eq!(payload["status"], "refusal");
    assert_eq!(
        payload["refusal"]["reason"],
        match serde_json::to_value(&case.expected_reason) {
            Ok(reason) => reason,
            Err(error) => panic!("serialise reason: {error}"),
        }
    );
    let Some(message) = payload["refusal"]["message"].as_str() else {
        panic!("refusal message");
    };
    assert!(
        message.contains(case.expected_message_substring),
        "expected message '{message}' to contain '{}'",
        case.expected_message_substring
    );
}

fn assert_cached_request_reuse(
    temp_dir: &TempDir,
    backends: &mut FusionBackends<SemanticBackendProvider>,
    detail: DetailLevel,
) {
    let path = write_source(
        temp_dir,
        SourceFile {
            name: "cache.rs",
            content: "fn greet() -> usize {\n    1\n}\n",
        },
    );
    let uri = match Url::from_file_path(&path) {
        Ok(uri) => uri,
        Err(()) => panic!("file uri"),
    }
    .to_string();
    let request = make_request(&uri, 1, 4, detail);

    let (first_result, first_payload) = dispatch_payload(&request, backends);
    let (second_result, second_payload) = dispatch_payload(&request, backends);
    let stats = backends.provider().card_extractor().cache_stats();

    assert_eq!(first_result.status, 0);
    assert_eq!(second_result.status, 0);
    assert_eq!(
        first_payload["card"]["provenance"]["extracted_at"],
        second_payload["card"]["provenance"]["extracted_at"]
    );
    assert_eq!(stats.hits, 1);
    assert_eq!(stats.misses, 1);
    if detail >= DetailLevel::Semantic {
        assert_eq!(first_payload["card"]["lsp"], second_payload["card"]["lsp"]);
    }
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
            content: "/// Greets callers.\nfn greet(name: &str) -> usize {\n    let count = \
                      name.len();\n    count\n}\n",
        },
    );
    let uri = Url::from_file_path(&path).expect("file uri").to_string();
    let request = make_request(&uri, 2, 4, DetailLevel::Structure);
    let mut output = Vec::new();
    let mut writer = ResponseWriter::new(&mut output);

    let result = match handle(&request, &mut writer, &mut backends) {
        Ok(result) => result,
        Err(error) => panic!("handler should succeed: {error}"),
    };

    assert_eq!(result.status, 0);
    let payload = response_payload(output);
    assert_eq!(payload["status"], "success");
    assert_eq!(payload["card"]["symbol"]["ref"]["name"], "greet");
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
    assert_cached_request_reuse(&temp_dir, &mut backends, DetailLevel::Structure);
}

#[rstest]
fn handle_reuses_cached_cards_for_identical_semantic_requests(
    temp_dir: TempDir,
    backends: (FusionBackends<SemanticBackendProvider>, TempDir),
) {
    let (mut backends, _dir) = backends;
    assert_cached_request_reuse(&temp_dir, &mut backends, DetailLevel::Semantic);
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
