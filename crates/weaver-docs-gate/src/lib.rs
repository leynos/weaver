//! Tooling for the `OrthoConfig` consumer boundary matrix.
//!
//! Public domain types and TOML deserialization live in this root module.
//! `loader` reads manifests and emits diagnostics; `renderer` turns a validated
//! [`BoundaryManifest`] into the checked-in Markdown matrix. Filesystem access
//! is limited to [`load_manifest_file`]; callers with manifest bytes use
//! [`load_manifest`] so parsing stays independent of path handling.

use camino::Utf8PathBuf;
use serde::{Deserialize, Deserializer};

mod loader;
mod renderer;
pub use loader::{load_manifest, load_manifest_file};
pub use renderer::render_matrix;

/// One boundary classification state for a Weaver roadmap task.
///
/// # Examples
/// ```
/// use weaver_docs_gate::BoundaryState;
///
/// assert_eq!(BoundaryState::Wraps.as_str(), "wraps");
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BoundaryState {
    /// Weaver follows an `OrthoConfig` contract that has shipped.
    Consumes,
    /// Weaver uses a temporary local adapter with a removal gate.
    Wraps,
    /// Weaver waits for an upstream contract whose shape is undecided.
    Pending,
    /// Weaver deliberately keeps a different contract.
    Divergent,
}

impl BoundaryState {
    /// Return the manifest spelling for the state.
    ///
    /// # Examples
    /// ```
    /// use weaver_docs_gate::BoundaryState;
    ///
    /// assert_eq!(BoundaryState::Consumes.as_str(), "consumes");
    /// ```
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Consumes => "consumes",
            Self::Wraps => "wraps",
            Self::Pending => "pending",
            Self::Divergent => "divergent",
        }
    }
}

/// The upstream `OrthoConfig` role that a Weaver task consumes or waits for.
///
/// # Examples
/// ```
/// use weaver_docs_gate::UpstreamRole;
///
/// assert_eq!(UpstreamRole::Renderer.as_str(), "renderer");
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UpstreamRole {
    /// Consumer-boundary ownership and governance.
    Boundary,
    /// Recursive command metadata.
    Metadata,
    /// Capability and provider provenance metadata.
    CapabilityProvenance,
    /// Canonical command vocabulary.
    Vocabulary,
    /// Human and machine renderer contracts.
    Renderer,
    /// Profile parsing, precedence, and redaction.
    Profile,
    /// Delivery sink contracts.
    Delivery,
    /// Feedback command contracts.
    Feedback,
    /// Execution ledger contracts.
    ExecutionLedger,
}

impl UpstreamRole {
    /// Return the manifest spelling for the role.
    ///
    /// # Examples
    /// ```
    /// use weaver_docs_gate::UpstreamRole;
    ///
    /// assert_eq!(UpstreamRole::Boundary.as_str(), "boundary");
    /// ```
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Boundary => "boundary",
            Self::Metadata => "metadata",
            Self::CapabilityProvenance => "capability_provenance",
            Self::Vocabulary => "vocabulary",
            Self::Renderer => "renderer",
            Self::Profile => "profile",
            Self::Delivery => "delivery",
            Self::Feedback => "feedback",
            Self::ExecutionLedger => "execution_ledger",
        }
    }
}

/// A single upstream `OrthoConfig` task reference.
///
/// # Examples
/// ```
/// use weaver_docs_gate::{UpstreamRef, UpstreamRole};
///
/// let upstream = UpstreamRef {
///     task: "ortho-config:renderer-contract".into(),
///     role: UpstreamRole::Renderer,
/// };
/// assert_eq!(upstream.role.as_str(), "renderer");
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct UpstreamRef {
    /// The upstream roadmap task or stable design section.
    pub task: String,
    /// The role that upstream reference plays for the Weaver task.
    pub role: UpstreamRole,
}

