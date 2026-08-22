//! Build-time utilities shared across Weaver build scripts.

use std::{
    env,
    io,
    process,
    sync::atomic::{AtomicU64, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};

use camino::{Utf8Path, Utf8PathBuf};
use cap_std::fs::Dir;
use time::{OffsetDateTime, format_description::well_known::Iso8601};

/// Date substituted when `SOURCE_DATE_EPOCH` is unset or unusable, so man
/// pages remain reproducible even without an explicit build timestamp.
const FALLBACK_DATE: &str = "1970-01-01";
/// Process-wide counter mixed into staging file names so concurrent man-page
/// writes within the same process never collide on a temporary file.
static MAN_PAGE_TMP_COUNTER: AtomicU64 = AtomicU64::new(0);

/// A `SOURCE_DATE_EPOCH` value parsed and resolved to a concrete instant.
struct SourceDate {
    /// Original `SOURCE_DATE_EPOCH` string, kept for use in diagnostics.
    raw: String,
    /// Parsed timestamp used to format the man page date.
    value: OffsetDateTime,
}

/// Reasons `SOURCE_DATE_EPOCH` could not be turned into a `SourceDate`.
enum SourceDateError {
    /// `SOURCE_DATE_EPOCH` was not set; no warning is needed for this case.
    Missing,
    /// The value was set but was not a valid integer of seconds.
    InvalidInteger {
        /// The unparsed value, reported back to the caller.
        raw: String,
    },
    /// The value parsed as an integer but is not a representable Unix
    /// timestamp.
    InvalidTimestamp {
        /// The unparsed value, reported back to the caller.
        raw: String,
    },
}

/// Derive the manual page date from a `SOURCE_DATE_EPOCH` value.
///
/// Warnings are collected without the `cargo:warning=` prefix so the caller can
/// decide how to emit them.
///
/// # Examples
/// ```
/// use weaver_build_util::manual_date;
///
/// let mut warnings = Vec::new();
/// let date = manual_date(Some("0"), &mut warnings);
///
/// assert_eq!(date, "1970-01-01");
/// assert!(warnings.is_empty());
/// ```
pub fn manual_date(source_date_epoch: Option<&str>, warnings: &mut Vec<String>) -> String {
    let source = match source_date_time(source_date_epoch) {
        Ok(source) => source,
        Err(error) => {
            push_source_date_warning(warnings, &error);
            return FALLBACK_DATE.into();
        }
    };

    let SourceDate { raw, value } = source;
    value.format(&Iso8601::DATE).unwrap_or_else(|_| {
        warnings.push(format!(
            "Invalid SOURCE_DATE_EPOCH '{raw}'; formatting failed; falling back to {FALLBACK_DATE}"
        ));
        FALLBACK_DATE.into()
    })
}

/// Convenience wrapper around [`manual_date`] that reads `SOURCE_DATE_EPOCH` from the environment.
///
/// # Examples
/// ```no_run
/// use weaver_build_util::manual_date_from_env;
///
/// let mut warnings = Vec::new();
/// let date = manual_date_from_env(&mut warnings);
///
/// // When SOURCE_DATE_EPOCH is unset, the fallback date is used.
/// println!("{date}");
/// ```
pub fn manual_date_from_env(warnings: &mut Vec<String>) -> String {
    let source_date_epoch = env::var("SOURCE_DATE_EPOCH").ok();
    manual_date(source_date_epoch.as_deref(), warnings)
}

/// Parse and resolve a raw `SOURCE_DATE_EPOCH` value into a `SourceDate`.
///
/// # Errors
/// Returns `SourceDateError::Missing` if `source_date_epoch` is `None`,
/// `InvalidInteger` if it is not a base-10 integer, or `InvalidTimestamp`
/// if it parses but is out of range for `OffsetDateTime`.
fn source_date_time(source_date_epoch: Option<&str>) -> Result<SourceDate, SourceDateError> {
    let Some(raw) = source_date_epoch else {
        return Err(SourceDateError::Missing);
    };
    let Ok(timestamp) = raw.parse::<i64>() else {
        return Err(SourceDateError::InvalidInteger { raw: raw.into() });
    };
    let Ok(value) = OffsetDateTime::from_unix_timestamp(timestamp) else {
        return Err(SourceDateError::InvalidTimestamp { raw: raw.into() });
    };

    Ok(SourceDate {
        raw: raw.into(),
        value,
    })
}

/// Translate a `SourceDateError` into a human-readable warning, if any.
///
/// `Missing` produces no warning: an unset `SOURCE_DATE_EPOCH` is expected
/// in most builds and is not worth flagging to the caller.
fn push_source_date_warning(warnings: &mut Vec<String>, error: &SourceDateError) {
    match error {
        SourceDateError::Missing => {}
        SourceDateError::InvalidInteger { raw } => warnings.push(format!(
            "Invalid SOURCE_DATE_EPOCH '{raw}'; expected integer seconds since Unix epoch; \
             falling back to {FALLBACK_DATE}"
        )),
        SourceDateError::InvalidTimestamp { raw } => warnings.push(format!(
            "Invalid SOURCE_DATE_EPOCH '{raw}'; not a valid Unix timestamp; falling back to \
             {FALLBACK_DATE}"
        )),
    }
}

/// Derive the workspace target directory from `OUT_DIR`.
///
/// `OUT_DIR` structure varies based on build type:
/// - Native:      `{workspace}/target/{profile}/build/{crate}-{hash}/out`
/// - Cross-build: `{workspace}/target/{target}/{profile}/build/{crate}-{hash}/out`
///
/// We find the `target` directory by searching up the path for a component named "target".
///
/// # Examples
/// ```
/// use weaver_build_util::workspace_target_dir;
///
/// let out_dir =
///     camino::Utf8Path::new("/tmp/workspace/target/release/build/weaver-cli-abc123/out");
///
/// let target_dir = workspace_target_dir(out_dir).expect("target directory not found");
///
/// assert!(target_dir.as_path().ends_with("target"));
/// ```
#[must_use]
pub fn workspace_target_dir(out_dir: &Utf8Path) -> Option<Utf8PathBuf> {
    // Walk up the path until we find a directory named "target".
    let mut current = out_dir;
    while let Some(parent) = current.parent() {
        if current.file_name() == Some("target") {
            return Some(current.to_path_buf());
        }
        current = parent;
    }
    None
}

/// Compute the target directory for generated man pages based on TARGET and PROFILE.
///
/// # Examples
/// ```
/// use weaver_build_util::out_dir_for_target_profile;
///
/// let out_dir = camino::Utf8Path::new(
///     "/tmp/workspace/target/aarch64-unknown-linux-gnu/release/build/weaver-cli-abc123/out",
/// );
///
/// let generated =
///     out_dir_for_target_profile("aarch64-unknown-linux-gnu", "release", Some(out_dir));
///
/// assert!(
///     generated
///         .as_path()
///         .ends_with("generated-man/aarch64-unknown-linux-gnu/release")
/// );
/// ```
pub fn out_dir_for_target_profile(
    target: &str,
    profile: &str,
    out_dir: Option<&Utf8Path>,
) -> Utf8PathBuf {
    // Use workspace target directory if available, otherwise fall back to relative path.
    let base = out_dir
        .and_then(workspace_target_dir)
        .unwrap_or_else(|| Utf8PathBuf::from("target"));
    base.join(format!("generated-man/{target}/{profile}"))
}

/// Creates a directory and all its parents using capability-based filesystem operations.
fn create_dir_all_cap(base: &Dir, path: &Utf8Path) -> io::Result<()> {
    let mut current_path = Utf8PathBuf::new();

    for component in path.components() {
        current_path.push(component.as_str());
        match base.create_dir(&current_path) {
            Ok(()) => {}
            Err(err) if err.kind() == io::ErrorKind::AlreadyExists => {}
            Err(err) => return Err(err),
        }
    }

    Ok(())
}

/// Walk `dir` upward until an already-existing directory is found.
///
/// Used before capability-based directory creation, since `cap_std` needs an
/// existing ancestor to open before it can create the missing descendants.
/// Falls back to `"."` if no ancestor exists (for example a relative path
/// with no existing prefix).
fn find_existing_ancestor(dir: &Utf8Path) -> &Utf8Path {
    let mut candidate = dir;
    loop {
        if Dir::open_ambient_dir(candidate, cap_std::ambient_authority()).is_ok() {
            return candidate;
        }
        let Some(parent) = candidate.parent() else {
            break;
        };
        if parent == candidate {
            break;
        }
        candidate = parent;
    }
    Utf8Path::new(".")
}

/// Open `relative_path` under `base_dir`, creating any missing components.
///
/// # Errors
/// Returns any I/O error from directory creation or opening. An empty
/// `relative_path` is treated as "no subdirectory needed" and returns
/// `base_dir` unchanged.
fn ensure_target_dir(base_dir: Dir, relative_path: &Utf8Path) -> io::Result<Dir> {
    if relative_path.as_str().is_empty() {
        return Ok(base_dir);
    }
    create_dir_all_cap(&base_dir, relative_path)?;
    base_dir.open_dir(relative_path)
}

/// Build a unique staging file name for a man page write.
///
/// Mixes the process ID, wall-clock nanoseconds, and a monotonic counter so
/// concurrent invocations (same process or sibling processes) cannot collide
/// on the temporary file before the atomic rename into place.
fn staging_file_name(page_name: &str) -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or_default();
    let counter = MAN_PAGE_TMP_COUNTER.fetch_add(1, Ordering::Relaxed);
    format!("{page_name}.tmp-{}-{nanos}-{counter}", process::id())
}

