//! Per-call context threaded through the host's capability gate.
//!
//! Every host call needs the language whose session will serve it and the
//! operation name recorded on emitted errors; capability-gated calls also name
//! the capability that must be enabled. Bundling the three keeps the gated call
//! sites from repeating the same argument triple.

use crate::{capability::CapabilityKind, errors::HostOperation, language::Language};

/// Per-call context used to locate the session and enforce capabilities.
pub(super) struct CallContext {
    /// Language whose session handles the call.
    pub(super) language: Language,
    /// Operation recorded in emitted errors.
    pub(super) operation: HostOperation,
    /// Capability that must be enabled, when the call is capability-gated.
    pub(super) capability: Option<CapabilityKind>,
}

impl CallContext {
    /// Builds a context for calls that do not require a capability.
    pub(super) const fn for_operation(language: Language, operation: HostOperation) -> Self {
        Self {
            language,
            operation,
            capability: None,
        }
    }

    /// Builds a context for calls gated behind `capability`.
    pub(super) const fn with_capability(
        language: Language,
        operation: HostOperation,
        capability: CapabilityKind,
    ) -> Self {
        Self {
            language,
            operation,
            capability: Some(capability),
        }
    }
}

/// The capability gate and audit operation for one host call.
pub(super) struct CallSpec {
    /// Capability that must be enabled before the call is dispatched.
    pub(super) capability: CapabilityKind,
    /// Operation recorded in emitted errors.
    pub(super) operation: HostOperation,
}
