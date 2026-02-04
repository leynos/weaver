//! Domain and operation routing for command dispatch.
//!
//! This module routes incoming requests to the appropriate domain handler based
//! on the command descriptor. Each domain (`observe`, `act`, `verify`) has its
//! own set of supported operations. Unknown domains or operations are rejected
//! with structured errors.

use std::io::Write;

use tracing::debug;

use crate::backends::FusionBackends;
use crate::semantic_provider::SemanticBackendProvider;

use super::act;
use super::errors::DispatchError;
use super::observe;
use super::request::CommandRequest;
use super::response::ResponseWriter;

/// Tracing target for dispatch operations.
pub(crate) const DISPATCH_TARGET: &str = concat!(env!("CARGO_PKG_NAME"), "::dispatch");

/// Known command domains.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Domain {
    /// Query commands for inspecting the codebase.
    Observe,
    /// Modification commands for changing the codebase.
    Act,
    /// Verification commands for checking codebase integrity.
    Verify,
}

impl Domain {
    /// Parses a domain string (case-insensitive).
    ///
    /// # Errors
    ///
    /// Returns `DispatchError::UnknownDomain` if the value does not match any
    /// known domain.
    pub fn parse(value: &str) -> Result<Self, DispatchError> {
        match value.to_ascii_lowercase().as_str() {
            "observe" => Ok(Self::Observe),
            "act" => Ok(Self::Act),
            "verify" => Ok(Self::Verify),
            _ => Err(DispatchError::unknown_domain(value)),
        }
    }

    /// Returns the canonical string representation.
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Observe => "observe",
            Self::Act => "act",
            Self::Verify => "verify",
        }
    }
}

/// Result of routing and dispatching a command.
pub struct DispatchResult {
    /// Exit status to return to the client.
    pub status: i32,
}

impl DispatchResult {
    /// Creates a successful result (status 0).
    pub const fn success() -> Self {
        Self { status: 0 }
    }

    /// Creates a result with the given status code.
    pub const fn with_status(status: i32) -> Self {
        Self { status }
    }
}

/// Context for routing operations within a domain.
pub struct DomainRoutingContext {
    pub(crate) domain: &'static str,
    pub(crate) known_operations: &'static [&'static str],
}

impl DomainRoutingContext {
    /// Routing context for the `observe` domain.
    const OBSERVE: Self = Self {
        domain: "observe",
        known_operations: &[
            "get-definition",
            "find-references",
            "grep",
            "diagnostics",
            "call-hierarchy",
        ],
    };

    /// Routing context for the `act` domain.
    const ACT: Self = Self {
        domain: "act",
        known_operations: &[
            "rename-symbol",
            "apply-edits",
            "apply-patch",
            "apply-rewrite",
            "refactor",
        ],
    };

    /// Routing context for the `verify` domain.
    const VERIFY: Self = Self {
        domain: "verify",
        known_operations: &["diagnostics", "syntax"],
    };
}

/// Request context for routing and executing domain handlers.
struct RouteContext<'a, W: Write> {
    request: &'a CommandRequest,
    writer: &'a mut ResponseWriter<W>,
    backends: &'a mut FusionBackends<SemanticBackendProvider>,
}

impl<'a, W: Write> RouteContext<'a, W> {
    fn new(
        request: &'a CommandRequest,
        writer: &'a mut ResponseWriter<W>,
        backends: &'a mut FusionBackends<SemanticBackendProvider>,
    ) -> Self {
        Self {
            request,
            writer,
            backends,
        }
    }
}

/// Routes commands to domain handlers.
///
/// The router parses the domain from the request, validates the operation, and
/// delegates to the appropriate handler. MVP handlers return "not implemented"
/// responses for all known operations.
#[derive(Debug, Default)]
pub struct DomainRouter;

impl DomainRouter {
    /// Creates a new domain router.
    pub const fn new() -> Self {
        Self
    }