/// Write a man page to the provided directory with best-effort replacement.
///
/// # Errors
/// Returns any filesystem errors encountered while creating the directory,
/// writing the file, or replacing an existing page. On platforms where
/// overwriting a destination rename is unavailable, replacement falls back to a
/// non-atomic delete-then-rename sequence.
///
/// # Examples
/// ```no_run
/// use weaver_build_util::write_man_page;
///
/// let dir = camino::Utf8PathBuf::from(std::env::temp_dir().to_string_lossy().as_ref());
/// let data = b".TH WEAVER 1 1970-01-01 weaver 0.1.0\n";
/// let path = write_man_page(data, &dir, "weaver.1").expect("man page write failed");
///
/// assert!(path.ends_with("weaver.1"));
/// ```
pub fn write_man_page(data: &[u8], dir: &Utf8Path, page_name: &str) -> io::Result<Utf8PathBuf> {
    let existing_ancestor = find_existing_ancestor(dir);
    let base_dir = Dir::open_ambient_dir(existing_ancestor, cap_std::ambient_authority())?;
    let relative_path = dir.strip_prefix(existing_ancestor).unwrap_or(dir);
    let target_dir = ensure_target_dir(base_dir, relative_path)?;
    let tmp = staging_file_name(page_name);
    target_dir.write(&tmp, data)?;

    match target_dir.rename(&tmp, &target_dir, page_name) {
        Ok(()) => {}
        Err(error) if should_retry_replace(&error) => {
            match target_dir.rename(&tmp, &target_dir, page_name) {
                Ok(()) => {}
                Err(retry_error) if should_retry_replace(&retry_error) => {
                    remove_existing_file(&target_dir, page_name)?;
                    target_dir.rename(&tmp, &target_dir, page_name)?;
                }
                Err(retry_error) => return Err(retry_error),
            }
        }
        Err(error) => return Err(error),
    }

    Ok(dir.join(page_name))
}

