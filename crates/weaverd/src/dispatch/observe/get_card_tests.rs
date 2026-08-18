//! Unit tests for `observe::get_card`.

use rstest::rstest;
use tempfile::TempDir;
use url::Url;
use weaver_cards::{DetailLevel, RefusalReason};
use weaver_lsp_host::{Language, ServerCapabilitySet};

use self::support::{
    CardProbe,
    SourceFile,
    backends,
    dispatch_payload,
    dispatch_source,
    make_request,
    response_payload,
    temp_dir,
    write_source,
};
use super::*;
use crate::{
    backends::FusionBackends,
    dispatch::observe::test_support::{
        StubLanguageServer,
        markdown_hover,
        semantic_backends_with_server,
    },
    semantic_provider::SemanticBackendProvider,
    tests::support::fs as test_fs,
};

#[path = "get_card_test_support.rs"]
mod support;

#[path = "get_card_semantic_tests.rs"]
mod semantic_tests;

type BackendsFixture = Result<(FusionBackends<SemanticBackendProvider>, TempDir), String>;

#[derive(Clone)]
struct RefusalCase<'a> {
    file: SourceFile<'a>,
    line: u32,
    column: u32,
    expected_reason: RefusalReason,
    expected_message_substring: &'a str,
}

/// The observable outcome of dispatching the same request twice.
struct CachedReuse {
    first_status: i32,
    second_status: i32,
    first_payload: serde_json::Value,
    second_payload: serde_json::Value,
    hits: u64,
    misses: u64,
}

/// Dispatches an identical request twice and reports the cache behaviour.
fn dispatch_cached_pair(
    temp_dir: &TempDir,
    backends: &mut FusionBackends<SemanticBackendProvider>,
    detail: DetailLevel,
) -> Result<CachedReuse, String> {
    let path = write_source(
        temp_dir,
        SourceFile {
            name: "cache.rs",
            content: "fn greet() -> usize {\n    1\n}\n",
        },
    )?;
    let uri = Url::from_file_path(&path)
        .map_err(|()| "file uri".to_string())?
        .to_string();
    let request = make_request(&uri, 1, 4, detail)?;

    let (first_result, first_payload) = dispatch_payload(&request, backends)?;
    let (second_result, second_payload) = dispatch_payload(&request, backends)?;
    let stats = backends.provider().card_extractor().cache_stats();

    Ok(CachedReuse {
        first_status: first_result.status,
        second_status: second_result.status,
        first_payload,
        second_payload,
        hits: stats.hits,
        misses: stats.misses,
    })
}

/// Asserts a repeated request was served from the card cache.
macro_rules! assert_cached_request_reuse {
    ($reuse:expr, $detail:expr) => {{
        let reuse = $reuse;
        let detail: DetailLevel = $detail;
        assert_eq!(reuse.first_status, 0);
        assert_eq!(reuse.second_status, 0);
        assert_eq!(
            reuse.first_payload["card"]["provenance"]["extracted_at"],
            reuse.second_payload["card"]["provenance"]["extracted_at"]
        );
        assert_eq!(reuse.hits, 1);
        assert_eq!(reuse.misses, 1);
        if detail >= DetailLevel::Semantic {
            assert_eq!(
                reuse.first_payload["card"]["lsp"],
                reuse.second_payload["card"]["lsp"]
            );
        }
    }};
}