    /// Routes a command request to the appropriate domain handler.
    ///
    /// # Errors
    ///
    /// Returns an error if the domain or operation is unknown.
    pub fn route<W: Write>(
        &self,
        request: &CommandRequest,
        writer: &mut ResponseWriter<W>,
        backends: &mut FusionBackends<SemanticBackendProvider>,
    ) -> Result<DispatchResult, DispatchError> {
        let domain = Domain::parse(request.domain())?;

        debug!(
            target: DISPATCH_TARGET,
            domain = domain.as_str(),
            operation = request.operation(),
            "routing command"
        );

        match domain {
            Domain::Observe => self.route_observe(request, writer, backends),
            Domain::Act => self.route_act(request, writer, backends),
            Domain::Verify => self.route_verify(request, writer),
        }
    }

    /// Generic routing for domains with specific operation handlers.
    fn route_with_handlers<W: Write, F>(
        &self,
        request: &CommandRequest,
        writer: &mut ResponseWriter<W>,
        backends: &mut FusionBackends<SemanticBackendProvider>,
        routing: &DomainRoutingContext,
        handler: F,
    ) -> Result<DispatchResult, DispatchError>
    where
        F: FnOnce(&str, &mut RouteContext<'_, W>) -> Option<Result<DispatchResult, DispatchError>>,
    {
        let mut context = RouteContext::new(request, writer, backends);
        let operation = context.request.operation().to_ascii_lowercase();

        if let Some(result) = handler(operation.as_str(), &mut context) {
            return result;
        }

        if routing.known_operations.contains(&operation.as_str()) {
            return self.write_not_implemented(context.writer, routing.domain, &operation);
        }

        Err(DispatchError::unknown_operation(routing.domain, operation))
    }

    fn route_observe<W: Write>(
        &self,
        request: &CommandRequest,
        writer: &mut ResponseWriter<W>,
        backends: &mut FusionBackends<SemanticBackendProvider>,
    ) -> Result<DispatchResult, DispatchError> {
        self.route_with_handlers(
            request,
            writer,
            backends,
            &DomainRoutingContext::OBSERVE,
            |operation, context| match operation {
                "get-definition" => Some(observe::get_definition::handle(
                    context.request,
                    context.writer,
                    context.backends,
                )),
                _ => None,
            },
        )
    }

    fn route_act<W: Write>(
        &self,
        request: &CommandRequest,
        writer: &mut ResponseWriter<W>,
        backends: &mut FusionBackends<SemanticBackendProvider>,
    ) -> Result<DispatchResult, DispatchError> {
        self.route_with_handlers(
            request,
            writer,
            backends,
            &DomainRoutingContext::ACT,
            |operation, context| match operation {
                "apply-patch" => Some(act::apply_patch::handle(
                    context.request,
                    context.writer,
                    context.backends,
                )),
                _ => None,
            },
        )
    }

    fn route_verify<W: Write>(
        &self,
        request: &CommandRequest,
        writer: &mut ResponseWriter<W>,
    ) -> Result<DispatchResult, DispatchError> {
        self.route_domain(request, writer, &DomainRoutingContext::VERIFY)
    }

    fn route_domain<W: Write>(
        &self,
        request: &CommandRequest,
        writer: &mut ResponseWriter<W>,
        context: &DomainRoutingContext,
    ) -> Result<DispatchResult, DispatchError> {
        let operation = request.operation().to_ascii_lowercase();
        if context.known_operations.contains(&operation.as_str()) {
            self.write_not_implemented(writer, context.domain, &operation)
        } else {
            Err(DispatchError::unknown_operation(context.domain, operation))
        }
    }

    fn write_not_implemented<W: Write>(
        &self,
        writer: &mut ResponseWriter<W>,
        domain: &str,
        operation: &str,
    ) -> Result<DispatchResult, DispatchError> {
        writer.write_stderr(format!(
            "{domain} {operation}: operation not yet implemented\n"
        ))?;
        Ok(DispatchResult::with_status(1))
    }
}

#[cfg(test)]
mod tests {
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

