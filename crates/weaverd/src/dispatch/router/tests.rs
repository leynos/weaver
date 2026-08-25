//! Router behaviour tests for command dispatch.

use std::path::PathBuf;

use rstest::{fixture, rstest};
use tempfile::TempDir;
use url::Url;
use weaver_cards::DEFAULT_CACHE_CAPACITY;
use weaver_config::{CapabilityMatrix, Config, SocketEndpoint};
use weaver_test_macros::allow_fixture_expansion_lints;

use super::*;
use crate::{dispatch::request::CommandRequest, tests::support::fs as test_fs};

fn make_request(domain: &str, operation: &str) -> Result<CommandRequest, String> {
    let payload = serde_json::json!({
        "command": {"domain": domain, "operation": operation},
    });
    let json =
        serde_json::to_vec(&payload).map_err(|error| format!("serialise test request: {error}"))?;
    CommandRequest::parse(&json).map_err(|error| format!("test request: {error}"))
}

fn build_backends() -> FusionBackends<SemanticBackendProvider> {
    let config = Config {
        daemon_socket: SocketEndpoint::unix("/tmp/weaver-test/socket.sock"),
        ..Config::default()
    };
    let provider =
        SemanticBackendProvider::new(CapabilityMatrix::default(), DEFAULT_CACHE_CAPACITY);
    FusionBackends::new(config, provider)
}

fn build_router() -> Result<DomainRouter, String> {
    DomainRouter::new(PathBuf::from("/tmp/weaver-test-workspace"))
        .map_err(|error| format!("absolute workspace root: {error}"))
}

#[test]
fn build_router_rejects_relative_workspace_root() {
    let error = match DomainRouter::new(PathBuf::from("relative/workspace")) {
        Ok(_) => panic!("relative workspace roots should be rejected"),
        Err(error) => error,
    };

    assert!(matches!(error, DispatchError::InvalidArguments { .. }));
}

#[allow_fixture_expansion_lints]
#[fixture]
fn backends() -> FusionBackends<SemanticBackendProvider> { build_backends() }

fn invalid_arguments_message(domain: &str, operation: &str) -> Option<&'static str> {
    match (domain, operation) {
        ("observe", "get-definition") => {
            Some("observe get-definition should fail with InvalidArguments (no args provided)")
        }
        ("observe", "get-card") => {
            Some("observe get-card should fail with InvalidArguments (no args provided)")
        }
        ("observe", "graph-slice") => {
            Some("observe graph-slice should fail with InvalidArguments (no args provided)")
        }
        ("act", "apply-patch") => {
            Some("act apply-patch should fail with InvalidArguments (missing patch)")
        }
        ("act", "refactor") => {
            Some("act refactor should fail with InvalidArguments (missing required flags)")
        }
        _ => None,
    }
}

/// Routes one operation and returns the dispatch outcome under test.
///
/// The outer `Result` reports harness failures; the inner one is the value the
/// assertions inspect.
fn route_operation(
    router: &DomainRouter,
    domain: &str,
    operation: &str,
    backends: &mut FusionBackends<SemanticBackendProvider>,
) -> Result<Result<DispatchResult, DispatchError>, String> {
    let request = make_request(domain, operation)?;
    let mut output = Vec::new();
    let mut writer = ResponseWriter::new(&mut output);
    Ok(router.route(&request, &mut writer, backends))
}

/// Asserts an operation routed, tolerating the argument errors that operations
/// requiring flags report when invoked without them: reaching argument
/// validation already proves the operation was recognized and routed.
macro_rules! assert_routed {
    ($result:expr, $domain:expr, $operation:expr) => {{
        let result = $result;
        let domain: &str = $domain;
        let operation: &str = $operation;
        match invalid_arguments_message(domain, operation) {
            Some(message) => assert!(
                matches!(result, Err(DispatchError::InvalidArguments { .. })),
                "{message}"
            ),
            None => assert!(
                result.is_ok(),
                "{domain} {operation} should route successfully"
            ),
        }
    }};
}

#[rstest]
#[case::observe_lower("observe", Domain::Observe)]
#[case::observe_upper("OBSERVE", Domain::Observe)]
#[case::observe_mixed("Observe", Domain::Observe)]
#[case::act_lower("act", Domain::Act)]
#[case::act_upper("ACT", Domain::Act)]
#[case::verify_lower("verify", Domain::Verify)]
#[case::verify_upper("VERIFY", Domain::Verify)]
fn domain_parse_case_insensitive(#[case] input: &str, #[case] expected: Domain) {
    assert_eq!(Domain::parse(input).expect("parse domain"), expected);
}

