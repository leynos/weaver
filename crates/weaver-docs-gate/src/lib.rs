//! Tooling for the `OrthoConfig` consumer boundary matrix.
//!
//! This crate keeps the boundary manifest pipeline in three small pieces:
//! public domain types live in this root module, `manifest_adapter` owns the
//! TOML and Serde conversion layer, and `renderer` turns a validated
//! [`BoundaryManifest`] into the checked-in Markdown matrix. Filesystem access
//! is intentionally limited to [`load_manifest_file`]; callers that already
//! have manifest bytes should use [`load_manifest`] so parsing stays independent
//! of path handling.

use std::io::{self, ErrorKind, Read};

use camino::{Utf8Path, Utf8PathBuf};
use cap_std::{ambient_authority, fs::Dir};
use metrics::counter;
use tracing::{debug, warn};

mod manifest_adapter;
mod renderer;
pub use renderer::render_matrix;

const MANIFEST_REMEDIATION: &str = "update docs/orthoconfig-consumer-boundary.toml, regenerate \
                                    docs/orthoconfig-consumer-boundary.md, then rerun cargo test \
                                    -p weaver-docs-gate";
const METRIC_LOAD_TOTAL: &str = "weaver_docs_gate_boundary_manifest_load_total";
const OBSERVABILITY_TARGET: &str = "weaver_docs_gate::boundary_manifest";

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

/// Parse the boundary manifest from a byte reader.
///
/// # Observability
///
/// Emits `tracing` events with target
/// `weaver_docs_gate::boundary_manifest` when loading starts, succeeds, or
/// fails. Increments the
/// `weaver_docs_gate_boundary_manifest_load_total` metrics counter with
/// `source = "reader"` and an `outcome` label of `success`, `unreadable`, or
/// `invalid_schema`. The function does not install a tracing subscriber or
/// metrics recorder.
///
/// # Errors
///
/// Returns [`BoundaryError`] when the reader fails or when the bytes do not
/// match the boundary schema.
///
/// # Examples
/// ```
/// use weaver_docs_gate::load_manifest;
///
/// let manifest = load_manifest(
///     br#"schema_version = 1
/// managed_tasks = []
/// task = []
/// "#
///     .as_slice(),
/// )?;
/// assert_eq!(manifest.schema_version, 1);
/// # Ok::<(), weaver_docs_gate::BoundaryError>(())
/// ```
pub fn load_manifest(mut reader: impl Read) -> Result<BoundaryManifest, BoundaryError> {
    debug!(
        target: OBSERVABILITY_TARGET,
        source = "reader",
        "loading boundary manifest",
    );
    let mut contents = String::new();
    reader.read_to_string(&mut contents).map_err(|source| {
        let detail = unreadable_detail(&source.to_string());
        record_load_failure("reader", "unreadable", &detail);
        BoundaryError::Unreadable { detail }
    })?;
    parse_manifest(&contents)
        .inspect(record_load_success)
        .inspect_err(|error| {
            record_load_failure(
                "reader",
                boundary_failure_outcome(error),
                &error.to_string(),
            );
        })
}

/// Load the boundary manifest from disk.
///
/// # Observability
///
/// Emits `tracing` events with target
/// `weaver_docs_gate::boundary_manifest` when loading starts, succeeds, or
/// fails. Increments the
/// `weaver_docs_gate_boundary_manifest_load_total` metrics counter with
/// `source = "file"` and an `outcome` label of `success`, `not_found`,
/// `invalid_path`, `unreadable`, or `invalid_schema`. The function does not
/// install a tracing subscriber or metrics recorder.
///
/// # Errors
///
/// Returns [`BoundaryFileError`] when the manifest is missing, unreadable, or
/// does not match the boundary schema.
///
/// # Examples
/// ```no_run
/// use camino::Utf8Path;
/// use weaver_docs_gate::load_manifest_file;
///
/// let manifest = load_manifest_file(Utf8Path::new("docs/orthoconfig-consumer-boundary.toml"))?;
/// assert_eq!(manifest.schema_version, 1);
/// # Ok::<(), weaver_docs_gate::BoundaryFileError>(())
/// ```
pub fn load_manifest_file(path: &Utf8Path) -> Result<BoundaryManifest, BoundaryFileError> {
    debug!(
        target: OBSERVABILITY_TARGET,
        source = "file",
        path = %path,
        "loading boundary manifest",
    );
    let parent = path.parent().unwrap_or_else(|| Utf8Path::new("."));
    let file_name = path.file_name().ok_or_else(|| invalid_path_error(path))?;
    let dir = Dir::open_ambient_dir(parent, ambient_authority()).map_err(|source| {
        let error = read_error(path, &source);
        record_file_load_failure(path, &error);
        error
    })?;

    let contents = dir.read_to_string(file_name).map_err(|source| {
        let error = read_error(path, &source);
        record_file_load_failure(path, &error);
        error
    })?;
    let manifest = parse_manifest(&contents).map_err(|error| {
        let file_error = file_error(path, error);
        record_file_load_failure(path, &file_error);
        file_error
    })?;
    record_file_load_success(path, &manifest);
    Ok(manifest)
}

