//! Request types for the `observe get-card` operation.
//!
//! The [`GetCardRequest`] struct captures the parsed arguments from a
//! `get-card` command. It is serializable for logging and testability,
//! and provides a [`parse`](GetCardRequest::parse) constructor that
//! accepts the raw argument vector from the daemon's `CommandRequest`.

use serde::{Deserialize, Serialize};

use crate::DetailLevel;
use crate::error::GetCardError;

/// Parsed request for the `observe get-card` operation.
///
/// # Example
///
/// ```
/// use weaver_cards::{DetailLevel, GetCardRequest};
///
/// let args = vec![
///     String::from("--uri"), String::from("file:///src/main.rs"),
///     String::from("--position"), String::from("10:5"),
/// ];
/// let request = GetCardRequest::parse(&args).expect("valid request");
/// assert_eq!(request.detail, DetailLevel::Structure);
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GetCardRequest {
    /// File URI (e.g. `file:///src/main.rs`).
    pub uri: String,
    /// Line number (1-indexed, user-facing).
    pub line: u32,
    /// Column number (1-indexed, user-facing).
    pub column: u32,
    /// Requested detail level.
    #[serde(default)]
    pub detail: DetailLevel,
}

impl GetCardRequest {
    /// Parses a `get-card` request from a CLI argument list.
    ///
    /// Expects `--uri <URI> --position <LINE:COL>` format with optional
    /// `--detail <LEVEL>` and `--format <FORMAT>` flags. Arguments can
    /// appear in any order. `--uri` and `--position` are required.
    ///
    /// # Errors
    ///
    /// Returns [`GetCardError`] if required flags are missing, values are
    /// malformed, or an unknown flag is encountered.
    pub fn parse(arguments: &[String]) -> Result<Self, GetCardError> {
        let mut uri: Option<String> = None;
        let mut position: Option<(u32, u32)> = None;
        let mut detail = DetailLevel::default();

        let mut iter = arguments.iter().peekable();
        while let Some(arg) = iter.next() {
            match arg.as_str() {
                "--uri" => {
                    let value = require_arg_value(&mut iter, "--uri")?;
                    uri = Some(String::from(value));
                }
                "--position" => {
                    let value = require_arg_value(&mut iter, "--position")?;
                    position = Some(parse_position(value)?);
                }
                "--detail" => {
                    let value = require_arg_value(&mut iter, "--detail")?;
                    detail = parse_detail(value)?;
                }
                "--format" => {
                    let value = require_arg_value(&mut iter, "--format")?;
                    validate_format(value)?;
                }
                other => {
                    return Err(GetCardError::UnknownArgument {
                        argument: String::from(other),
                    });
                }
            }
        }

        let resolved_uri = uri.ok_or_else(|| GetCardError::MissingArgument {
            flag: String::from("--uri"),
        })?;
        let (line, column) = position.ok_or_else(|| GetCardError::MissingArgument {
            flag: String::from("--position"),
        })?;

        Ok(Self {
            uri: resolved_uri,
            line,
            column,
            detail,
        })
    }
}

/// Extracts the next argument value or returns an error.
fn require_arg_value<'a, I>(iter: &mut I, flag: &str) -> Result<&'a str, GetCardError>
where
    I: Iterator<Item = &'a String>,
{
    iter.next()
        .map(String::as_str)
        .ok_or_else(|| GetCardError::InvalidValue {
            flag: String::from(flag),
            message: String::from("requires a value"),
        })
}

/// Parses a detail level string via [`DetailLevel::from_str`].
fn parse_detail(value: &str) -> Result<DetailLevel, GetCardError> {
    value
        .parse()
        .map_err(|e: crate::DetailLevelParseError| GetCardError::InvalidValue {
            flag: String::from("--detail"),
            message: e.to_string(),
        })
}

/// Validates that the format flag value is `"json"`.
fn validate_format(value: &str) -> Result<(), GetCardError> {
    if value != "json" {
        return Err(GetCardError::InvalidValue {
            flag: String::from("--format"),
            message: format!("unsupported format: {value}; only \"json\" is supported"),
        });
    }
    Ok(())
}

