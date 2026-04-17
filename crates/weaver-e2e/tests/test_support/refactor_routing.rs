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

struct ValidatedRefactorRequest<'a> {
    file: &'a Path,
    requested_provider: Option<RequestedProvider>,
}

/// Extracts the string CLI arguments from a daemon request envelope.
///
/// The request JSON is expected to carry an `arguments` array at the top
/// level. Non-string entries are ignored so malformed requests can still be
/// inspected in tests. Example: `{"arguments":["act","refactor"]}` yields
/// `vec!["act", "refactor"]`.
pub fn request_arguments(parsed_request: &serde_json::Value) -> Vec<&str> {
    parsed_request
        .get("arguments")
        .and_then(serde_json::Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(serde_json::Value::as_str)
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

/// Maps a file path to the language label used by refactor routing snapshots.
///
/// `.py` resolves to `python`, `.rs` resolves to `rust`, and unsupported
/// extensions return `None`. Example: `Path::new("src/main.py")` returns
/// `Some("python")`.
pub fn language_for_extension(file: &Path) -> Option<&'static str> {
    match file.extension().and_then(|ext| ext.to_str()) {
        Some("py") => Some("python"),
        Some("rs") => Some("rust"),
        _ => None,
    }
}

/// Builds the automatic routing payload written to stderr for supported files.
///
/// The returned JSON string mirrors the daemon's `CapabilityResolution`
/// selection envelope. Unsupported extensions return `None` so callers can
/// emit an explicit refusal instead of a success transcript.
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

/// Builds the explicit-provider mismatch refusal payload for supported files.
///
/// The JSON mirrors the daemon's stderr refusal envelope with
/// `status: "error"` and a full candidate list. Matching providers or
/// unsupported extensions return `None`.
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

/// Writes the fake daemon response for a refactor request.
///
/// Capability-resolution notifications and refusals are written to stderr.
/// Successful synthetic operation payloads are written to stdout, followed by
/// an `exit` record. Unsupported extensions without an explicit provider are
/// refused with exit status `1`.
pub fn write_refactor_response(
    writer: &mut TcpStream,
    operation: Operation,
    arguments: &[&str],
    renamed_symbol: &str,
) -> Result<(), io::Error> {
    let request = validate_refactor_request(arguments);

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

    if request.requested_provider.is_none() && language_for_extension(request.file).is_none() {
        write_json_line(
            writer,
            &json!({
                "kind": "stream",
                "stream": "stderr",
                "data": unsupported_language_payload(),
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

/// Writes a stdout stream event followed by an exit event.
///
/// `payload` is embedded as the `data` field of the stdout event and `status`
/// becomes the process-style exit code in the trailing event.
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

/// Builds the stdout payload for a successful synthetic operation.
///
/// `get-definition` returns a one-element symbol array, `refactor` returns the
/// summary object used by the snapshot harness, and unknown operations are
/// wrapped in an `unexpected` diagnostic object.
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
    assert!(
        arguments
            .iter()
            .any(|argument| argument.starts_with("new_name=")),
        "refactor snapshot requests must include new_name=<value>"
    );
    assert!(
        arguments
            .iter()
            .any(|argument| argument.starts_with("offset=")),
        "refactor snapshot requests must include offset=<value>"
    );

    ValidatedRefactorRequest {
        file: Path::new(file),
        requested_provider: requested_provider(arguments),
    }
}

fn unsupported_language_payload() -> String {
    json!({
        "status": "error",
        "type": "CapabilityResolution",
        "details": {
            "capability": "rename-symbol",
            "language": serde_json::Value::Null,
            "selection_mode": "automatic",
            "outcome": "refused",
            "refusal_reason": "unsupported_language",
            "candidates": [
                {
                    "provider": "rope",
                    "accepted": false,
                    "reason": "unsupported_language"
                },
                {
                    "provider": "rust-analyzer",
                    "accepted": false,
                    "reason": "unsupported_language"
                }
            ]
        }
    })
    .to_string()
}

fn write_json_line(writer: &mut impl Write, payload: &serde_json::Value) -> Result<(), io::Error> {
    writer.write_all(payload.to_string().as_bytes())?;
    writer.write_all(b"\n")?;
    writer.flush()
}
