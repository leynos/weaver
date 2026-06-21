//! Unit-level coverage for boundary manifest parsing and file loading.
//!
//! The tests in this module deliberately exercise both entry points in the
//! docs-gate crate. `load_manifest` receives in-memory manifest bytes and
//! proves the path-free parser behaviour; `load_manifest_file` receives UTF-8
//! paths and proves the filesystem adapter's error mapping.

use std::process::{Command, Output};

use camino::{Utf8Path, Utf8PathBuf};
use cap_std::{ambient_authority, fs::Dir};
use tempfile::TempDir;
use weaver_docs_gate::{
    BoundaryError,
    BoundaryFileError,
    BoundaryState,
    load_manifest,
    load_manifest_file,
};

type TestResult<T = ()> = Result<T, String>;

const VALID_MANIFEST: &str = r#"
schema_version = 1
managed_tasks = ["12.1.1"]

[[task]]
id = "12.1.1"
gist = "Track the downstream consumer boundary."
state = "consumes"
shipped_in = "4339a6f3"
removal_gate = ""
adr_anchor = ""
next_review_by = ""
last_reviewed = "2026-06-20"

[[task.upstream]]
task = "renderer-contract"
role = "renderer"
"#;

/// Prove missing manifest files return the not-found variant.
#[test]
fn load_manifest_reports_missing_files() -> TestResult {
    let temp = temp_dir()?;
    let missing = temp.path().join("missing.toml");

    let error = load_manifest_file(&missing)
        .err()
        .ok_or("missing file loaded")?;

    assert_error_variant(&error, |e| matches!(e, BoundaryFileError::NotFound(_)))
}

/// Prove paths without a file name return the invalid-path variant.
#[test]
fn load_manifest_reports_invalid_paths() -> TestResult {
    let error = load_manifest_file(Utf8Path::new("/"))
        .err()
        .ok_or("root path loaded as manifest")?;

    assert_error_variant(&error, |e| matches!(e, BoundaryFileError::InvalidPath(_)))
}

/// Prove non-file manifest paths return the unreadable variant.
#[test]
fn load_manifest_reports_read_errors() -> TestResult {
    let temp = temp_dir()?;
    let manifest_dir = temp.path().join("manifest.toml");
    Dir::open_ambient_dir(temp.path(), ambient_authority())
        .map_err(|error| format!("open temp dir: {error}"))?
        .create_dir("manifest.toml")
        .map_err(|error| format!("create manifest directory: {error}"))?;

    let error = load_manifest_file(&manifest_dir)
        .err()
        .ok_or("directory loaded as manifest")?;

    assert_error_variant(&error, |e| {
        matches!(e, BoundaryFileError::Unreadable { .. })
    })
}

/// Prove malformed manifest contents return the schema variant.
#[test]
fn load_manifest_reports_invalid_toml() -> TestResult {
    let error = load_manifest(b"schema_version = ".as_slice())
        .err()
        .ok_or("invalid TOML loaded as manifest")?;

    assert_error_variant(&error, |e| matches!(e, BoundaryError::InvalidSchema { .. }))
}

/// Snapshot the stable display strings for public parser errors.
#[test]
fn boundary_error_display_messages_are_stable() {
    insta::assert_snapshot!(
        BoundaryError::Unreadable {
            detail: "stream closed".into(),
        }
        .to_string(),
        @"boundary manifest cannot be read: stream closed"
    );
    insta::assert_snapshot!(
        BoundaryError::InvalidSchema {
            detail: "missing field `schema_version`".into(),
        }
        .to_string(),
        @"invalid boundary manifest schema: missing field `schema_version`"
    );
}

/// Snapshot the stable display strings for public file-adapter errors.
#[test]
fn boundary_file_error_display_messages_are_stable() {
    insta::assert_snapshot!(
        BoundaryFileError::NotFound(Utf8PathBuf::from("docs/missing.toml")).to_string(),
        @"manifest file not found: docs/missing.toml"
    );
    insta::assert_snapshot!(
        BoundaryFileError::InvalidPath(Utf8PathBuf::from("/")).to_string(),
        @"invalid manifest path: /"
    );
    insta::assert_snapshot!(
        BoundaryFileError::Unreadable {
            path: Utf8PathBuf::from("docs/manifest.toml"),
            detail: "permission denied".into(),
        }
        .to_string(),
        @"boundary manifest cannot be read: docs/manifest.toml: permission denied"
    );
    insta::assert_snapshot!(
        BoundaryFileError::InvalidSchema {
            path: Utf8PathBuf::from("docs/manifest.toml"),
            detail: "missing field `schema_version`".into(),
        }
        .to_string(),
        @"invalid boundary manifest schema in docs/manifest.toml: missing field `schema_version`"
    );
}

