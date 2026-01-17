//! Argument parsing for observe domain operations.
//!
//! This module provides typed argument structs for each observe operation,
//! parsing CLI arguments from the `CommandRequest::arguments` vector into
//! strongly-typed values suitable for calling backend services.

use lsp_types::{
    GotoDefinitionParams, Position, TextDocumentIdentifier, TextDocumentPositionParams, Uri,
};
use weaver_lsp_host::Language;

use crate::dispatch::errors::DispatchError;

/// Parsed arguments for the `get-definition` operation.
///
/// # Example
///
/// ```text
/// weaver observe get-definition --uri file:///src/main.rs --position 10:5
/// ```
#[derive(Debug, Clone)]
pub struct GetDefinitionArgs {
    /// The document URI.
    pub uri: Uri,
    /// Line number (1-indexed for user-facing).
    pub line: u32,
    /// Column number (1-indexed for user-facing).
    pub column: u32,
}

impl GetDefinitionArgs {
    /// Parses arguments from a CLI argument list.
    ///
    /// Expects `--uri <URI> --position <LINE:COL>` format. Arguments can appear
    /// in any order. Both flags are required.
    ///
    /// # Errors
    ///
    /// Returns `InvalidArguments` if required flags are missing, values are
    /// malformed, or the URI cannot be parsed.
    pub fn parse(arguments: &[String]) -> Result<Self, DispatchError> {
        let mut uri: Option<Uri> = None;
        let mut position: Option<(u32, u32)> = None;

        let mut iter = arguments.iter().peekable();
        while let Some(arg) = iter.next() {
            match arg.as_str() {
                "--uri" => {
                    let value = require_arg_value(&mut iter, "--uri")?;
                    uri = Some(parse_uri(value)?);
                }
                "--position" => {
                    let value = require_arg_value(&mut iter, "--position")?;
                    position = Some(parse_position(value)?);
                }
                other => {
                    return Err(DispatchError::invalid_arguments(format!(
                        "unknown argument: {other}"
                    )));
                }
            }
        }

        let uri = uri.ok_or_else(|| DispatchError::invalid_arguments("missing required --uri"))?;
        let (line, column) = position
            .ok_or_else(|| DispatchError::invalid_arguments("missing required --position"))?;

        Ok(Self { uri, line, column })
    }

    /// Infers the language from the URI's file extension.
    ///
    /// # Errors
    ///
    /// Returns `UnsupportedLanguage` if the file extension is not recognised.
    pub fn language(&self) -> Result<Language, DispatchError> {
        let path = self.uri.path().as_str();
        let extension = path
            .rsplit('.')
            .next()
            .ok_or_else(|| DispatchError::unsupported_language("(no extension)"))?;

        match extension.to_ascii_lowercase().as_str() {
            "rs" => Ok(Language::Rust),
            "py" => Ok(Language::Python),
            "ts" | "tsx" => Ok(Language::TypeScript),
            other => Err(DispatchError::unsupported_language(other)),
        }
    }

    /// Converts to LSP `GotoDefinitionParams`.
    ///
    /// Line and column are converted from 1-indexed (user-facing) to 0-indexed
    /// (LSP protocol).
    #[must_use]
    pub fn into_params(self) -> GotoDefinitionParams {
        GotoDefinitionParams {
            text_document_position_params: TextDocumentPositionParams {
                text_document: TextDocumentIdentifier { uri: self.uri },
                position: Position {
                    line: self.line.saturating_sub(1),
                    character: self.column.saturating_sub(1),
                },
            },
            work_done_progress_params: Default::default(),
            partial_result_params: Default::default(),
        }
    }
}

/// Extracts the next argument value or returns an error.
fn require_arg_value<'a, I>(iter: &mut I, flag: &str) -> Result<&'a str, DispatchError>
where
    I: Iterator<Item = &'a String>,
{
    iter.next()
        .map(String::as_str)
        .ok_or_else(|| DispatchError::invalid_arguments(format!("{flag} requires a value")))
}

/// Parses a URI string into an `lsp_types::Uri`.
fn parse_uri(value: &str) -> Result<Uri, DispatchError> {
    value
        .parse()
        .map_err(|_| DispatchError::invalid_arguments(format!("invalid URI: {value}")))
}

