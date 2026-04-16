//! Shared request parsing and routing payload helpers for refactor snapshots.

use std::io::{self, Write};
use std::net::TcpStream;

use serde_json::json;

#[derive(Clone, Copy)]
enum RequestedProvider {
    Rope,
    RustAnalyzer,
}

impl RequestedProvider {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Rope => "rope",
            Self::RustAnalyzer => "rust-analyzer",
        }
    }
}

struct ValidatedRefactorRequest<'a> {
    file: &'a str,
    requested_provider: Option<RequestedProvider>,
}

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

pub fn language_for_extension(file: &str) -> Option<&'static str> {
    match std::path::Path::new(file)
        .extension()
        .and_then(|ext| ext.to_str())
    {
        Some("py") => Some("python"),
        Some("rs") => Some("rust"),
        _ => None,
    }
}

pub fn automatic_resolution_payload(file: &str) -> Option<String> {
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

pub fn provider_mismatch_payload(file: &str, provider: &str) -> Option<String> {
    let language = language_for_extension(file)?;
    let mismatched = matches!(
        (language, provider),
        ("python", "rust-analyzer") | ("rust", "rope")
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
                "requested_provider": provider,
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
                        "provider": provider,
                        "accepted": false,
                        "reason": "explicit_provider_mismatch"
                    }
                ]
            }
        })
        .to_string(),
    )
}

pub fn write_refactor_response(
    writer: &mut TcpStream,
    operation: &str,
    arguments: &[&str],
    renamed_symbol: &str,
) -> Result<(), io::Error> {
    let request = validate_refactor_request(arguments);

    if let Some(provider) = request.requested_provider
        && let Some(payload) = provider_mismatch_payload(request.file, provider.as_str())
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

pub fn response_payload_for_operation(operation: &str, renamed_symbol: &str) -> String {
    match operation {
        "get-definition" => json!([{ "symbol": renamed_symbol }]).to_string(),
        "refactor" => json!({
            "status": "ok",
            "files_written": 1,
            "files_deleted": 0
        })
        .to_string(),
        _ => json!({ "status": "unexpected", "operation": operation }).to_string(),
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
        file,
        requested_provider: requested_provider(arguments),
    }
}

fn write_json_line(writer: &mut impl Write, payload: &serde_json::Value) -> Result<(), io::Error> {
    writer.write_all(payload.to_string().as_bytes())?;
    writer.write_all(b"\n")?;
    writer.flush()
}
