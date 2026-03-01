//! Capability contract system for actuator plugins.
//!
//! A capability contract defines the schema and validation rules for a
//! specific refactoring operation. The broker uses contracts to verify
//! that plugin requests and responses conform to expected shapes before
//! and after plugin execution.
//!
//! Each capability is identified by a [`CapabilityId`] and versioned
//! with a [`ContractVersion`]. The [`CapabilityContract`] trait provides
//! the validation interface that concrete contracts implement.

pub mod reason_code;
pub mod rename_symbol;

#[cfg(test)]
mod tests;

use crate::error::PluginError;
use crate::protocol::{PluginRequest, PluginResponse};

pub use self::reason_code::ReasonCode;
pub use self::rename_symbol::{
    RENAME_SYMBOL_CONTRACT_VERSION, RenameSymbolContract, RenameSymbolRequest,
};

// ---------------------------------------------------------------------------
// CapabilityId
// ---------------------------------------------------------------------------

/// Identifies a specific refactoring capability in the Weaver plugin model.
///
/// Capability IDs are stable identifiers used for routing, manifest
/// declarations, and contract lookup. The set is defined by
/// ADR 001 and extended only through architectural review.
///
/// # Example
///
/// ```
/// use weaver_plugins::capability::CapabilityId;
///
/// let id = CapabilityId::RenameSymbol;
/// assert_eq!(id.as_str(), "rename-symbol");
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum CapabilityId {
    /// Rename a symbol across all references.
    RenameSymbol,
    /// Move a symbol to a different module or file.
    ExtricateSymbol,
    /// Extract a block of code into a new method or function.
    ExtractMethod,
    /// Replace the body of a function while preserving its signature.
    ReplaceBody,
    /// Extract a boolean expression into a named predicate function.
    ExtractPredicate,
}

impl CapabilityId {
    /// Returns the canonical kebab-case string for this capability.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::RenameSymbol => "rename-symbol",
            Self::ExtricateSymbol => "extricate-symbol",
            Self::ExtractMethod => "extract-method",
            Self::ReplaceBody => "replace-body",
            Self::ExtractPredicate => "extract-predicate",
        }
    }
}

impl std::fmt::Display for CapabilityId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

// ---------------------------------------------------------------------------
// ContractVersion
// ---------------------------------------------------------------------------

/// Version of a capability contract for compatibility negotiation.
///
/// A major version bump indicates a breaking schema change. A minor
/// version bump indicates an additive, backwards-compatible change.
/// Two versions are compatible when they share the same major number.
///
/// # Example
///
/// ```
/// use weaver_plugins::capability::ContractVersion;
///
/// let v = ContractVersion::new(1, 0);
/// assert_eq!(v.major(), 1);
/// assert!(v.is_compatible_with(&ContractVersion::new(1, 2)));
/// assert!(!v.is_compatible_with(&ContractVersion::new(2, 0)));
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ContractVersion {
    major: u16,
    minor: u16,
}

impl ContractVersion {
    /// Creates a new contract version.
    #[must_use]
    pub const fn new(major: u16, minor: u16) -> Self {
        Self { major, minor }
    }

    /// Returns the major version number.
    #[must_use]
    pub const fn major(self) -> u16 {
        self.major
    }

    /// Returns the minor version number.
    #[must_use]
    pub const fn minor(self) -> u16 {
        self.minor
    }

    /// Returns `true` if `other` is compatible (same major version).
    #[must_use]
    pub const fn is_compatible_with(self, other: &Self) -> bool {
        self.major == other.major
    }
}

impl std::fmt::Display for ContractVersion {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}.{}", self.major, self.minor)
    }
}

// ---------------------------------------------------------------------------
// CapabilityContract trait
// ---------------------------------------------------------------------------

/// Validation interface for a capability contract.
///
/// Each capability (e.g., `rename-symbol`) has a concrete implementation
/// that validates request arguments and response shapes. The broker calls
/// these methods before sending a request to the plugin and after
/// receiving the response.
pub trait CapabilityContract {
    /// Returns the capability ID this contract validates.
    fn capability_id(&self) -> CapabilityId;

    /// Returns the contract version.
    fn version(&self) -> ContractVersion;

    /// Validates that a plugin request conforms to the contract schema.
    ///
    /// # Errors
    ///
    /// Returns [`PluginError::InvalidOutput`] if the request does not
    /// conform to the expected schema.
    fn validate_request(&self, request: &PluginRequest) -> Result<(), PluginError>;

    /// Validates that a plugin response conforms to the contract schema.
    ///
    /// # Errors
    ///
    /// Returns [`PluginError::InvalidOutput`] if a successful response
    /// does not conform to the expected output schema.
    fn validate_response(&self, response: &PluginResponse) -> Result<(), PluginError>;
}
