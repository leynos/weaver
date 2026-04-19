//! Request types for the `observe get-card` operation.
//!
//! The [`GetCardRequest`] struct captures the parsed arguments from a
//! `get-card` command. It is serializable for logging and testability,
//! and provides a [`parse`](GetCardRequest::parse) constructor that
//! accepts the raw argument vector from the daemon's `CommandRequest`.

use serde::{Deserialize, Serialize};

use crate::{DetailLevel, error::GetCardError};

/// Parsed request for the `observe get-card` operation.
///
/// # Example
///
/// ```
/// use weaver_cards::{DetailLevel, GetCardRequest};
///
/// let args = vec![
///     String::from("--uri"),
///     String::from("file:///src/main.rs"),
///     String::from("--position"),
///     String::from("10:5"),
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

/// Intermediate state while parsing CLI arguments.
#[derive(Default)]
struct ParseState {
    uri: Option<String>,
    position: Option<(u32, u32)>,
    detail: DetailLevel,
}

impl ParseState {
    /// Applies a single argument to the parse state.
    fn apply_arg(
        &mut self,
        arg: &str,
        iter: &mut std::iter::Peekable<std::slice::Iter<'_, String>>,
    ) -> Result<(), GetCardError> {
        match arg {
            "--uri" => self.parse_uri(iter),
            "--position" => self.parse_position(iter),
            "--detail" => self.parse_detail(iter),
            "--format" => Self::parse_format(iter),
            other if other.starts_with("--") => {
                skip_unknown_flag_value(iter);
                Ok(())
            }
            other => Err(GetCardError::UnknownArgument {
                argument: String::from(other),
            }),
        }
    }

    /// Parses the `--uri` argument.
    fn parse_uri(
        &mut self,
        iter: &mut std::iter::Peekable<std::slice::Iter<'_, String>>,
    ) -> Result<(), GetCardError> {
        let value = require_arg_value(iter, "--uri")?;
        self.uri = Some(String::from(value));
        Ok(())
    }

    /// Parses the `--position` argument.
    fn parse_position(
        &mut self,
        iter: &mut std::iter::Peekable<std::slice::Iter<'_, String>>,
    ) -> Result<(), GetCardError> {
        let value = require_arg_value(iter, "--position")?;
        self.position = Some(parse_position(value)?);
        Ok(())
    }

    /// Parses the `--detail` argument.
    fn parse_detail(
        &mut self,
        iter: &mut std::iter::Peekable<std::slice::Iter<'_, String>>,
    ) -> Result<(), GetCardError> {
        let value = require_arg_value(iter, "--detail")?;
        self.detail = parse_detail(value)?;
        Ok(())
    }

    /// Parses the `--format` argument.
    fn parse_format(
        iter: &mut std::iter::Peekable<std::slice::Iter<'_, String>>,
    ) -> Result<(), GetCardError> {
        let value = require_arg_value(iter, "--format")?;
        validate_format(value)
    }
}

impl TryFrom<ParseState> for GetCardRequest {
    type Error = GetCardError;

    fn try_from(state: ParseState) -> Result<Self, Self::Error> {
        let uri = state.uri.ok_or_else(|| GetCardError::MissingArgument {
            flag: String::from("--uri"),
        })?;
        let (line, column) = state
            .position
            .ok_or_else(|| GetCardError::MissingArgument {
                flag: String::from("--position"),
            })?;

        Ok(Self {
            uri,
            line,
            column,
            detail: state.detail,
        })
    }
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
    /// malformed, or a non-flag positional token is encountered. Unknown
    /// `--` prefixed flags are silently skipped for forward compatibility.
    pub fn parse(arguments: &[String]) -> Result<Self, GetCardError> {
        let mut state = ParseState::default();
        let mut iter = arguments.iter().peekable();

        while let Some(arg) = iter.next() {
            state.apply_arg(arg, &mut iter)?;
        }

        state.try_into()
    }
}

/// Extracts the next argument value or returns an error.
///
/// Returns an error if the iterator is exhausted or the next token
/// looks like a flag (starts with `'-'`).
fn require_arg_value<'a, I>(iter: &mut I, flag: &str) -> Result<&'a str, GetCardError>
where
    I: Iterator<Item = &'a String>,
{
    match iter.next().map(String::as_str) {
        Some(value) if value.starts_with('-') => Err(GetCardError::InvalidValue {
            flag: String::from(flag),
            message: String::from("requires a value"),
        }),
        Some(value) => Ok(value),
        None => Err(GetCardError::InvalidValue {
            flag: String::from(flag),
            message: String::from("requires a value"),
        }),
    }
}

/// Parses a detail level string via [`DetailLevel::from_str`].
fn parse_detail(value: &str) -> Result<DetailLevel, GetCardError> {
    value.parse().map_err(
        |e: crate::DetailLevelParseError| GetCardError::InvalidValue {
            flag: String::from("--detail"),
            message: e.to_string(),
        },
    )
}

/// Consumes the next token if it does not look like a flag.
///
/// This allows unknown `--` prefixed flags to consume their value
/// argument without producing an error.
fn skip_unknown_flag_value<'a, I>(iter: &mut std::iter::Peekable<I>)
where
    I: Iterator<Item = &'a String>,
{
    let is_value = iter.peek().is_some_and(|next| !next.starts_with('-'));
    if is_value {
        iter.next();
    }
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
    //! Unit tests for [`GetCardRequest`] parsing and validation.

    use rstest::rstest;

    use super::*;

    fn args(items: &[&str]) -> Vec<String> { items.iter().map(|s| String::from(*s)).collect() }

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

    #[test]
    fn skips_unknown_flags() {
        let arguments = args(&[
            "--uri",
            "file:///main.rs",
            "--position",
            "1:1",
            "--bogus",
            "whatever",
            "--experimental",
        ]);
        let request = GetCardRequest::parse(&arguments).expect("should parse");
        assert_eq!(request.uri, "file:///main.rs");
        assert_eq!(request.line, 1);
        assert_eq!(request.column, 1);
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
    #[case::positional_token(
        &["--uri", "file:///main.rs", "--position", "1:1", "stray"],
        "stray"
    )]
    #[case::bad_detail(
        &["--uri", "file:///main.rs", "--position", "1:1", "--detail", "extreme"],
        "extreme"
    )]
    #[case::bad_format(
        &["--uri", "file:///main.rs", "--position", "1:1", "--format", "xml"],
        "xml"
    )]
    #[case::flag_as_uri_value(
        &["--uri", "--position", "1:1"],
        "requires a value"
    )]
    #[case::flag_as_position_value(
        &["--uri", "file:///main.rs", "--position", "--detail"],
        "requires a value"
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
