//! Build-time utilities shared across Weaver build scripts.

use std::{
    env, fs, io,
    path::{Path, PathBuf},
};
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
/// let out_dir = std::path::Path::new(
///     "/tmp/workspace/target/release/build/weaver-cli-abc123/out",
/// );
///
/// let target_dir = workspace_target_dir(out_dir).expect("target directory not found");
///
/// assert!(target_dir.as_path().ends_with("target"));
/// ```
#[must_use]
pub fn workspace_target_dir(out_dir: &Path) -> Option<PathBuf> {
    // Walk up the path until we find a directory named "target".
    let mut current = out_dir;
    while let Some(parent) = current.parent() {
        if current.file_name().and_then(|name| name.to_str()) == Some("target") {
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
/// let out_dir = std::path::Path::new(
///     "/tmp/workspace/target/aarch64-unknown-linux-gnu/release/build/weaver-cli-abc123/out",
/// );
///
/// let generated = out_dir_for_target_profile(
///     "aarch64-unknown-linux-gnu",
///     "release",
///     Some(out_dir),
/// );
///
/// assert!(generated
///     .as_path()
///     .ends_with("generated-man/aarch64-unknown-linux-gnu/release"));
/// ```
pub fn out_dir_for_target_profile(target: &str, profile: &str, out_dir: Option<&Path>) -> PathBuf {
    // Use workspace target directory if available, otherwise fall back to relative path.
    let base = out_dir
        .and_then(workspace_target_dir)
        .unwrap_or_else(|| PathBuf::from("target"));
    base.join(format!("generated-man/{target}/{profile}"))
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
/// let dir = std::env::temp_dir().join("weaver-build-util-manpage");
/// let data = b".TH WEAVER 1 1970-01-01 weaver 0.1.0\n";
/// let path = write_man_page(data, &dir, "weaver.1").expect("man page write failed");
///
/// assert!(path.ends_with("weaver.1"));
/// ```
pub fn write_man_page(data: &[u8], dir: &Path, page_name: &str) -> io::Result<PathBuf> {
    fs::create_dir_all(dir)?;
    let destination = dir.join(page_name);
    let tmp = dir.join(format!("{page_name}.tmp"));
    fs::write(&tmp, data)?;
    match fs::rename(&tmp, &destination) {
        Ok(()) => Ok(destination),
        Err(error) if should_retry_replace(&error) => {
            remove_existing_file(&destination)?;
            fs::rename(&tmp, &destination)?;
            Ok(destination)
        }
        Err(error) => Err(error),
    }
}

fn should_retry_replace(error: &io::Error) -> bool {
    error.kind() == io::ErrorKind::AlreadyExists
        || (cfg!(windows) && error.kind() == io::ErrorKind::PermissionDenied)
}

fn remove_existing_file(path: &Path) -> io::Result<()> {
    match fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error),
    }
}
