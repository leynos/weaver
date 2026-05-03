//! Helpers for reading and parsing daemon runtime files.

use std::{io, path::Path};

use cap_std::fs::Dir;

use crate::lifecycle::{error::LifecycleError, monitoring::HealthSnapshot};

macro_rules! define_reader {
    (
        $(#[$attr:meta])*
        $vis:vis fn $name:ident(
            dir: &Dir,
            filename: &str,
            full_path: &Path,
        ) -> Result<Option<$ret_ty:ty>, LifecycleError> {
            read_error: $read_variant:ident,
            parse_error: $parse_variant:ident,
            parse: $parse_expr:expr
        }
    ) => {
        $(#[$attr])*
        $vis fn $name(
            dir: &Dir,
            filename: &str,
            full_path: &Path,
        ) -> Result<Option<$ret_ty>, LifecycleError> {
            read_and_parse(
                dir,
                filename,
                |source| LifecycleError::$read_variant {
                    path: full_path.to_path_buf(),
                    source,
                },
                |content| {
                    let trimmed = content.trim();
                    if trimmed.is_empty() {
                        return Ok(None);
                    }
                    $parse_expr(trimmed)
                        .map(Some)
                        .map_err(|source| LifecycleError::$parse_variant {
                            path: full_path.to_path_buf(),
                            source,
                        })
                },
            )
        }
    };
}

/// Reads a file from the runtime directory, treating `NotFound` as `Ok(None)`.
///
/// This encapsulates the common pattern where a missing file is a valid state
/// (for example, during daemon startup before health or PID files are written),
/// rather than an error. Other I/O errors are propagated.
fn read_optional_file(dir: &Dir, filename: &str) -> Result<Option<String>, io::Error> {
    match dir.read_to_string(filename) {
        Ok(content) => Ok(Some(content)),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error),
    }
}

/// Reads and parses an optional runtime file with customizable error handling.
///
/// Combines file reading with parsing, handling the common pattern of:
/// 1. Read file (returning `Ok(None)` if missing)
/// 2. Map I/O errors using the provided `read_error` constructor
/// 3. Parse content using the provided `parse` function
fn read_and_parse<T, R, P>(
    dir: &Dir,
    filename: &str,
    read_error: R,
    parse: P,
) -> Result<Option<T>, LifecycleError>
where
    R: FnOnce(io::Error) -> LifecycleError,
    P: FnOnce(&str) -> Result<Option<T>, LifecycleError>,
{
    let Some(content) = read_optional_file(dir, filename).map_err(read_error)? else {
        return Ok(None);
    };
    parse(&content)
}

define_reader! {
    /// Reads and parses the daemon health snapshot from the runtime directory.
    ///
    /// Returns:
    /// * `Ok(Some(snapshot))` when parsed successfully.
    /// * `Ok(None)` when the file is absent.
    /// * `Err(ReadHealth)` if I/O fails.
    /// * `Err(ParseHealth)` if JSON is invalid.
    pub(crate) fn read_health(
        dir: &Dir,
        filename: &str,
        full_path: &Path,
    ) -> Result<Option<HealthSnapshot>, LifecycleError> {
        read_error: ReadHealth,
        parse_error: ParseHealth,
        parse: serde_json::from_str
    }
}

define_reader! {
    /// Reads and parses the daemon PID from the runtime directory.
    ///
    /// Returns:
    /// * `Ok(Some(pid))` when present and valid.
    /// * `Ok(None)` when absent or empty.
    /// * `Err(ReadPid)` if I/O fails.
    /// * `Err(ParsePid)` if the value is not a valid integer.
    pub(crate) fn read_pid(
        dir: &Dir,
        filename: &str,
        full_path: &Path,
    ) -> Result<Option<u32>, LifecycleError> {
        read_error: ReadPid,
        parse_error: ParsePid,
        parse: |s: &str| s.parse::<u32>()
    }
}
