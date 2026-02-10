//! Router behaviour tests for command dispatch.

use std::path::PathBuf;

use rstest::{fixture, rstest};
use weaver_config::{CapabilityMatrix, Config, SocketEndpoint};

use super::*;
use crate::dispatch::request::CommandRequest;

fn make_request(domain: &str, operation: &str) -> CommandRequest {
    let json = format!(
        r#"{{"command":{{"domain":"{}","operation":"{}"}}}}"#,
        domain, operation
    );
    CommandRequest::parse(json.as_bytes()).expect("test request")
}

fn build_backends() -> FusionBackends<SemanticBackendProvider> {
    let config = Config {
        daemon_socket: SocketEndpoint::unix("/tmp/weaver-test/socket.sock"),
        ..Config::default()
    };
    let provider = SemanticBackendProvider::new(CapabilityMatrix::default());
    FusionBackends::new(config, provider)
}

fn build_router() -> DomainRouter {
    DomainRouter::new(PathBuf::from("/tmp/weaver-test-workspace"))
}

#[fixture]
fn backends() -> FusionBackends<SemanticBackendProvider> {
    build_backends()
}

fn invalid_arguments_message(domain: &str, operation: &str) -> Option<&'static str> {
    match (domain, operation) {
        ("observe", "get-definition") => {
            Some("observe get-definition should fail with InvalidArguments (no args provided)")
        }
        ("act", "apply-patch") => {
            Some("act apply-patch should fail with InvalidArguments (missing patch)")
        }
        ("act", "refactor") => {
            Some("act refactor should fail with InvalidArguments (missing --provider)")
        }
        _ => None,
    }
}

fn assert_routes_operations(domain: &str, operations: &[&str]) {
    let router = build_router();
    for op in operations {
        let request = make_request(domain, op);
        let mut output = Vec::new();
        let mut writer = ResponseWriter::new(&mut output);
        let mut backends = build_backends();
        let result = router.route(&request, &mut writer, &mut backends);
        // get-definition requires --uri/--position args, so it will fail
        // with InvalidArguments when called without them, but this still
        // proves the operation is recognized and routed correctly
        if let Some(message) = invalid_arguments_message(domain, op) {
            assert!(
                matches!(result, Err(DispatchError::InvalidArguments { .. })),
                "{message}"
            );
        } else {
            assert!(result.is_ok(), "{domain} {op} should route successfully");
        }
    }
}

fn assert_rejects_unknown_operation(domain: &str, operation: &str) {
    let router = build_router();
    let request = make_request(domain, operation);
    let mut output = Vec::new();
    let mut writer = ResponseWriter::new(&mut output);
    let mut backends = build_backends();
    let result = router.route(&request, &mut writer, &mut backends);
    assert!(matches!(
        result,
        Err(DispatchError::UnknownOperation { .. })
    ));
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
fn routes_known_operations(#[case] domain: &str, #[case] operations: &'static [&'static str]) {
    assert_routes_operations(domain, operations);
}

#[rstest]
#[case::observe("observe", "nonexistent")]
#[case::act("act", "bogus")]
#[case::verify("verify", "unknown")]
fn rejects_unknown_operation(#[case] domain: &str, #[case] operation: &str) {
    assert_rejects_unknown_operation(domain, operation);
}

#[rstest]
#[case::title("act", "Apply-Patch")]
#[case::screaming("verify", "DIAGNOSTICS")]
fn routes_operations_case_insensitively(
    #[case] domain: &str,
    #[case] operation: &str,
    mut backends: FusionBackends<SemanticBackendProvider>,
) {
    let router = build_router();
    let request = make_request(domain, operation);
    let mut output = Vec::new();
    let mut writer = ResponseWriter::new(&mut output);
    let result = router.route(&request, &mut writer, &mut backends);
    let domain_norm = domain.to_ascii_lowercase();
    let operation_norm = operation.to_ascii_lowercase();
    if let Some(message) = invalid_arguments_message(&domain_norm, &operation_norm) {
        assert!(
            matches!(result, Err(DispatchError::InvalidArguments { .. })),
            "{message}"
        );
    } else {
        assert!(
            result.is_ok(),
            "{domain} {operation} should route successfully despite case"
        );
    }
}

#[rstest]
fn find_references_not_implemented(mut backends: FusionBackends<SemanticBackendProvider>) {
    let router = build_router();
    let request = make_request("observe", "find-references");
    let mut output = Vec::new();
    let mut writer = ResponseWriter::new(&mut output);
    let result = router
        .route(&request, &mut writer, &mut backends)
        .expect("route");
    assert_eq!(result.status, 1);

    let response = String::from_utf8(output).expect("utf8");
    assert!(response.contains("not yet implemented"));
}