/// Prove adapter DTOs map manifest values into plain domain types.
#[test]
fn load_manifest_maps_toml_dto_to_domain_types() -> TestResult {
    let manifest = load_manifest(VALID_MANIFEST.as_bytes()).map_err(|error| error.to_string())?;

    ensure_equal(&manifest.schema_version, &1, "schema version should load")?;
    ensure_equal(
        &manifest.tasks.first().map(|task| task.state),
        &Some(BoundaryState::Consumes),
        "state should map from TOML DTO",
    )?;
    ensure_equal(
        &manifest
            .tasks
            .first()
            .and_then(|task| task.next_review_by.as_ref()),
        &None,
        "empty optional strings should map to None",
    )?;
    Ok(())
}

/// Prove the checked example binary writes a generated matrix.
#[test]
fn render_boundary_matrix_example_writes_output() -> TestResult {
    let temp = temp_dir()?;
    let manifest_path = temp.path().join("manifest.toml");
    let output_path = temp.path().join("matrix.md");
    write_file(temp.path(), "manifest.toml", VALID_MANIFEST)?
        .map_err(|error| format!("write valid manifest: {error}"))?;

    let output = run_render_boundary_matrix(&[manifest_path.as_str(), output_path.as_str()])?;

    if !output.status.success() {
        return Err(format!(
            "example failed\nstdout: {}\nstderr: {}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        ));
    }

    let rendered = read_file(temp.path(), "matrix.md")?
        .map_err(|error| format!("read rendered matrix: {error}"))?;
    ensure(
        rendered.contains("[12.1.1]"),
        "rendered output should name task",
    )
}

/// Prove the example reports a missing manifest path.
#[test]
fn render_boundary_matrix_example_reports_missing_manifest() -> TestResult {
    let temp = temp_dir()?;
    let manifest_path = temp.path().join("missing.toml");
    let output_path = temp.path().join("matrix.md");

    let output = run_render_boundary_matrix(&[manifest_path.as_str(), output_path.as_str()])?;

    assert_example_failure(&output, "manifest file not found")
}

/// Prove the example reports a missing output argument.
#[test]
fn render_boundary_matrix_example_reports_missing_output_argument() -> TestResult {
    let temp = temp_dir()?;
    let manifest_path = temp.path().join("manifest.toml");
    write_file(temp.path(), "manifest.toml", VALID_MANIFEST)?
        .map_err(|error| format!("write valid manifest: {error}"))?;

    let output = run_render_boundary_matrix(&[manifest_path.as_str()])?;

    assert_example_failure(&output, "usage: render_boundary_matrix <manifest> <output>")
}

/// Prove the example reports unexpected extra arguments.
#[test]
fn render_boundary_matrix_example_reports_extra_arguments() -> TestResult {
    let temp = temp_dir()?;
    let manifest_path = temp.path().join("manifest.toml");
    let output_path = temp.path().join("matrix.md");
    write_file(temp.path(), "manifest.toml", VALID_MANIFEST)?
        .map_err(|error| format!("write valid manifest: {error}"))?;

    let output =
        run_render_boundary_matrix(&[manifest_path.as_str(), output_path.as_str(), "extra"])?;

    assert_example_failure(&output, "usage: render_boundary_matrix <manifest> <output>")
}

/// Prove the example reports an output path that cannot name a file.
#[test]
fn render_boundary_matrix_example_reports_invalid_output_path() -> TestResult {
    let temp = temp_dir()?;
    let manifest_path = temp.path().join("manifest.toml");
    write_file(temp.path(), "manifest.toml", VALID_MANIFEST)?
        .map_err(|error| format!("write valid manifest: {error}"))?;

    let output = run_render_boundary_matrix(&[manifest_path.as_str(), "/"])?;

    assert_example_failure(&output, "invalid output path: /")
}

/// Create a UTF-8 temp directory guard for filesystem tests.
fn temp_dir() -> TestResult<Utf8TempDir> {
    let temp = tempfile::tempdir().map_err(|error| format!("create temp dir: {error}"))?;
    let path = Utf8PathBuf::from_path_buf(temp.path().to_path_buf())
        .map_err(|path| format!("temp path is not UTF-8: {}", path.display()))?;
    Ok(Utf8TempDir { _temp: temp, path })
}

/// Write a file through a capability-oriented directory handle.
fn write_file(
    dir_path: &Utf8Path,
    file_name: &str,
    content: &str,
) -> TestResult<Result<(), std::io::Error>> {
    let dir = Dir::open_ambient_dir(dir_path, ambient_authority())
        .map_err(|error| format!("open {dir_path}: {error}"))?;
    Ok(dir.write(file_name, content))
}

/// Read a file through a capability-oriented directory handle.
fn read_file(dir_path: &Utf8Path, file_name: &str) -> TestResult<Result<String, std::io::Error>> {
    let dir = Dir::open_ambient_dir(dir_path, ambient_authority())
        .map_err(|error| format!("open {dir_path}: {error}"))?;
    Ok(dir.read_to_string(file_name))
}

/// Run the documented matrix-rendering example with caller-supplied args.
fn run_render_boundary_matrix(args: &[&str]) -> TestResult<Output> {
    Command::new(env!("CARGO"))
        .args([
            "run",
            "-p",
            "weaver-docs-gate",
            "--example",
            "render_boundary_matrix",
            "--",
        ])
        .args(args)
        .current_dir(repo_root()?)
        .output()
        .map_err(|error| format!("run render_boundary_matrix example: {error}"))
}

/// Check that the example failed and wrote the expected diagnostic.
fn assert_example_failure(output: &Output, expected_stderr: &str) -> TestResult {
    ensure(
        !output.status.success(),
        "example should exit with failure status",
    )?;
    ensure(
        output.stdout.is_empty(),
        format!(
            "example failure should not write stdout\nstdout: {}",
            String::from_utf8_lossy(&output.stdout),
        ),
    )?;
    ensure(
        String::from_utf8_lossy(&output.stderr).contains(expected_stderr),
        format!(
            "example stderr should contain {expected_stderr:?}\nstderr: {}",
            String::from_utf8_lossy(&output.stderr),
        ),
    )
}

/// Resolve the repository root from the test crate directory.
fn repo_root() -> TestResult<Utf8PathBuf> {
    let crate_dir = Utf8Path::new(env!("CARGO_MANIFEST_DIR"));
    crate_dir
        .parent()
        .and_then(Utf8Path::parent)
        .map(Utf8Path::to_path_buf)
        .ok_or_else(|| format!("cannot resolve repository root from {crate_dir}"))
}

/// Return an error when a test invariant is false.
fn ensure(condition: bool, message: impl Into<String>) -> TestResult {
    if condition {
        Ok(())
    } else {
        Err(message.into())
    }
}

/// Return a diff-friendly error when two values differ.
fn ensure_equal<T>(left: &T, right: &T, message: impl Into<String>) -> TestResult
where
    T: std::fmt::Debug + PartialEq,
{
    if left == right {
        Ok(())
    } else {
        Err(format!(
            "{}\nleft: {left:?}\nright: {right:?}",
            message.into()
        ))
    }
}

/// Check an error variant while preserving full debug diagnostics on mismatch.
fn assert_error_variant<E>(error: &E, predicate: impl FnOnce(&E) -> bool) -> TestResult
where
    E: std::fmt::Debug,
{
    if predicate(error) {
        Ok(())
    } else {
        Err(format!("unexpected error variant: {error:?}"))
    }
}

/// Temp directory wrapper that exposes a UTF-8 path.
struct Utf8TempDir {
    _temp: TempDir,
    path: Utf8PathBuf,
}

impl Utf8TempDir {
    /// Return the UTF-8 temp directory path.
    fn path(&self) -> &Utf8Path { &self.path }
}