    #[fixture]
    fn backends() -> FusionBackends<SemanticBackendProvider> {
        let config = Config {
            daemon_socket: SocketEndpoint::unix("/tmp/weaver-test/socket.sock"),
            ..Config::default()
        };
        let provider = SemanticBackendProvider::new(CapabilityMatrix::default());
        FusionBackends::new(config, provider)
    }

    /// Creates backends for tests that iterate (can't use fixtures in loops).
    fn create_backends() -> FusionBackends<SemanticBackendProvider> {
        let config = Config {
            daemon_socket: SocketEndpoint::unix("/tmp/weaver-test/socket.sock"),
            ..Config::default()
        };
        let provider = SemanticBackendProvider::new(CapabilityMatrix::default());
        FusionBackends::new(config, provider)
    }

    fn assert_routes_operations(domain: &str, operations: &[&str]) {
        let router = DomainRouter::new();
        for op in operations {
            let request = make_request(domain, op);
            let mut output = Vec::new();
            let mut writer = ResponseWriter::new(&mut output);
            let mut backends = create_backends();
            let result = router.route(&request, &mut writer, &mut backends);
            // get-definition requires --uri/--position args, so it will fail
            // with InvalidArguments when called without them, but this still
            // proves the operation is recognized and routed correctly
            if domain == "observe" && *op == "get-definition" {
                assert!(
                    matches!(result, Err(DispatchError::InvalidArguments { .. })),
                    "{domain} {op} should fail with InvalidArguments (no args provided)"
                );
            } else if domain == "act" && *op == "apply-patch" {
                assert!(
                    matches!(result, Err(DispatchError::InvalidArguments { .. })),
                    "{domain} {op} should fail with InvalidArguments (missing patch)"
                );
            } else {
                assert!(result.is_ok(), "{domain} {op} should route successfully");
            }
        }
    }

    fn assert_rejects_unknown_operation(domain: &str, operation: &str) {
        let router = DomainRouter::new();
        let request = make_request(domain, operation);
        let mut output = Vec::new();
        let mut writer = ResponseWriter::new(&mut output);
        let mut backends = create_backends();
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

    #[test]
    fn routes_known_observe_operations() {
        assert_routes_operations("observe", DomainRoutingContext::OBSERVE.known_operations);
    }

    #[test]
    fn routes_known_act_operations() {
        assert_routes_operations("act", DomainRoutingContext::ACT.known_operations);
    }

    #[test]
    fn routes_known_verify_operations() {
        assert_routes_operations("verify", DomainRoutingContext::VERIFY.known_operations);
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
        let router = DomainRouter::new();
        let request = make_request(domain, operation);
        let mut output = Vec::new();
        let mut writer = ResponseWriter::new(&mut output);
        let result = router.route(&request, &mut writer, &mut backends);
        if domain.eq_ignore_ascii_case("act") && operation.eq_ignore_ascii_case("apply-patch") {
            assert!(
                matches!(result, Err(DispatchError::InvalidArguments { .. })),
                "{domain} {operation} should fail with InvalidArguments (missing patch)"
            );
        } else {
            assert!(
                result.is_ok(),
                "{domain} {operation} should route successfully despite case"
            );
        }
    }

    #[rstest]
    fn get_definition_requires_arguments(mut backends: FusionBackends<SemanticBackendProvider>) {
        let router = DomainRouter::new();
        let request = make_request("observe", "get-definition");
        let mut output = Vec::new();
        let mut writer = ResponseWriter::new(&mut output);
        let result = router.route(&request, &mut writer, &mut backends);

        // Should fail with InvalidArguments because no --uri/--position
        assert!(matches!(
            result,
            Err(DispatchError::InvalidArguments { .. })
        ));
    }

    #[rstest]
    fn find_references_not_implemented(mut backends: FusionBackends<SemanticBackendProvider>) {
        let router = DomainRouter::new();
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
}
