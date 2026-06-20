//! Tooling for the `OrthoConfig` consumer boundary matrix.

use std::io::{self, ErrorKind};

use camino::{Utf8Path, Utf8PathBuf};
use cap_std::{ambient_authority, fs::Dir};

mod manifest_adapter;
mod renderer;
pub use renderer::render_matrix;

/// One boundary classification state for a Weaver roadmap task.
///
/// # Examples
/// ```
/// use weaver_docs_gate::BoundaryState;
///
/// assert_eq!(BoundaryState::Wraps.as_str(), "wraps");
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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
#[derive(Debug, Clone, PartialEq, Eq)]
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
#[derive(Debug, Clone, PartialEq, Eq)]
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
    pub shipped_in: Option<String>,
    /// Replacement condition for temporary wrappers.
    pub removal_gate: Option<String>,
    /// ADR 007 heading slug for deliberate divergences.
    pub adr_anchor: Option<String>,
    /// ISO-8601 review date for pending contracts.
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
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BoundaryManifest {
    /// Manifest schema version.
    pub schema_version: u32,
    /// Ordered registry of Weaver roadmap task IDs governed by the matrix.
    pub managed_tasks: Vec<String>,
    /// Classified task rows.
    pub tasks: Vec<BoundaryTask>,
}

/// Errors returned while loading the boundary manifest.
#[derive(Debug, thiserror::Error)]
pub enum BoundaryError {
    /// The manifest path does not exist.
    #[error("manifest file not found: {0}")]
    NotFound(Utf8PathBuf),
    /// The manifest path cannot be opened through a parent directory handle.
    #[error("invalid manifest path: {0}")]
    InvalidPath(Utf8PathBuf),
    /// The file could not be read.
    #[error("manifest file could not be read: {path}: {source}")]
    Read {
        /// Manifest path.
        path: Utf8PathBuf,
        /// Underlying I/O error.
        source: Box<io::Error>,
    },
    /// The file is not valid TOML for the boundary schema.
    #[error("invalid TOML in {path}: {source}")]
    InvalidToml {
        /// Manifest path.
        path: Utf8PathBuf,
        /// TOML parse error.
        source: Box<toml::de::Error>,
    },
}

/// Load the boundary manifest from disk.
///
/// # Errors
///
/// Returns [`BoundaryError`] when the manifest is missing, unreadable, or not
/// valid TOML for the boundary schema.
///
/// # Examples
/// ```no_run
/// use camino::Utf8Path;
/// use weaver_docs_gate::load_manifest;
///
/// let manifest = load_manifest(Utf8Path::new("docs/orthoconfig-consumer-boundary.toml"))?;
/// assert_eq!(manifest.schema_version, 1);
/// # Ok::<(), weaver_docs_gate::BoundaryError>(())
/// ```
pub fn load_manifest(path: &Utf8Path) -> Result<BoundaryManifest, BoundaryError> {
    let parent = path.parent().unwrap_or_else(|| Utf8Path::new("."));
    let file_name = path
        .file_name()
        .ok_or_else(|| BoundaryError::InvalidPath(path.to_path_buf()))?;
    let dir = Dir::open_ambient_dir(parent, ambient_authority())
        .map_err(|source| read_error(path, source))?;

    let contents = dir
        .read_to_string(file_name)
        .map_err(|source| read_error(path, source))?;
    let manifest =
        toml::from_str::<manifest_adapter::BoundaryManifestDto>(&contents).map_err(|source| {
            BoundaryError::InvalidToml {
                path: path.to_path_buf(),
                source: Box::new(source),
            }
        })?;
    Ok(manifest.into())
}

fn read_error(path: &Utf8Path, source: io::Error) -> BoundaryError {
    if source.kind() == ErrorKind::NotFound {
        BoundaryError::NotFound(path.to_path_buf())
    } else {
        BoundaryError::Read {
            path: path.to_path_buf(),
            source: Box::new(source),
        }
    }
}