/// Decide whether a failed rename-into-place should be retried.
///
/// Covers the two cases where the destination is occupied: `AlreadyExists`
/// on platforms that reject renaming over an existing file, and, on
/// Windows specifically, `PermissionDenied`, which that platform can also
/// raise when the destination is in use.
fn should_retry_replace(error: &io::Error) -> bool {
    error.kind() == io::ErrorKind::AlreadyExists
        || (cfg!(windows) && error.kind() == io::ErrorKind::PermissionDenied)
}

/// Remove `name` from `dir`, treating an already-absent file as success.
///
/// # Errors
/// Returns any I/O error other than `NotFound`, which is swallowed so the
/// caller's retry-then-remove-then-rename sequence stays idempotent.
fn remove_existing_file(dir: &Dir, name: &str) -> io::Result<()> {
    match dir.remove_file(name) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error),
    }
}

#[cfg(test)]
mod tests {
    //! Regression tests for capability-based filesystem helpers.

    use super::*;

    #[test]
    fn write_man_page_creates_nested_directories() {
        let temp_dir = tempfile::tempdir().expect("tempdir");
        let temp_path =
            Utf8PathBuf::from_path_buf(temp_dir.path().to_path_buf()).expect("utf-8 tempdir");
        let nested_dir = temp_path.join("target/generated-man/test-target/debug");
        let temp_dir_handle =
            Dir::open_ambient_dir(&temp_path, cap_std::ambient_authority()).expect("open tempdir");
        let existing_output_path = nested_dir.join("weaver.1");
        let existing_relative_path = existing_output_path
            .strip_prefix(&temp_path)
            .expect("existing path should live under tempdir");
        let existing_parent = existing_relative_path
            .parent()
            .expect("existing path should have parent");
        temp_dir_handle
            .create_dir_all(existing_parent)
            .expect("create existing parent dirs");
        temp_dir_handle
            .write(existing_relative_path, b"old content\n")
            .expect("seed existing man page");

        let output_path =
            write_man_page(b".TH WEAVER 1\n", &nested_dir, "weaver.1").expect("write man page");
        let relative_output_path = output_path
            .strip_prefix(&temp_path)
            .expect("output path should live under tempdir");

        let expected_output_path = nested_dir.join("weaver.1");
        assert_eq!(
            output_path, expected_output_path,
            "unexpected output path: expected {expected_output_path}, got {output_path}"
        );

        let written_content = temp_dir_handle
            .read_to_string(relative_output_path)
            .expect("read man page");
        assert_eq!(
            written_content, ".TH WEAVER 1\n",
            "unexpected man page content: expected {:?}, got {:?}",
            ".TH WEAVER 1\n", written_content
        );
    }
}
