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

/// Derive the manual page date using SOURCE_DATE_EPOCH with a reproducible fallback.
///
/// Warnings are collected without the `cargo:warning=` prefix so the caller can
/// decide how to emit them.
///
/// # Examples
/// ```
/// use weaver_build_util::manual_date;
///
/// let previous = std::env::var("SOURCE_DATE_EPOCH").ok();
/// std::env::set_var("SOURCE_DATE_EPOCH", "0");
///
/// let mut warnings = Vec::new();
/// let date = manual_date(&mut warnings);
///
/// if let Some(value) = previous {
///     std::env::set_var("SOURCE_DATE_EPOCH", value);
/// } else {
///     std::env::remove_var("SOURCE_DATE_EPOCH");
/// }
///
/// assert_eq!(date, "1970-01-01");
/// assert!(warnings.is_empty());
/// ```
pub fn manual_date(warnings: &mut Vec<String>) -> String {
    let source = match source_date_time() {
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

fn source_date_time() -> Result<SourceDate, SourceDateError> {
    let raw = match env::var("SOURCE_DATE_EPOCH") {
        Ok(value) => value,
        Err(_) => return Err(SourceDateError::Missing),
    };

    let timestamp = match raw.parse::<i64>() {
        Ok(value) => value,
        Err(_) => return Err(SourceDateError::InvalidInteger { raw }),
    };

    let value = match OffsetDateTime::from_unix_timestamp(timestamp) {
        Ok(value) => value,
        Err(_) => return Err(SourceDateError::InvalidTimestamp { raw }),
    };

    Ok(SourceDate { raw, value })
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

/// Derive the workspace target directory from OUT_DIR.
///
/// OUT_DIR structure varies based on build type:
/// - Native:      `{workspace}/target/{profile}/build/{crate}-{hash}/out`
/// - Cross-build: `{workspace}/target/{target}/{profile}/build/{crate}-{hash}/out`
///
/// We find the `target` directory by searching up the path for a component named "target".
///
/// # Examples
/// ```
/// use weaver_build_util::workspace_target_dir;
///
/// let previous = std::env::var_os("OUT_DIR");
/// std::env::set_var(
///     "OUT_DIR",
///     "/tmp/workspace/target/release/build/weaver-cli-abc123/out",
/// );
///
/// let target_dir = workspace_target_dir().expect("target directory not found");
///
/// if let Some(value) = previous {
///     std::env::set_var("OUT_DIR", value);
/// } else {
///     std::env::remove_var("OUT_DIR");
/// }
///
/// assert!(target_dir.as_path().ends_with("target"));
/// ```
pub fn workspace_target_dir() -> Option<PathBuf> {
    let out_dir = PathBuf::from(env::var_os("OUT_DIR")?);

    // Walk up the path until we find a directory named "target".
    let mut current = out_dir.as_path();
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
/// let previous_out = std::env::var_os("OUT_DIR");
/// let previous_target = std::env::var("TARGET").ok();
/// let previous_profile = std::env::var("PROFILE").ok();
///
/// std::env::set_var(
///     "OUT_DIR",
///     "/tmp/workspace/target/aarch64-unknown-linux-gnu/release/build/weaver-cli-abc123/out",
/// );
/// std::env::set_var("TARGET", "aarch64-unknown-linux-gnu");
/// std::env::set_var("PROFILE", "release");
///
/// let out_dir = out_dir_for_target_profile();
///
/// if let Some(value) = previous_out {
///     std::env::set_var("OUT_DIR", value);
/// } else {
///     std::env::remove_var("OUT_DIR");
/// }
/// if let Some(value) = previous_target {
///     std::env::set_var("TARGET", value);
/// } else {
///     std::env::remove_var("TARGET");
/// }
/// if let Some(value) = previous_profile {
///     std::env::set_var("PROFILE", value);
/// } else {
///     std::env::remove_var("PROFILE");
/// }
///
/// assert!(out_dir
///     .as_path()
///     .ends_with("generated-man/aarch64-unknown-linux-gnu/release"));
/// ```
pub fn out_dir_for_target_profile() -> PathBuf {
    let target = env::var("TARGET").unwrap_or_else(|_| "unknown-target".into());
    let profile = env::var("PROFILE").unwrap_or_else(|_| "unknown-profile".into());

    // Use workspace target directory if available, otherwise fall back to relative path.
    let base = workspace_target_dir().unwrap_or_else(|| PathBuf::from("target"));
    base.join(format!("generated-man/{target}/{profile}"))
}

/// Write a man page to the provided directory, ensuring atomic replacement.
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
    if destination.exists() {
        fs::remove_file(&destination)?;
    }
    fs::rename(&tmp, &destination)?;
    Ok(destination)
}
