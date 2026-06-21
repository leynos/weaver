//! TOML adapter for the `OrthoConfig` consumer boundary manifest.
//!
//! This module is the crate's outbound deserialization adapter. It owns every
//! Serde derive and TOML spelling detail, then converts those DTOs into the
//! plain domain structs exported by `lib.rs`. `load_manifest` and
//! `load_manifest_file` parse text through `BoundaryManifestDto`; the renderer
//! never sees these adapter types and consumes only the resulting
//! `BoundaryManifest`.

use serde::{Deserialize, Deserializer};

use crate::{BoundaryManifest, BoundaryState, BoundaryTask, UpstreamRef, UpstreamRole};

/// Deserialization shape for the complete TOML manifest.
#[derive(Debug, Deserialize)]
pub(super) struct BoundaryManifestDto {
    pub(super) schema_version: u32,
    pub(super) managed_tasks: Vec<String>,
    #[serde(rename = "task")]
    pub(super) tasks: Vec<BoundaryTaskDto>,
}

/// Deserialization shape for one `[[task]]` TOML row.
#[derive(Debug, Deserialize)]
pub(super) struct BoundaryTaskDto {
    pub(super) id: String,
    pub(super) gist: String,
    pub(super) state: BoundaryStateDto,
    pub(super) upstream: Vec<UpstreamRefDto>,
    #[serde(deserialize_with = "empty_string_as_none")]
    pub(super) shipped_in: Option<String>,
    #[serde(deserialize_with = "empty_string_as_none")]
    pub(super) removal_gate: Option<String>,
    #[serde(deserialize_with = "empty_string_as_none")]
    pub(super) adr_anchor: Option<String>,
    #[serde(deserialize_with = "empty_string_as_none")]
    pub(super) next_review_by: Option<String>,
    pub(super) last_reviewed: String,
}

/// Deserialization shape for one `[[task.upstream]]` entry.
#[derive(Debug, Deserialize)]
pub(super) struct UpstreamRefDto {
    pub(super) task: String,
    pub(super) role: UpstreamRoleDto,
}

/// Serde-backed spelling of the public boundary state vocabulary.
#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum BoundaryStateDto {
    Consumes,
    Wraps,
    Pending,
    Divergent,
}

/// Serde-backed spelling of upstream role values in the manifest.
#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum UpstreamRoleDto {
    Boundary,
    Metadata,
    CapabilityProvenance,
    Vocabulary,
    Renderer,
    Profile,
    Delivery,
    Feedback,
    ExecutionLedger,
}

impl From<BoundaryManifestDto> for BoundaryManifest {
    /// Convert adapter-owned TOML data into the public domain manifest.
    fn from(dto: BoundaryManifestDto) -> Self {
        Self {
            schema_version: dto.schema_version,
            managed_tasks: dto.managed_tasks,
            tasks: dto.tasks.into_iter().map(BoundaryTask::from).collect(),
        }
    }
}

impl From<BoundaryTaskDto> for BoundaryTask {
    /// Convert one TOML task row into a public domain task.
    fn from(dto: BoundaryTaskDto) -> Self {
        Self {
            id: dto.id,
            gist: dto.gist,
            state: BoundaryState::from(dto.state),
            upstream: dto.upstream.into_iter().map(UpstreamRef::from).collect(),
            shipped_in: dto.shipped_in,
            removal_gate: dto.removal_gate,
            adr_anchor: dto.adr_anchor,
            next_review_by: dto.next_review_by,
            last_reviewed: dto.last_reviewed,
        }
    }
}

impl From<UpstreamRefDto> for UpstreamRef {
    /// Convert one TOML upstream reference into a public domain reference.
    fn from(dto: UpstreamRefDto) -> Self {
        Self {
            task: dto.task,
            role: UpstreamRole::from(dto.role),
        }
    }
}

impl From<BoundaryStateDto> for BoundaryState {
    /// Convert the adapter state vocabulary into the public state enum.
    fn from(dto: BoundaryStateDto) -> Self {
        match dto {
            BoundaryStateDto::Consumes => Self::Consumes,
            BoundaryStateDto::Wraps => Self::Wraps,
            BoundaryStateDto::Pending => Self::Pending,
            BoundaryStateDto::Divergent => Self::Divergent,
        }
    }
}

impl From<UpstreamRoleDto> for UpstreamRole {
    /// Convert the adapter role vocabulary into the public role enum.
    fn from(dto: UpstreamRoleDto) -> Self {
        match dto {
            UpstreamRoleDto::Boundary => Self::Boundary,
            UpstreamRoleDto::Metadata => Self::Metadata,
            UpstreamRoleDto::CapabilityProvenance => Self::CapabilityProvenance,
            UpstreamRoleDto::Vocabulary => Self::Vocabulary,
            UpstreamRoleDto::Renderer => Self::Renderer,
            UpstreamRoleDto::Profile => Self::Profile,
            UpstreamRoleDto::Delivery => Self::Delivery,
            UpstreamRoleDto::Feedback => Self::Feedback,
            UpstreamRoleDto::ExecutionLedger => Self::ExecutionLedger,
        }
    }
}

/// Treat empty TOML strings as absent optional manifest evidence.
fn empty_string_as_none<'de, D>(deserializer: D) -> Result<Option<String>, D::Error>
where
    D: Deserializer<'de>,
{
    let value = String::deserialize(deserializer)?;
    Ok((!value.is_empty()).then_some(value))
}