/// Parse TOML manifest text into the public domain manifest.
fn parse_manifest(contents: &str) -> Result<BoundaryManifest, BoundaryError> {
    let manifest =
        toml::from_str::<manifest_adapter::BoundaryManifestDto>(contents).map_err(|source| {
            let detail = schema_detail(&source.to_string());
            BoundaryError::InvalidSchema { detail }
        })?;
    Ok(manifest.into())
}

/// Convert filesystem failures into stable manifest loading errors.
fn read_error(path: &Utf8Path, source: &io::Error) -> BoundaryFileError {
    if source.kind() == ErrorKind::NotFound {
        BoundaryFileError::NotFound(path.to_path_buf())
    } else {
        BoundaryFileError::Unreadable {
            path: path.to_path_buf(),
            detail: unreadable_detail(&source.to_string()),
        }
    }
}

/// Attach file context to domain parser errors.
fn file_error(path: &Utf8Path, error: BoundaryError) -> BoundaryFileError {
    match error {
        BoundaryError::Unreadable { detail } => BoundaryFileError::Unreadable {
            path: path.to_path_buf(),
            detail,
        },
        BoundaryError::InvalidSchema { detail } => BoundaryFileError::InvalidSchema {
            path: path.to_path_buf(),
            detail,
        },
    }
}

/// Build an invalid-path error and emit the matching operational event.
fn invalid_path_error(path: &Utf8Path) -> BoundaryFileError {
    let error = BoundaryFileError::InvalidPath(path.to_path_buf());
    record_file_load_failure(path, &error);
    error
}

/// Add remediation context to read failures.
fn unreadable_detail(source: &str) -> String {
    format!("{source}; remediation: {MANIFEST_REMEDIATION}")
}

/// Add remediation context to schema failures.
fn schema_detail(source: &str) -> String {
    format!("{source}; remediation: {MANIFEST_REMEDIATION}")
}

/// Record a successful manifest parse with low-cardinality labels.
fn record_load_success(manifest: &BoundaryManifest) {
    counter!(METRIC_LOAD_TOTAL, "source" => "reader", "outcome" => "success").increment(1);
    debug!(
        target: OBSERVABILITY_TARGET,
        task_count = manifest.tasks.len(),
        managed_task_count = manifest.managed_tasks.len(),
        "loaded boundary manifest",
    );
}

/// Record a successful file-backed manifest load.
fn record_file_load_success(path: &Utf8Path, manifest: &BoundaryManifest) {
    counter!(METRIC_LOAD_TOTAL, "source" => "file", "outcome" => "success").increment(1);
    debug!(
        target: OBSERVABILITY_TARGET,
        source = "file",
        path = %path,
        task_count = manifest.tasks.len(),
        managed_task_count = manifest.managed_tasks.len(),
        "loaded boundary manifest",
    );
}

/// Record a path-free manifest load failure.
fn record_load_failure(source: &'static str, outcome: &'static str, detail: &str) {
    counter!(METRIC_LOAD_TOTAL, "source" => source, "outcome" => outcome).increment(1);
    warn!(
        target: OBSERVABILITY_TARGET,
        source,
        outcome,
        remediation = MANIFEST_REMEDIATION,
        detail,
        "boundary manifest load failed",
    );
}

/// Record a file-backed manifest load failure.
fn record_file_load_failure(path: &Utf8Path, error: &BoundaryFileError) {
    let outcome = file_failure_outcome(error);
    counter!(METRIC_LOAD_TOTAL, "source" => "file", "outcome" => outcome).increment(1);
    warn!(
        target: OBSERVABILITY_TARGET,
        source = "file",
        outcome,
        path = %path,
        remediation = MANIFEST_REMEDIATION,
        error = %error,
        "boundary manifest load failed",
    );
}

/// Return the metric outcome label for a file-backed error.
const fn file_failure_outcome(error: &BoundaryFileError) -> &'static str {
    match error {
        BoundaryFileError::NotFound(_) => "not_found",
        BoundaryFileError::InvalidPath(_) => "invalid_path",
        BoundaryFileError::Unreadable { .. } => "unreadable",
        BoundaryFileError::InvalidSchema { .. } => "invalid_schema",
    }
}

/// Return the metric outcome label for a path-free parser error.
const fn boundary_failure_outcome(error: &BoundaryError) -> &'static str {
    match error {
        BoundaryError::Unreadable { .. } => "unreadable",
        BoundaryError::InvalidSchema { .. } => "invalid_schema",
    }
}