/// Parses a position string in `LINE:COL` format.
fn parse_position(value: &str) -> Result<(u32, u32), DispatchError> {
    let parts: Vec<&str> = value.split(':').collect();
    if parts.len() != 2 {
        return Err(DispatchError::invalid_arguments(format!(
            "position must be LINE:COL, got: {value}"
        )));
    }

    let line: u32 = parts[0].parse().map_err(|_| {
        DispatchError::invalid_arguments(format!("invalid line number: {}", parts[0]))
    })?;
    let column: u32 = parts[1].parse().map_err(|_| {
        DispatchError::invalid_arguments(format!("invalid column number: {}", parts[1]))
    })?;

    if line == 0 {
        return Err(DispatchError::invalid_arguments("line number must be >= 1"));
    }
    if column == 0 {
        return Err(DispatchError::invalid_arguments(
            "column number must be >= 1",
        ));
    }

    Ok((line, column))
}

#[cfg(test)]
mod tests {
    use rstest::rstest;

    use super::*;

    fn args(items: &[&str]) -> Vec<String> {
        items.iter().map(|s| (*s).to_string()).collect()
    }

    /// Asserts that parsing the given arguments fails with `InvalidArguments`
    /// and the error message contains the expected substring.
    #[track_caller]
    fn assert_invalid_arguments(arg_list: &[&str], expected_substring: &str) {
        let arguments = args(arg_list);
        let error = GetDefinitionArgs::parse(&arguments).expect_err("should fail");

        assert!(
            matches!(error, DispatchError::InvalidArguments { .. }),
            "expected InvalidArguments, got: {error:?}"
        );
        assert!(
            error.to_string().contains(expected_substring),
            "expected error to contain {expected_substring:?}, got: {error}"
        );
    }

    #[test]
    fn parses_valid_arguments() {
        let arguments = args(&["--uri", "file:///src/main.rs", "--position", "10:5"]);
        let parsed = GetDefinitionArgs::parse(&arguments).expect("should parse");

        assert_eq!(parsed.uri.to_string(), "file:///src/main.rs");
        assert_eq!(parsed.line, 10);
        assert_eq!(parsed.column, 5);
    }

    #[test]
    fn parses_arguments_in_reverse_order() {
        let arguments = args(&["--position", "42:17", "--uri", "file:///lib.rs"]);
        let parsed = GetDefinitionArgs::parse(&arguments).expect("should parse");

        assert_eq!(parsed.uri.to_string(), "file:///lib.rs");
        assert_eq!(parsed.line, 42);
        assert_eq!(parsed.column, 17);
    }

    #[test]
    fn rejects_missing_uri() {
        assert_invalid_arguments(&["--position", "10:5"], "--uri");
    }

    #[test]
    fn rejects_missing_position() {
        assert_invalid_arguments(&["--uri", "file:///main.rs"], "--position");
    }

    #[test]
    fn rejects_malformed_position() {
        assert_invalid_arguments(
            &["--uri", "file:///main.rs", "--position", "10"],
            "LINE:COL",
        );
    }

    #[test]
    fn rejects_zero_line() {
        assert_invalid_arguments(&["--uri", "file:///main.rs", "--position", "0:5"], "line");
    }

    #[test]
    fn rejects_unknown_argument() {
        assert_invalid_arguments(
            &[
                "--uri",
                "file:///main.rs",
                "--position",
                "10:5",
                "--unknown",
            ],
            "unknown",
        );
    }

    #[rstest]
    #[case("file:///main.rs", Language::Rust)]
    #[case("file:///lib.rs", Language::Rust)]
    #[case("file:///script.py", Language::Python)]
    #[case("file:///app.ts", Language::TypeScript)]
    #[case("file:///component.tsx", Language::TypeScript)]
    fn infers_language_from_extension(#[case] uri: &str, #[case] expected: Language) {
        let arguments = args(&["--uri", uri, "--position", "1:1"]);
        let parsed = GetDefinitionArgs::parse(&arguments).expect("should parse");
        let language = parsed.language().expect("should infer language");
        assert_eq!(language, expected);
    }

    #[test]
    fn rejects_unsupported_extension() {
        let arguments = args(&["--uri", "file:///main.xyz", "--position", "1:1"]);
        let parsed = GetDefinitionArgs::parse(&arguments).expect("should parse");
        let error = parsed.language().expect_err("should fail");

        assert!(matches!(error, DispatchError::UnsupportedLanguage { .. }));
    }

    #[test]
    fn converts_to_lsp_params_with_zero_indexed_position() {
        let arguments = args(&["--uri", "file:///main.rs", "--position", "10:5"]);
        let parsed = GetDefinitionArgs::parse(&arguments).expect("should parse");
        let params = parsed.into_params();

        // User-facing 10:5 becomes LSP 9:4 (0-indexed)
        assert_eq!(params.text_document_position_params.position.line, 9);
        assert_eq!(params.text_document_position_params.position.character, 4);
    }
}
