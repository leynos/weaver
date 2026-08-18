//! Shared request parsing and routing payload helpers for refactor snapshots.

// This file is itself included via `#[path]` from each snapshot test crate,
// so the child module needs an explicit path to resolve beside it.
#[path = "refactor_routing/payloads.rs"]
mod payloads;

use std::{
    io::{self, Write},
    net::TcpStream,
    path::Path,
};

use payloads::{automatic_resolution_payload, provider_mismatch_payload};
use serde_json::json;

/// Explicit provider override values supported by the refactor snapshots.
#[derive(Clone, Copy)]
pub enum RequestedProvider {
    Rope,
    RustAnalyzer,
}

impl RequestedProvider {
    /// Returns the CLI spelling used by the daemon payloads.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Rope => "rope",
            Self::RustAnalyzer => "rust-analyzer",
        }
    }
}

/// The daemon operation being exercised in a snapshot test.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Operation {
    GetDefinition,
    Refactor,
    Other(String),
}

impl<'a> From<&'a str> for Operation {
    fn from(s: &'a str) -> Self {
        match s {
            "get-definition" => Self::GetDefinition,
            "refactor" => Self::Refactor,
            other => Self::Other(other.to_owned()),
        }
    }
}

/// Carries the validated refactor request returned from
/// `validate_refactor_request`.
///
/// `file` is the path to the target file, and `requested_provider` is the
/// optional `RequestedProvider` chosen during validation.
struct ValidatedRefactorRequest<'a> {
    file: &'a Path,
    requested_provider: Option<RequestedProvider>,
}

/// Extracts the flat list of CLI argument strings from a parsed daemon
/// request JSON value.
///
/// Returns `io::ErrorKind::InvalidData` if the `arguments` array contains a
/// non-string entry.
pub fn request_arguments(parsed_request: &serde_json::Value) -> io::Result<Vec<&str>> {
    parsed_request
        .get("arguments")
        .and_then(serde_json::Value::as_array)
        .into_iter()
        .flatten()
        .map(|argument| {
            argument.as_str().ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    "daemon request arguments must all be JSON strings",
                )
            })
        })
        .collect()
}

/// Returns the value that follows the first matching flag in `arguments`.
///
/// The scan uses `windows(2)`, so only the first matching flag is considered
/// and later duplicate flags are ignored. If the flag is the final argument or
/// no following value exists, this returns `None`.
pub fn argument_value<'a>(arguments: &'a [&str], flag: &str) -> Option<&'a str> {
    arguments.windows(2).find_map(|pair| {
        let current = pair.first().copied()?;
        let next = pair.get(1).copied()?;
        (current == flag).then_some(next)
    })
}

/// Infers the language identifier from a file's extension.
///
/// Returns `Some("python")` for `.py`, `Some("rust")` for `.rs`, and `None`
/// for all other extensions.
pub fn language_for_extension(file: &Path) -> Option<&'static str> {
    match file.extension().and_then(|ext| ext.to_str()) {
        Some("py") => Some("python"),
        Some("rs") => Some("rust"),
        _ => None,
    }
}

/// Writes the complete fake-daemon response for a `refactor` operation to
/// `writer`.
///
/// Depending on the requested provider and the file extension, this writes
/// an optional capability-resolution `stderr` stream record followed by a
/// `stdout` payload and an exit record.
///
/// # Errors
/// Returns `io::ErrorKind::InvalidData` when the request is malformed — a
/// missing or unsupported `--refactoring`, `--file`, `new_name=`, `--position`,
/// or `--provider`, or a file extension with no routing rule — and any other
/// `io::Error` when writing to `writer` fails.
pub fn write_refactor_response(
    writer: &mut TcpStream,
    operation: Operation,
    arguments: &[&str],
    renamed_symbol: &str,
) -> Result<(), io::Error> {
    let request = validate_refactor_request(arguments)?;

    if language_for_extension(request.file).is_none() {
        return Err(unsupported_extension_error(request.file));
    }

    if write_provider_mismatch_response(writer, &request)? {
        return write_json_line(writer, &json!({ "kind": "exit", "status": 1 }));
    }

    write_automatic_resolution_stream(writer, &request)?;

    write_stdout_exit(
        writer,
        &response_payload_for_operation(operation, renamed_symbol),
        0,
    )
}

fn write_provider_mismatch_response(
    writer: &mut TcpStream,
    request: &ValidatedRefactorRequest<'_>,
) -> Result<bool, io::Error> {
    let Some(provider) = request.requested_provider else {
        return Ok(false);
    };
    let Some(payload) = provider_mismatch_payload(request.file, provider) else {
        return Ok(false);
    };
    write_optional_stderr_stream(writer, Some(payload))
}

fn write_automatic_resolution_stream(
    writer: &mut TcpStream,
    request: &ValidatedRefactorRequest<'_>,
) -> Result<(), io::Error> {
    if request.requested_provider.is_some() {
        return Ok(());
    }
    let Some(payload) = automatic_resolution_payload(request.file) else {
        return Ok(());
    };
    write_stderr_stream(writer, &payload)
}