#[rstest]
fn handle_returns_success_for_supported_rust_symbol(
    temp_dir: Result<TempDir, String>,
    backends: BackendsFixture,
) -> Result<(), String> {
    let (mut fusion, _dir) = backends?;
    let dir = temp_dir?;

    let (result, payload) = dispatch_source(
        &mut fusion,
        &dir,
        SourceFile {
            name: "card.rs",
            content: "/// Greets callers.\nfn greet(name: &str) -> usize {\n    let count = \
                      name.len();\n    count\n}\n",
        },
        CardProbe {
            position: (2, 4),
            detail: DetailLevel::Structure,
        },
    )?;

    assert_eq!(result.status, 0);
    assert_eq!(payload["status"], "success");
    assert_eq!(payload["card"]["symbol"]["ref"]["name"], "greet");
    Ok(())
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
    temp_dir: Result<TempDir, String>,
    #[case] case: RefusalCase<'static>,
    backends: BackendsFixture,
) -> Result<(), String> {
    let (mut fusion, _dir) = backends?;
    let dir = temp_dir?;
    let expected_reason = serde_json::to_value(&case.expected_reason)
        .map_err(|error| format!("serialise reason: {error}"))?;

    let (result, payload) = dispatch_source(
        &mut fusion,
        &dir,
        case.file,
        CardProbe {
            position: (case.line, case.column),
            detail: DetailLevel::Structure,
        },
    )?;

    assert_eq!(result.status, 1);
    assert_eq!(payload["status"], "refusal");
    assert_eq!(payload["refusal"]["reason"], expected_reason);
    let message = payload["refusal"]["message"]
        .as_str()
        .expect("refusal message should be a string");
    assert!(
        message.contains(case.expected_message_substring),
        "expected message '{message}' to contain '{}'",
        case.expected_message_substring
    );
    Ok(())
}

#[rstest]
fn handle_rejects_non_file_uri(backends: BackendsFixture) -> Result<(), String> {
    let (mut fusion, _dir) = backends?;
    let request = make_request("https://example.com/demo.rs", 1, 1, DetailLevel::Minimal)?;
    let mut output = Vec::new();
    let mut writer = ResponseWriter::new(&mut output);

    let error = match handle(&request, &mut writer, &mut fusion) {
        Ok(result) => panic!("handler unexpectedly succeeded: {}", result.status),
        Err(error) => error,
    };

    assert!(matches!(error, DispatchError::InvalidArguments { .. }));
    assert!(error.to_string().contains("unsupported URI scheme"));
    Ok(())
}

#[rstest]
fn handle_reuses_cached_cards_for_identical_requests(
    temp_dir: Result<TempDir, String>,
    backends: BackendsFixture,
) -> Result<(), String> {
    let (mut fusion, _dir) = backends?;
    let dir = temp_dir?;

    let reuse = dispatch_cached_pair(&dir, &mut fusion, DetailLevel::Structure)?;

    assert_cached_request_reuse!(reuse, DetailLevel::Structure);
    Ok(())
}

#[rstest]
fn handle_reuses_cached_cards_for_identical_semantic_requests(
    temp_dir: Result<TempDir, String>,
    backends: BackendsFixture,
) -> Result<(), String> {
    let (mut fusion, _dir) = backends?;
    let dir = temp_dir?;

    let reuse = dispatch_cached_pair(&dir, &mut fusion, DetailLevel::Semantic)?;

    assert_cached_request_reuse!(reuse, DetailLevel::Semantic);
    Ok(())
}

#[rstest]
fn handle_invalidates_stale_revisions_when_file_changes(
    temp_dir: Result<TempDir, String>,
    backends: BackendsFixture,
) -> Result<(), String> {
    let (mut fusion, _dir) = backends?;
    let dir = temp_dir?;
    let path = write_source(
        &dir,
        SourceFile {
            name: "cache.rs",
            content: "fn greet() -> usize {\n    1\n}\n",
        },
    )?;
    let uri = Url::from_file_path(&path)
        .map_err(|()| "file uri".to_string())?
        .to_string();
    let request = make_request(&uri, 1, 4, DetailLevel::Structure)?;

    let (_, first_payload) = dispatch_payload(&request, &mut fusion)?;
    test_fs::write(&path, "fn welcome() -> usize {\n    2\n}\n")
        .map_err(|error| format!("rewrite source: {error}"))?;
    let (_, second_payload) = dispatch_payload(&request, &mut fusion)?;
    let extractor = fusion.provider().card_extractor();
    let stats = extractor.cache_stats();

    assert_eq!(extractor.cache_len(), 1);
    assert_eq!(stats.hits, 0);
    assert_eq!(stats.misses, 2);
    assert_ne!(
        first_payload["card"]["symbol"]["ref"]["name"],
        second_payload["card"]["symbol"]["ref"]["name"]
    );
    Ok(())
}
