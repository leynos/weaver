//! Wire-protocol type definitions for the weaver daemon/CLI boundary.
//!
//! This crate provides shared type definitions for the JSONL protocol used
//! between the `weaverd` daemon and `weaver` CLI. These types ensure that
//! serialisation on the daemon side and deserialisation on the CLI side
//! remain in sync.
//!
//! ## Stability
//!
//! All types in this crate form part of the wire protocol and must maintain
//! backwards compatibility. Breaking changes require protocol versioning.

use serde::Deserialize;

/// Wire-protocol discriminator for unknown-operation error payloads.
///
/// This constant is part of the JSONL protocol contract between the daemon
/// and CLI. It must remain stable across releases.
pub const UNKNOWN_OPERATION_TYPE: &str = "UnknownOperation";

/// Unknown-operation error payload emitted by the daemon.
///
/// This type is used by the CLI for deserialisation. The daemon uses its own
/// serialisation-optimised types with borrowed string slices.
#[derive(Debug, Deserialize)]
pub struct UnknownOperationPayload {
    /// Payload type discriminator.
    #[serde(rename = "type")]
    pub r#type: String,

    /// Structured error details.
    pub details: UnknownOperationDetails,
}

/// Inner details for an unknown-operation error payload.
///
/// This type is used by the CLI for deserialisation. The daemon uses its own
/// serialisation-optimised types with borrowed string slices.
#[derive(Debug, Deserialize)]
pub struct UnknownOperationDetails {
    /// Routed domain containing the unknown operation.
    pub domain: String,

    /// Unknown operation requested by the client.
    pub operation: String,

    /// Canonical known operations for the routed domain.
    pub known_operations: Vec<String>,
}
