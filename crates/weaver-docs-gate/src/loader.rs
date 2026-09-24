//! Load boundary manifests from readers or files and emit load diagnostics.

use std::io::{self, ErrorKind, Read};

use camino::Utf8Path;
use cap_std::{ambient_authority, fs::Dir};
use metrics::counter;
use tracing::{debug, warn};

use super::{BoundaryError, BoundaryFileError, BoundaryManifest};

/// Remediation text shown in load-failure diagnostics, so a failing check
/// points a contributor straight at the fix instead of just the symptom.
const MANIFEST_REMEDIATION: &str = "update docs/orthoconfig-consumer-boundary.toml, regenerate \
                                    docs/orthoconfig-consumer-boundary.md, then rerun cargo test \
                                    -p weaver-docs-gate";
/// Name of the counter metric incremented on every manifest load attempt.
const METRIC_LOAD_TOTAL: &str = "weaver_docs_gate_boundary_manifest_load_total";
/// Tracing target used for manifest-load spans and events, kept distinct
/// from the crate's default target so log filters can isolate this path.
const OBSERVABILITY_TARGET: &str = "weaver_docs_gate::boundary_manifest";

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
        .inspect(|manifest| record_success("reader", None, manifest))
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
    record_success("file", Some(path), &manifest);
    Ok(manifest)
}

/// Parse TOML manifest text into the public domain manifest.
fn parse_manifest(contents: &str) -> Result<BoundaryManifest, BoundaryError> {
    toml::from_str::<BoundaryManifest>(contents).map_err(|source| {
        let detail = schema_detail(&source.to_string());
        BoundaryError::InvalidSchema { detail }
    })
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

/// Record a successful manifest load: increment the load counter and emit a
/// debug event carrying the source and task counts, so operators can see
/// load activity without the volume of a per-field trace.
fn record_success(source: &'static str, path: Option<&Utf8Path>, manifest: &BoundaryManifest) {
    counter!(METRIC_LOAD_TOTAL, "source" => source, "outcome" => "success").increment(1);
    if let Some(manifest_path) = path {
        debug!(
            target: OBSERVABILITY_TARGET,
            source,
            path = %manifest_path,
            task_count = manifest.tasks.len(),
            managed_task_count = manifest.managed_tasks.len(),
            "loaded boundary manifest",
        );
    } else {
        debug!(
            target: OBSERVABILITY_TARGET,
            source,
            task_count = manifest.tasks.len(),
            managed_task_count = manifest.managed_tasks.len(),
            "loaded boundary manifest",
        );
    }
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
