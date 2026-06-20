//! TOML adapter for the `OrthoConfig` consumer boundary manifest.

use serde::{Deserialize, Deserializer};

use crate::{BoundaryManifest, BoundaryState, BoundaryTask, UpstreamRef, UpstreamRole};

#[derive(Debug, Deserialize)]
pub(super) struct BoundaryManifestDto {
    pub(super) schema_version: u32,
    pub(super) managed_tasks: Vec<String>,
    #[serde(rename = "task")]
    pub(super) tasks: Vec<BoundaryTaskDto>,
}

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

#[derive(Debug, Deserialize)]
pub(super) struct UpstreamRefDto {
    pub(super) task: String,
    pub(super) role: UpstreamRoleDto,
}

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum BoundaryStateDto {
    Consumes,
    Wraps,
    Pending,
    Divergent,
}

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
    fn from(dto: BoundaryManifestDto) -> Self {
        Self {
            schema_version: dto.schema_version,
            managed_tasks: dto.managed_tasks,
            tasks: dto.tasks.into_iter().map(BoundaryTask::from).collect(),
        }
    }
}

impl From<BoundaryTaskDto> for BoundaryTask {
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
    fn from(dto: UpstreamRefDto) -> Self {
        Self {
            task: dto.task,
            role: UpstreamRole::from(dto.role),
        }
    }
}

impl From<BoundaryStateDto> for BoundaryState {
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

fn empty_string_as_none<'de, D>(deserializer: D) -> Result<Option<String>, D::Error>
where
    D: Deserializer<'de>,
{
    let value = String::deserialize(deserializer)?;
    Ok((!value.is_empty()).then_some(value))
}
