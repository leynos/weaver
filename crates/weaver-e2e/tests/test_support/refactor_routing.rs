//! Shared request parsing and routing payload helpers for refactor snapshots.

use std::io::{self, Write};
use std::net::TcpStream;
use std::path::Path;

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

/// Builds a capability-resolution `stderr` JSON payload for automatic
/// provider selection based on the file extension.
///
/// Returns `None` when the file extension is not recognised.
pub fn automatic_resolution_payload(file: &Path) -> Option<String> {
    match language_for_extension(file) {
        Some("python") => Some(
            json!({
                "status": "ok",
                "type": "CapabilityResolution",
                "details": {
                    "capability": "rename-symbol",
                    "language": "python",
                    "selected_provider": "rope",
                    "selection_mode": "automatic",
                    "outcome": "selected",
                    "candidates": [
                        { "provider": "rope", "accepted": true, "reason": "matched_language_and_capability" },
                        { "provider": "rust-analyzer", "accepted": false, "reason": "unsupported_language" }
                    ]
                }
            })
            .to_string(),
        ),
        Some("rust") => Some(
            json!({
                "status": "ok",
                "type": "CapabilityResolution",
                "details": {
                    "capability": "rename-symbol",
                    "language": "rust",
                    "selected_provider": "rust-analyzer",
                    "selection_mode": "automatic",
                    "outcome": "selected",
                    "candidates": [
                        { "provider": "rust-analyzer", "accepted": true, "reason": "matched_language_and_capability" },
                        { "provider": "rope", "accepted": false, "reason": "unsupported_language" }
                    ]
                }
            })
            .to_string(),
        ),
        _ => None,
    }
}

/// Builds a refused capability-resolution `stderr` JSON payload for the case
/// where an explicitly requested provider does not support the file's
/// language.
///
/// Returns `None` when the provider and language are compatible, or when the
/// file extension is not recognised.
pub fn provider_mismatch_payload(file: &Path, provider: RequestedProvider) -> Option<String> {
    let language = language_for_extension(file)?;
    let mismatched = matches!(
        (language, provider),
        ("python", RequestedProvider::RustAnalyzer) | ("rust", RequestedProvider::Rope)
    );
    if !mismatched {
        return None;
    }

    Some(
        json!({
            "status": "error",
            "type": "CapabilityResolution",
            "details": {
                "capability": "rename-symbol",
                "language": language,
                "requested_provider": provider.as_str(),
                "selection_mode": "explicit_provider",
                "outcome": "refused",
                "refusal_reason": "explicit_provider_mismatch",
                "candidates": [
                    {
                        "provider": if language == "python" { "rope" } else { "rust-analyzer" },
                        "accepted": false,
                        "reason": "not_requested"
                    },
                    {
                        "provider": provider.as_str(),
                        "accepted": false,
                        "reason": "explicit_provider_mismatch"
                    }
                ]
            }
        })
        .to_string(),
    )
}

/// Writes the complete fake-daemon response for a `refactor` operation to
/// `writer`.
///
/// Depending on the requested provider and the file extension, this writes
/// an optional capability-resolution `stderr` stream record followed by a
/// `stdout` payload and an exit record.
///
/// # Panics
/// Panics if the `--file` argument is missing, the refactoring is not
/// `rename`, `new_name` or `offset` is absent, the file extension is not
/// recognised, or the `--provider` value is not `rope` or `rust-analyzer`.
///
/// # Errors
/// Returns an `io::Error` if writing to `writer` fails.
pub fn write_refactor_response(
    writer: &mut TcpStream,
    operation: Operation,
    arguments: &[&str],
    renamed_symbol: &str,
) -> Result<(), io::Error> {
    let request = validate_refactor_request(arguments);

    if language_for_extension(request.file).is_none() {
        panic_unsupported_extension(request.file);
    }

    if let Some(provider) = request.requested_provider
        && let Some(payload) = provider_mismatch_payload(request.file, provider)
    {
        write_json_line(
            writer,
            &json!({
                "kind": "stream",
                "stream": "stderr",
                "data": payload,
            }),
        )?;
        return write_json_line(writer, &json!({ "kind": "exit", "status": 1 }));
    }

    if request.requested_provider.is_none()
        && let Some(payload) = automatic_resolution_payload(request.file)
    {
        write_json_line(
            writer,
            &json!({
                "kind": "stream",
                "stream": "stderr",
                "data": payload,
            }),
        )?;
    }

    write_stdout_exit(
        writer,
        &response_payload_for_operation(operation, renamed_symbol),
        0,
    )
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

fn requested_provider(arguments: &[&str]) -> Option<RequestedProvider> {
    match argument_value(arguments, "--provider") {
        Some("rope") => Some(RequestedProvider::Rope),
        Some("rust-analyzer") => Some(RequestedProvider::RustAnalyzer),
        Some(other) => panic!(
            "refactor snapshot requests only support --provider rope or rust-analyzer, got {other}"
        ),
        None => None,
    }
}

fn panic_unsupported_extension(file: &Path) -> ! {
    panic!(
        "fake daemon received a refactor request for unsupported file extension: {}; \
         add a routing rule to language_for_extension",
        file.display()
    );
}

fn validate_refactor_request<'a>(arguments: &'a [&'a str]) -> ValidatedRefactorRequest<'a> {
    let Some(refactoring) = argument_value(arguments, "--refactoring") else {
        panic!("refactor snapshot requests must include --refactoring");
    };
    assert_eq!(
        refactoring, "rename",
        "refactor snapshot requests only support --refactoring rename"
    );
    let Some(file) = argument_value(arguments, "--file") else {
        panic!("refactor snapshot requests must include --file");
    };
    let Some(new_name) = arguments
        .iter()
        .find_map(|argument| argument.strip_prefix("new_name="))
    else {
        panic!("refactor snapshot requests must include new_name=<value>");
    };
    assert!(
        !new_name.is_empty(),
        "refactor snapshot requests must include non-empty new_name=<value>"
    );
    let Some(offset) = arguments
        .iter()
        .find_map(|argument| argument.strip_prefix("offset="))
    else {
        panic!("refactor snapshot requests must include offset=<value>");
    };
    assert!(
        offset.parse::<usize>().is_ok(),
        "refactor snapshot requests must include numeric offset=<value>"
    );

    ValidatedRefactorRequest {
        file: Path::new(file),
        requested_provider: requested_provider(arguments),
    }
}

fn write_json_line(writer: &mut impl Write, payload: &serde_json::Value) -> Result<(), io::Error> {
    writer.write_all(payload.to_string().as_bytes())?;
    writer.write_all(b"\n")?;
    writer.flush()
}
