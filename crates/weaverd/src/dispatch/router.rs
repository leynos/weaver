//! Domain and operation routing for command dispatch.
//!
//! This module routes incoming requests to the appropriate domain handler based
//! on the command descriptor. Each domain (`observe`, `act`, `verify`) has its
//! own set of supported operations. Unknown domains or operations are rejected
//! with structured errors.

use std::io::Write;

use tracing::debug;

use super::errors::DispatchError;
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
    pub fn as_str(&self) -> &'static str {
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
    ///
    /// Currently unused pending backend wiring, but available for operations
    /// that complete successfully.
    #[allow(dead_code)]
    pub fn success() -> Self {
        Self { status: 0 }
    }

    /// Creates a result with the given status code.
    pub fn with_status(status: i32) -> Self {
        Self { status }
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
    pub fn new() -> Self {
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
    ) -> Result<DispatchResult, DispatchError> {
        let domain = Domain::parse(request.domain())?;

        debug!(
            target: DISPATCH_TARGET,
            domain = domain.as_str(),
            operation = request.operation(),
            "routing command"
        );

        match domain {
            Domain::Observe => self.route_observe(request, writer),
            Domain::Act => self.route_act(request, writer),
            Domain::Verify => self.route_verify(request, writer),
        }
    }

    fn route_observe<W: Write>(
        &self,
        request: &CommandRequest,
        writer: &mut ResponseWriter<W>,
    ) -> Result<DispatchResult, DispatchError> {
        let operation = request.operation().to_ascii_lowercase();
        match operation.as_str() {
            "get-definition" | "find-references" | "grep" | "diagnostics" | "call-hierarchy" => {
                self.write_not_implemented(writer, "observe", &operation)
            }
            _ => Err(DispatchError::unknown_operation("observe", operation)),
        }
    }

    fn route_act<W: Write>(
        &self,
        request: &CommandRequest,
        writer: &mut ResponseWriter<W>,
    ) -> Result<DispatchResult, DispatchError> {
        let operation = request.operation().to_ascii_lowercase();
        match operation.as_str() {
            "rename-symbol" | "apply-edits" | "apply-patch" | "apply-rewrite" | "refactor" => {
                self.write_not_implemented(writer, "act", &operation)
            }
            _ => Err(DispatchError::unknown_operation("act", operation)),
        }
    }

    fn route_verify<W: Write>(
        &self,
        request: &CommandRequest,
        writer: &mut ResponseWriter<W>,
    ) -> Result<DispatchResult, DispatchError> {
        let operation = request.operation().to_ascii_lowercase();
        match operation.as_str() {
            "diagnostics" | "syntax" => self.write_not_implemented(writer, "verify", &operation),
            _ => Err(DispatchError::unknown_operation("verify", operation)),
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
    use super::*;
    use crate::dispatch::request::CommandRequest;

    fn make_request(domain: &str, operation: &str) -> CommandRequest {
        let json = format!(
            r#"{{"command":{{"domain":"{}","operation":"{}"}}}}"#,
            domain, operation
        );
        CommandRequest::parse(json.as_bytes()).expect("test request")
    }

    #[test]
    fn domain_parse_case_insensitive() {
        assert_eq!(Domain::parse("observe").unwrap(), Domain::Observe);
        assert_eq!(Domain::parse("OBSERVE").unwrap(), Domain::Observe);
        assert_eq!(Domain::parse("Observe").unwrap(), Domain::Observe);
        assert_eq!(Domain::parse("act").unwrap(), Domain::Act);
        assert_eq!(Domain::parse("ACT").unwrap(), Domain::Act);
        assert_eq!(Domain::parse("verify").unwrap(), Domain::Verify);
        assert_eq!(Domain::parse("VERIFY").unwrap(), Domain::Verify);
    }

    #[test]
    fn domain_parse_rejects_unknown() {
        let result = Domain::parse("bogus");
        assert!(matches!(result, Err(DispatchError::UnknownDomain { .. })));
    }

    #[test]
    fn routes_known_observe_operations() {
        let router = DomainRouter::new();
        for op in &["get-definition", "find-references", "grep", "diagnostics"] {
            let request = make_request("observe", op);
            let mut output = Vec::new();
            let mut writer = ResponseWriter::new(&mut output);
            let result = router.route(&request, &mut writer);
            assert!(result.is_ok(), "observe {op} should route successfully");
        }
    }

    #[test]
    fn routes_known_act_operations() {
        let router = DomainRouter::new();
        for op in &[
            "rename-symbol",
            "apply-edits",
            "apply-patch",
            "apply-rewrite",
            "refactor",
        ] {
            let request = make_request("act", op);
            let mut output = Vec::new();
            let mut writer = ResponseWriter::new(&mut output);
            let result = router.route(&request, &mut writer);
            assert!(result.is_ok(), "act {op} should route successfully");
        }
    }

    #[test]
    fn routes_known_verify_operations() {
        let router = DomainRouter::new();
        for op in &["diagnostics", "syntax"] {
            let request = make_request("verify", op);
            let mut output = Vec::new();
            let mut writer = ResponseWriter::new(&mut output);
            let result = router.route(&request, &mut writer);
            assert!(result.is_ok(), "verify {op} should route successfully");
        }
    }

    #[test]
    fn rejects_unknown_observe_operation() {
        let router = DomainRouter::new();
        let request = make_request("observe", "nonexistent");
        let mut output = Vec::new();
        let mut writer = ResponseWriter::new(&mut output);
        let result = router.route(&request, &mut writer);
        assert!(matches!(
            result,
            Err(DispatchError::UnknownOperation { .. })
        ));
    }

    #[test]
    fn rejects_unknown_act_operation() {
        let router = DomainRouter::new();
        let request = make_request("act", "bogus");
        let mut output = Vec::new();
        let mut writer = ResponseWriter::new(&mut output);
        let result = router.route(&request, &mut writer);
        assert!(matches!(
            result,
            Err(DispatchError::UnknownOperation { .. })
        ));
    }

    #[test]
    fn writes_not_implemented_message() {
        let router = DomainRouter::new();
        let request = make_request("observe", "get-definition");
        let mut output = Vec::new();
        let mut writer = ResponseWriter::new(&mut output);
        let result = router.route(&request, &mut writer).expect("route");
        assert_eq!(result.status, 1);

        let response = String::from_utf8(output).expect("utf8");
        assert!(response.contains("not yet implemented"));
    }
}