/// Parses a position string in `LINE:COL` format.
fn parse_position(value: &str) -> Result<(u32, u32), GetCardError> {
    let (line_str, col_str) = value
        .split_once(':')
        .ok_or_else(|| GetCardError::InvalidValue {
            flag: String::from("--position"),
            message: format!("expected LINE:COL, got: {value}"),
        })?;

    let line: u32 = line_str.parse().map_err(|_| GetCardError::InvalidValue {
        flag: String::from("--position"),
        message: format!("invalid line number: {line_str}"),
    })?;
    let column: u32 = col_str.parse().map_err(|_| GetCardError::InvalidValue {
        flag: String::from("--position"),
        message: format!("invalid column number: {col_str}"),
    })?;

    if line == 0 {
        return Err(GetCardError::InvalidValue {
            flag: String::from("--position"),
            message: String::from("line number must be >= 1"),
        });
    }
    if column == 0 {
        return Err(GetCardError::InvalidValue {
            flag: String::from("--position"),
            message: String::from("column number must be >= 1"),
        });
    }

    Ok((line, column))
}

#[cfg(test)]
mod tests {
    use rstest::rstest;

    use super::*;

    fn args(items: &[&str]) -> Vec<String> {
        items.iter().map(|s| String::from(*s)).collect()
    }

    #[test]
    fn parses_valid_minimal_arguments() {
        let arguments = args(&["--uri", "file:///src/main.rs", "--position", "10:5"]);
        let request = GetCardRequest::parse(&arguments).expect("should parse");

        assert_eq!(request.uri, "file:///src/main.rs");
        assert_eq!(request.line, 10);
        assert_eq!(request.column, 5);
        assert_eq!(request.detail, DetailLevel::Structure);
    }

    #[test]
    fn parses_arguments_in_any_order() {
        let arguments = args(&[
            "--position",
            "42:17",
            "--detail",
            "minimal",
            "--uri",
            "file:///lib.rs",
        ]);
        let request = GetCardRequest::parse(&arguments).expect("should parse");

        assert_eq!(request.uri, "file:///lib.rs");
        assert_eq!(request.line, 42);
        assert_eq!(request.column, 17);
        assert_eq!(request.detail, DetailLevel::Minimal);
    }

    #[test]
    fn parses_json_format_flag() {
        let arguments = args(&[
            "--uri",
            "file:///main.rs",
            "--position",
            "1:1",
            "--format",
            "json",
        ]);
        let request = GetCardRequest::parse(&arguments).expect("should parse");
        assert_eq!(request.detail, DetailLevel::Structure);
    }

    #[rstest]
    #[case::minimal("minimal", DetailLevel::Minimal)]
    #[case::signature("signature", DetailLevel::Signature)]
    #[case::structure("structure", DetailLevel::Structure)]
    #[case::semantic("semantic", DetailLevel::Semantic)]
    #[case::full("full", DetailLevel::Full)]
    fn parses_all_detail_levels(#[case] level: &str, #[case] expected: DetailLevel) {
        let arguments = args(&[
            "--uri",
            "file:///main.rs",
            "--position",
            "1:1",
            "--detail",
            level,
        ]);
        let request = GetCardRequest::parse(&arguments).expect("should parse");
        assert_eq!(request.detail, expected);
    }

    #[rstest]
    #[case::missing_uri(&["--position", "10:5"], "--uri")]
    #[case::missing_position(&["--uri", "file:///main.rs"], "--position")]
    #[case::bad_position_format(
        &["--uri", "file:///main.rs", "--position", "10"],
        "LINE:COL"
    )]
    #[case::zero_line(
        &["--uri", "file:///main.rs", "--position", "0:5"],
        "line"
    )]
    #[case::zero_column(
        &["--uri", "file:///main.rs", "--position", "1:0"],
        "column"
    )]
    #[case::unknown_flag(
        &["--uri", "file:///main.rs", "--position", "1:1", "--bogus"],
        "--bogus"
    )]
    #[case::bad_detail(
        &["--uri", "file:///main.rs", "--position", "1:1", "--detail", "extreme"],
        "extreme"
    )]
    #[case::bad_format(
        &["--uri", "file:///main.rs", "--position", "1:1", "--format", "xml"],
        "xml"
    )]
    fn rejects_invalid_arguments(#[case] arg_list: &[&str], #[case] expected_substring: &str) {
        let arguments = args(arg_list);
        let error = GetCardRequest::parse(&arguments).expect_err("should fail");
        let message = error.to_string();
        assert!(
            message.contains(expected_substring),
            "expected error to contain {expected_substring:?}, got: {message}"
        );
    }
}
