//! Build-time utilities shared across Weaver build scripts.

use std::{env, io};

use camino::{Utf8Path, Utf8PathBuf};
use cap_std::fs::Dir;
use time::{OffsetDateTime, format_description::well_known::Iso8601};

const FALLBACK_DATE: &str = "1970-01-01";

struct SourceDate {
    raw: String,
    value: OffsetDateTime,
}

enum SourceDateError {
    Missing,
    InvalidInteger { raw: String },
    InvalidTimestamp { raw: String },
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

fn ensure_target_dir(base_dir: Dir, relative_path: &Utf8Path) -> io::Result<Dir> {
    if relative_path.as_str().is_empty() {
        return Ok(base_dir);
    }
    create_dir_all_cap(&base_dir, relative_path)?;
    base_dir.open_dir(relative_path)
}

/// Write a man page to the provided directory, ensuring atomic replacement.
///
/// # Errors
/// Returns any filesystem errors encountered while creating the directory or
/// writing the file.
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
    let tmp = format!("{page_name}.tmp");
    target_dir.write(&tmp, data)?;

    match target_dir.rename(&tmp, &target_dir, page_name) {
        Ok(()) => {}
        Err(error) if should_retry_replace(&error) => {
            remove_existing_file(&target_dir, page_name)?;
            target_dir.rename(&tmp, &target_dir, page_name)?;
        }
        Err(error) => return Err(error),
    }

    Ok(dir.join(page_name))
}

fn should_retry_replace(error: &io::Error) -> bool {
    error.kind() == io::ErrorKind::AlreadyExists
        || (cfg!(windows) && error.kind() == io::ErrorKind::PermissionDenied)
}

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

        let output_path =
            write_man_page(b".TH WEAVER 1\n", &nested_dir, "weaver.1").expect("write man page");
        let relative_output_path = output_path
            .strip_prefix(&temp_path)
            .expect("output path should live under tempdir");

        assert_eq!(output_path, nested_dir.join("weaver.1"));
        assert_eq!(
            temp_dir_handle
                .read_to_string(relative_output_path)
                .expect("read man page"),
            ".TH WEAVER 1\n"
        );
    }
}
