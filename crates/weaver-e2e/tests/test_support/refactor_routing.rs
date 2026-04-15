//! Shared helpers for refactor-routing end-to-end snapshots.

use std::io::Write;
use std::net::TcpStream;

use serde_json::json;

/// Identifies a rename-symbol plugin provider in test scenarios.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Provider {
    Rope,
    RustAnalyzer,
}

impl Provider {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Rope => "rope",
            Self::RustAnalyzer => "rust-analyzer",
        }
    }
}

/// Identifies the daemon operation being exercised in test scenarios.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Operation {
    Refactor,
    Other,
}

#[must_use]
pub fn classify_operation(operation: &str) -> Operation {
    if matches!(operation, "refactor") {
        Operation::Refactor
    } else {
        Operation::Other
    }
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

pub fn argument_value<'a>(arguments: &'a [&str], flag: &str) -> Option<&'a str> {
    arguments.windows(2).find_map(|pair| {
        let current = pair.first().copied()?;
        let next = pair.get(1).copied()?;
        (current == flag).then_some(next)
    })
}

fn build_capability_resolution_payload(
    language: &str,
    selected_provider: &str,
    candidates: [(&str, bool, &str); 2],
) -> String {
    let candidate_entries = candidates
        .into_iter()
        .map(|(provider, accepted, reason)| {
            json!({
                "provider": provider,
                "accepted": accepted,
                "reason": reason,
            })
        })
        .collect::<Vec<_>>();

    json!({
        "status": "ok",
        "type": "CapabilityResolution",
        "details": {
            "capability": "rename-symbol",
            "language": language,
            "selected_provider": selected_provider,
            "selection_mode": "automatic",
            "outcome": "selected",
            "candidates": candidate_entries
        }
    })
    .to_string()
}

pub fn language_for_extension(file: &std::path::Path) -> Option<&'static str> {
    match file.extension().and_then(|ext| ext.to_str()) {
        Some("py") => Some("python"),
        Some("rs") => Some("rust"),
        _ => None,
    }
}

pub fn automatic_resolution_payload(file: &std::path::Path) -> Option<String> {
    match language_for_extension(file) {
        Some("python") => Some(build_capability_resolution_payload(
            "python",
            Provider::Rope.as_str(),
            [
                ("rope", true, "matched_language_and_capability"),
                ("rust-analyzer", false, "unsupported_language"),
            ],
        )),
        Some("rust") => Some(build_capability_resolution_payload(
            "rust",
            Provider::RustAnalyzer.as_str(),
            [
                ("rust-analyzer", true, "matched_language_and_capability"),
                ("rope", false, "unsupported_language"),
            ],
        )),
        _ => None,
    }
}

pub fn provider_mismatch_payload(file: &std::path::Path, provider: Provider) -> Option<String> {
    let language = language_for_extension(file)?;
    let mismatched = matches!(
        (language, provider),
        ("python", Provider::RustAnalyzer) | ("rust", Provider::Rope)
    );
    if !mismatched {
        return None;
    }

    Some(
        json!({
            "status": "ok",
            "type": "CapabilityResolution",
            "details": {
                "capability": "rename-symbol",
                "language": language,
                "requested_provider": provider.as_str(),
                "selection_mode": "explicit_provider",
                "outcome": "refused",
                "refusal_reason": "explicit_provider_mismatch",
                "candidates": [
                    { "provider": provider.as_str(), "accepted": false, "reason": "explicit_provider_mismatch" }
                ]
            }
        })
        .to_string(),
    )
}

pub fn write_refactor_response(
    writer: &mut TcpStream,
    operation: Operation,
    arguments: &[&str],
    response_payload_for_operation: &dyn Fn(Operation) -> String,
) -> Result<(), std::io::Error> {
    let file = std::path::Path::new(argument_value(arguments, "--file").unwrap_or_default());
    let requested_provider = argument_value(arguments, "--provider").and_then(|p| match p {
        "rope" => Some(Provider::Rope),
        "rust-analyzer" => Some(Provider::RustAnalyzer),
        _ => None,
    });

    if let Some(provider_name) = requested_provider
        && let Some(payload) = provider_mismatch_payload(file, provider_name)
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

    if requested_provider.is_none()
        && let Some(payload) = automatic_resolution_payload(file)
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

    write_stdout_exit(writer, &response_payload_for_operation(operation), 0)
}

pub fn write_stdout_exit(
    writer: &mut TcpStream,
    payload: &str,
    status: i32,
) -> Result<(), std::io::Error> {
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

fn write_json_line(
    writer: &mut impl Write,
    payload: &serde_json::Value,
) -> Result<(), std::io::Error> {
    writer.write_all(payload.to_string().as_bytes())?;
    writer.write_all(b"\n")?;
    writer.flush()
}
