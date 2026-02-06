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

    fn route_observe<W: Write>(
        &self,
        request: &CommandRequest,
        writer: &mut ResponseWriter<W>,
        backends: &mut FusionBackends<SemanticBackendProvider>,
    ) -> Result<DispatchResult, DispatchError> {
        let operation = request.operation().to_ascii_lowercase();
        match operation.as_str() {
            "get-definition" => observe::get_definition::handle(request, writer, backends),
            _ => self.route_fallback(&DomainRoutingContext::OBSERVE, operation.as_str(), writer),
        }
    }

    fn route_act<W: Write>(
        &self,
        request: &CommandRequest,
        writer: &mut ResponseWriter<W>,
        backends: &mut FusionBackends<SemanticBackendProvider>,
    ) -> Result<DispatchResult, DispatchError> {
        let operation = request.operation().to_ascii_lowercase();
        match operation.as_str() {
            "apply-patch" => act::apply_patch::handle(request, writer, backends),
            _ => self.route_fallback(&DomainRoutingContext::ACT, operation.as_str(), writer),
        }
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
        self.route_fallback(context, operation.as_str(), writer)
    }

    /// Handles routing fallbacks for known-but-unimplemented and unknown operations.
    fn route_fallback<W: Write>(
        &self,
        routing: &DomainRoutingContext,
        operation: &str,
        writer: &mut ResponseWriter<W>,
    ) -> Result<DispatchResult, DispatchError> {
        if routing.known_operations.contains(&operation) {
            self.write_not_implemented(writer, routing.domain, operation)
        } else {
            Err(DispatchError::unknown_operation(
                routing.domain,
                operation.to_string(),
            ))
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
mod tests;