/// Writes a `stdout` stream record containing `payload` followed by an exit
/// record with the given `status` code.
///
/// # Errors
/// Returns an `io::Error` if writing to `writer` fails.
pub fn write_stdout_exit(
    writer: &mut TcpStream,
    payload: &str,
    status: i32,
) -> Result<(), io::Error> {
    write_json_line(
        writer,
        &json!({
            "kind": "stream",
            "stream": "stdout",
            "data": payload,
        }),
    )?;
    write_json_line(writer, &json!({ "kind": "exit", "status": status }))
}

/// Returns a deterministic JSON string suitable for the `stdout` stream of
/// the given operation, incorporating `renamed_symbol` where the response
/// schema requires a symbol name.
pub fn response_payload_for_operation(operation: Operation, renamed_symbol: &str) -> String {
    match operation {
        Operation::GetDefinition => json!([{ "symbol": renamed_symbol }]).to_string(),
        Operation::Refactor => json!({
            "status": "ok",
            "files_written": 1,
            "files_deleted": 0
        })
        .to_string(),
        Operation::Other(op) => json!({ "status": "unexpected", "operation": op }).to_string(),
    }
}

/// Builds the `InvalidData` error used to report a malformed snapshot request.
///
/// The fake daemon surfaces these through its response thread, so a broken test
/// request fails loudly instead of producing a misleading transcript.
fn invalid_request(message: impl Into<String>) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message.into())
}

/// Reports a file extension for which no routing rule exists.
fn unsupported_extension_error(file: &Path) -> io::Error {
    invalid_request(format!(
        "fake daemon received a refactor request for unsupported file extension: {}; add a \
         routing rule to language_for_extension",
        file.display()
    ))
}

fn requested_provider(arguments: &[&str]) -> io::Result<Option<RequestedProvider>> {
    match argument_value(arguments, "--provider") {
        Some("rope") => Ok(Some(RequestedProvider::Rope)),
        Some("rust-analyzer") => Ok(Some(RequestedProvider::RustAnalyzer)),
        Some(other) => Err(invalid_request(format!(
            "refactor snapshot requests only support --provider rope or rust-analyzer, got {other}"
        ))),
        None => Ok(None),
    }
}

/// Checks `--refactoring` names the only refactoring these snapshots cover.
fn validate_refactoring(arguments: &[&str]) -> io::Result<()> {
    match argument_value(arguments, "--refactoring") {
        Some("rename") => Ok(()),
        Some(other) => Err(invalid_request(format!(
            "refactor snapshot requests only support --refactoring rename, got {other}"
        ))),
        None => Err(invalid_request(
            "refactor snapshot requests must include --refactoring",
        )),
    }
}

/// Checks a non-empty `new_name=<value>` positional argument is present.
fn validate_new_name(arguments: &[&str]) -> io::Result<()> {
    let Some(new_name) = arguments
        .iter()
        .find_map(|argument| argument.strip_prefix("new_name="))
    else {
        return Err(invalid_request(
            "refactor snapshot requests must include new_name=<value>",
        ));
    };
    if new_name.is_empty() {
        return Err(invalid_request(
            "refactor snapshot requests must include non-empty new_name=<value>",
        ));
    }
    Ok(())
}

/// Checks `--position` carries a one-indexed `LINE:COL` pair.
fn validate_position(arguments: &[&str]) -> io::Result<()> {
    const EXPECTATION: &str =
        "refactor snapshot requests must include one-indexed --position <LINE:COL>";

    let Some((line, column)) =
        argument_value(arguments, "--position").and_then(|position| position.split_once(':'))
    else {
        return Err(invalid_request(EXPECTATION));
    };
    let is_one_indexed = |value: &str| value.parse::<u32>().is_ok_and(|parsed| parsed >= 1);
    if is_one_indexed(line) && is_one_indexed(column) {
        Ok(())
    } else {
        Err(invalid_request(EXPECTATION))
    }
}

/// Validates a refactor request and extracts the parts the responder needs.
///
/// # Errors
/// Returns `io::ErrorKind::InvalidData` describing the first malformed or
/// missing argument.
fn validate_refactor_request<'a>(
    arguments: &'a [&'a str],
) -> io::Result<ValidatedRefactorRequest<'a>> {
    validate_refactoring(arguments)?;
    validate_new_name(arguments)?;
    validate_position(arguments)?;

    let Some(file) = argument_value(arguments, "--file") else {
        return Err(invalid_request(
            "refactor snapshot requests must include --file",
        ));
    };

    Ok(ValidatedRefactorRequest {
        file: Path::new(file),
        requested_provider: requested_provider(arguments)?,
    })
}

fn write_stderr_stream(writer: &mut TcpStream, payload: &str) -> io::Result<()> {
    write_json_line(
        writer,
        &json!({ "kind": "stream", "stream": "stderr", "data": payload }),
    )
}

fn write_optional_stderr_stream(
    writer: &mut TcpStream,
    payload: Option<String>,
) -> io::Result<bool> {
    if let Some(stream_payload) = payload {
        write_stderr_stream(writer, &stream_payload)?;
        return Ok(true);
    }
    Ok(false)
}

fn write_json_line(writer: &mut impl Write, payload: &serde_json::Value) -> Result<(), io::Error> {
    writer.write_all(payload.to_string().as_bytes())?;
    writer.write_all(b"\n")?;
    writer.flush()
}
