//! Capability contract for the `rename-symbol` actuator operation.
//!
//! This module defines the typed request schema and validation rules
//! for rename-symbol. A valid request must provide `uri` (file URI),
//! `position` (line:col or byte offset), and `new_name` (the
//! replacement identifier). A valid successful response must contain
//! [`PluginOutput::Diff`] output.

use std::collections::HashMap;

use crate::capability::{CapabilityContract, CapabilityId, ContractVersion};
use crate::error::PluginError;
use crate::protocol::{PluginOutput, PluginRequest, PluginResponse};

/// Contract version for `rename-symbol` v1.0.
pub const RENAME_SYMBOL_CONTRACT_VERSION: ContractVersion = ContractVersion::new(1, 0);

/// Typed request fields for a `rename-symbol` operation.
///
/// This struct represents the validated, typed view of the arguments
/// that a `rename-symbol` request must contain. It is extracted from
/// the generic [`PluginRequest::arguments()`] map during validation.
///
/// # Example
///
/// ```
/// use weaver_plugins::capability::RenameSymbolRequest;
///
/// let request = RenameSymbolRequest::new(
///     "file:///project/src/main.py",
///     "10:5",
///     "new_function_name",
/// );
/// assert_eq!(request.uri(), "file:///project/src/main.py");
/// assert_eq!(request.new_name(), "new_function_name");
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RenameSymbolRequest {
    uri: String,
    position: String,
    new_name: String,
}

impl RenameSymbolRequest {
    /// Creates a new typed rename-symbol request.
    #[must_use]
    pub fn new(
        uri: impl Into<String>,
        position: impl Into<String>,
        new_name: impl Into<String>,
    ) -> Self {
        Self {
            uri: uri.into(),
            position: position.into(),
            new_name: new_name.into(),
        }
    }

    /// Returns the file URI.
    #[must_use]
    pub fn uri(&self) -> &str {
        &self.uri
    }

    /// Returns the position string (e.g., "10:5" or byte offset).
    #[must_use]
    pub fn position(&self) -> &str {
        &self.position
    }

    /// Returns the new symbol name.
    #[must_use]
    pub fn new_name(&self) -> &str {
        &self.new_name
    }

    /// Extracts and validates a [`RenameSymbolRequest`] from generic
    /// plugin request arguments.
    ///
    /// # Errors
    ///
    /// Returns [`PluginError::InvalidOutput`] if required fields are
    /// missing or have invalid types.
    pub fn extract(request: &PluginRequest) -> Result<Self, PluginError> {
        let args = request.arguments();

        let uri = extract_string_field(args, "uri")?;
        let position = extract_string_field(args, "position")?;
        let new_name = extract_string_field(args, "new_name")?;

        if new_name.trim().is_empty() {
            return Err(PluginError::InvalidOutput {
                name: String::from("rename-symbol"),
                message: String::from("rename-symbol contract requires 'new_name' to be non-empty"),
            });
        }

        Ok(Self {
            uri,
            position,
            new_name,
        })
    }
}

/// Extracts a required string field from the arguments map.
fn extract_string_field(
    args: &HashMap<String, serde_json::Value>,
    field: &str,
) -> Result<String, PluginError> {
    let value = args.get(field).ok_or_else(|| PluginError::InvalidOutput {
        name: String::from("rename-symbol"),
        message: format!("rename-symbol contract requires '{field}' argument"),
    })?;

    value
        .as_str()
        .map(String::from)
        .ok_or_else(|| PluginError::InvalidOutput {
            name: String::from("rename-symbol"),
            message: format!("rename-symbol contract requires '{field}' to be a string",),
        })
}

/// Validates that a successful response contains diff output.
fn validate_success_output(response: &PluginResponse) -> Result<(), PluginError> {
    if !response.is_success() {
        // Failed responses are valid refusals; the contract does not
        // constrain the output variant on failure.
        return Ok(());
    }

    match response.output() {
        PluginOutput::Diff { .. } => Ok(()),
        other => Err(PluginError::InvalidOutput {
            name: String::from("rename-symbol"),
            message: format!(
                "rename-symbol contract requires successful responses to \
                 contain diff output, got {other:?}",
            ),
        }),
    }
}

/// Contract validator for the `rename-symbol` capability.
///
/// # Example
///
/// ```
/// use weaver_plugins::capability::{
///     CapabilityContract, CapabilityId, RenameSymbolContract,
/// };
///
/// let contract = RenameSymbolContract;
/// assert_eq!(contract.capability_id(), CapabilityId::RenameSymbol);
/// ```
#[derive(Debug, Clone, Copy)]
pub struct RenameSymbolContract;

impl CapabilityContract for RenameSymbolContract {
    fn capability_id(&self) -> CapabilityId {
        CapabilityId::RenameSymbol
    }

    fn version(&self) -> ContractVersion {
        RENAME_SYMBOL_CONTRACT_VERSION
    }

    fn validate_request(&self, request: &PluginRequest) -> Result<(), PluginError> {
        RenameSymbolRequest::extract(request).map(|_| ())
    }

    fn validate_response(&self, response: &PluginResponse) -> Result<(), PluginError> {
        validate_success_output(response)
    }
}