/// One classified Weaver roadmap task.
///
/// # Examples
/// ```
/// use weaver_docs_gate::{BoundaryState, BoundaryTask};
///
/// let task = BoundaryTask {
///     id: "12.1.1".into(),
///     gist: "Track the downstream consumer boundary.".into(),
///     state: BoundaryState::Pending,
///     upstream: Vec::new(),
///     shipped_in: None,
///     removal_gate: None,
///     adr_anchor: None,
///     next_review_by: Some("2026-12-31".into()),
///     last_reviewed: "2026-06-20".into(),
/// };
/// assert_eq!(task.state.as_str(), "pending");
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct BoundaryTask {
    /// Weaver roadmap task ID, such as `13.1.2`.
    pub id: String,
    /// One-sentence task summary.
    pub gist: String,
    /// Boundary classification state.
    pub state: BoundaryState,
    /// Upstream `OrthoConfig` task references.
    pub upstream: Vec<UpstreamRef>,
    /// `OrthoConfig` release tag or pinned SHA for shipped contracts.
    #[serde(deserialize_with = "empty_string_as_none")]
    pub shipped_in: Option<String>,
    /// Replacement condition for temporary wrappers.
    #[serde(deserialize_with = "empty_string_as_none")]
    pub removal_gate: Option<String>,
    /// ADR 007 heading slug for deliberate divergences.
    #[serde(deserialize_with = "empty_string_as_none")]
    pub adr_anchor: Option<String>,
    /// ISO-8601 review date for pending contracts.
    #[serde(deserialize_with = "empty_string_as_none")]
    pub next_review_by: Option<String>,
    /// ISO-8601 date when the row was last reviewed.
    pub last_reviewed: String,
}

/// The complete boundary manifest.
///
/// # Examples
/// ```
/// use weaver_docs_gate::BoundaryManifest;
///
/// let manifest = BoundaryManifest {
///     schema_version: 1,
///     managed_tasks: Vec::new(),
///     tasks: Vec::new(),
/// };
/// assert!(manifest.tasks.is_empty());
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct BoundaryManifest {
    /// Manifest schema version.
    pub schema_version: u32,
    /// Ordered registry of Weaver roadmap task IDs governed by the matrix.
    pub managed_tasks: Vec<String>,
    /// Classified task rows.
    #[serde(rename = "task")]
    pub tasks: Vec<BoundaryTask>,
}

/// Errors returned while parsing the boundary manifest.
///
/// # Examples
/// ```
/// use weaver_docs_gate::BoundaryError;
///
/// let error = BoundaryError::InvalidSchema {
///     detail: "missing field `schema_version`".into(),
/// };
/// assert!(
///     error
///         .to_string()
///         .contains("invalid boundary manifest schema")
/// );
/// ```
#[derive(Debug, thiserror::Error)]
pub enum BoundaryError {
    /// Manifest bytes could not be read from the supplied reader.
    #[error("boundary manifest cannot be read: {detail}")]
    Unreadable {
        /// Stable human-readable read failure detail.
        detail: String,
    },
    /// The manifest contents do not match the boundary manifest schema.
    #[error("invalid boundary manifest schema: {detail}")]
    InvalidSchema {
        /// Stable human-readable schema failure detail.
        detail: String,
    },
}

/// Errors returned while loading a boundary manifest from a file.
///
/// # Examples
/// ```
/// use camino::Utf8PathBuf;
/// use weaver_docs_gate::BoundaryFileError;
///
/// let error = BoundaryFileError::InvalidSchema {
///     path: Utf8PathBuf::from("docs/orthoconfig-consumer-boundary.toml"),
///     detail: "missing field `schema_version`".into(),
/// };
/// assert!(
///     error
///         .to_string()
///         .contains("invalid boundary manifest schema")
/// );
/// ```
#[derive(Debug, thiserror::Error)]
pub enum BoundaryFileError {
    /// The manifest path does not exist.
    #[error("manifest file not found: {0}")]
    NotFound(Utf8PathBuf),
    /// The manifest path cannot be opened through a parent directory handle.
    #[error("invalid manifest path: {0}")]
    InvalidPath(Utf8PathBuf),
    /// The manifest exists but cannot be read as file contents.
    #[error("boundary manifest cannot be read: {path}: {detail}")]
    Unreadable {
        /// Manifest path.
        path: Utf8PathBuf,
        /// Stable human-readable read failure detail.
        detail: String,
    },
    /// The manifest contents do not match the boundary manifest schema.
    #[error("invalid boundary manifest schema in {path}: {detail}")]
    InvalidSchema {
        /// Manifest path.
        path: Utf8PathBuf,
        /// Stable human-readable schema failure detail.
        detail: String,
    },
}

/// Treat empty TOML strings as absent optional manifest evidence.
fn empty_string_as_none<'de, D>(deserializer: D) -> Result<Option<String>, D::Error>
where
    D: Deserializer<'de>,
{
    let value = String::deserialize(deserializer)?;
    Ok((!value.is_empty()).then_some(value))
}
