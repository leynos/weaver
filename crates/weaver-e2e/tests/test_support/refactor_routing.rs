//! Shared helpers for refactor-routing end-to-end snapshots.

use std::io::Write;
use std::net::TcpStream;

use serde_json::json;

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
            "status": "ok",
            "type": "CapabilityResolution",
            "details": {
                "capability": "rename-symbol",
                "language": language,
                "requested_provider": provider,
                "selection_mode": "explicit_provider",
                "outcome": "refused",
                "refusal_reason": "explicit_provider_mismatch",
                "candidates": [
                    { "provider": provider, "accepted": false, "reason": "explicit_provider_mismatch" }
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
    response_payload_for_operation: &dyn Fn(&str) -> String,
) -> Result<(), std::io::Error> {
    let file = argument_value(arguments, "--file").unwrap_or_default();
    let requested_provider = argument_value(arguments, "--provider");

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