#[test]
fn domain_parse_rejects_unknown() {
    let result = Domain::parse("bogus");
    assert!(matches!(result, Err(DispatchError::UnknownDomain { .. })));
}

#[rstest]
#[case::observe("observe", DomainRoutingContext::OBSERVE.known_operations)]
#[case::act("act", DomainRoutingContext::ACT.known_operations)]
#[case::verify("verify", DomainRoutingContext::VERIFY.known_operations)]
fn routes_known_operations(
    #[case] domain: &str,
    #[case] operations: &'static [&'static str],
) -> Result<(), String> {
    let router = build_router()?;
    for operation in operations {
        let mut backends = build_backends();
        let result = route_operation(&router, domain, operation, &mut backends)?;
        assert_routed!(result, domain, operation);
    }
    Ok(())
}

#[rstest]
#[case::observe("observe", "nonexistent")]
#[case::act("act", "bogus")]
#[case::verify("verify", "unknown")]
fn rejects_unknown_operation(#[case] domain: &str, #[case] operation: &str) -> Result<(), String> {
    let router = build_router()?;
    let mut backends = build_backends();

    let result = route_operation(&router, domain, operation, &mut backends)?;

    assert!(matches!(
        result,
        Err(DispatchError::UnknownOperation { .. })
    ));
    Ok(())
}

#[rstest]
#[case::title("act", "Apply-Patch")]
#[case::screaming("verify", "DIAGNOSTICS")]
fn routes_operations_case_insensitively(
    #[case] domain: &str,
    #[case] operation: &str,
    mut backends: FusionBackends<SemanticBackendProvider>,
) -> Result<(), String> {
    let router = build_router()?;

    let result = route_operation(&router, domain, operation, &mut backends)?;

    let domain_norm = domain.to_ascii_lowercase();
    let operation_norm = operation.to_ascii_lowercase();
    assert_routed!(result, &domain_norm, &operation_norm);
    Ok(())
}

#[rstest]
fn get_card_returns_structured_refusal(mut backends: FusionBackends<SemanticBackendProvider>) {
    let router = build_router().expect("router with absolute workspace root");
    let temp_dir = TempDir::new().expect("temp dir");
    let path = temp_dir.path().join("empty.py");
    test_fs::write(&path, "").expect("write fixture");
    let uri = Url::from_file_path(&path).expect("file uri").to_string();
    let payload = serde_json::json!({
        "command": {"domain": "observe", "operation": "get-card"},
        "arguments": ["--uri", uri, "--position", "1:1"],
    });
    let json = serde_json::to_vec(&payload).expect("serialise test request");
    let request = CommandRequest::parse(&json).expect("test request");
    let mut output = Vec::new();
    let mut writer = ResponseWriter::new(&mut output);
    let result = router
        .route(&request, &mut writer, &mut backends)
        .expect("route");
    assert_eq!(result.status, 1);

    let response = String::from_utf8(output).expect("utf8");
    let stream_line = response.lines().next().expect("stream line");
    let envelope: serde_json::Value = serde_json::from_str(stream_line).expect("parse envelope");
    assert_eq!(envelope["kind"], "stream");
    assert_eq!(envelope["stream"], "stdout");

    let data_str = envelope["data"].as_str().expect("data string");
    let card: serde_json::Value = serde_json::from_str(data_str).expect("parse card");
    assert_eq!(card["status"], "refusal");
    assert_eq!(card["refusal"]["reason"], "position_out_of_range");
    assert_eq!(card["refusal"]["requested_detail"], "structure");
}

#[rstest]
fn find_references_not_implemented(mut backends: FusionBackends<SemanticBackendProvider>) {
    let router = build_router().expect("router with absolute workspace root");
    let request = make_request("observe", "find-references").expect("test request");
    let mut output = Vec::new();
    let mut writer = ResponseWriter::new(&mut output);
    let result = router
        .route(&request, &mut writer, &mut backends)
        .expect("route");
    assert_eq!(result.status, 1);

    let response = String::from_utf8(output).expect("utf8");
    assert!(response.contains("not yet implemented"));
}
