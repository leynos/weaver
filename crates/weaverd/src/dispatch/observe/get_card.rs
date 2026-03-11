//! Handler for the `observe get-card` operation.
//!
//! This module resolves file URIs, loads source text, and delegates
//! Tree-sitter-backed card extraction to `weaver-cards`.

use std::fs;
use std::io::Write;
use std::path::PathBuf;

use url::Url;
use weaver_cards::{
    CardExtractionError, CardExtractionInput, CardRefusal, GetCardRequest, GetCardResponse,
    RefusalReason, TreeSitterCardExtractor,
};

use crate::dispatch::errors::DispatchError;
use crate::dispatch::request::CommandRequest;
use crate::dispatch::response::ResponseWriter;
use crate::dispatch::router::DispatchResult;

/// Handles the `observe get-card` command.
///
/// # Errors
///
/// Returns a [`DispatchError`] if the request arguments are malformed, the URI
/// cannot be resolved to a local file path, the file cannot be read, or
/// extraction fails in a way that is not expressible as a structured refusal.
pub fn handle<W: Write>(
    request: &CommandRequest,
    writer: &mut ResponseWriter<W>,
) -> Result<DispatchResult, DispatchError> {
    let card_request = GetCardRequest::parse(&request.arguments)
        .map_err(|error| DispatchError::invalid_arguments(error.to_string()))?;
    let path = resolve_file_path(&card_request.uri)?;
    let source = fs::read_to_string(&path)?;
    let extractor = TreeSitterCardExtractor::new();

    let response = match extractor.extract(CardExtractionInput {
        path: &path,
        source: &source,
        line: card_request.line,
        column: card_request.column,
        detail: card_request.detail,
    }) {
        Ok(card) => GetCardResponse::Success {
            card: Box::new(card),
        },
        Err(error) => map_extraction_error(error, card_request.detail)?,
    };

    let status = match &response {
        GetCardResponse::Success { .. } => 0,
        GetCardResponse::Refusal { .. } => 1,
        _ => 1,
    };
    let json = serde_json::to_string(&response)?;
    writer.write_stdout(json)?;

    Ok(DispatchResult::with_status(status))
}

fn resolve_file_path(uri: &str) -> Result<PathBuf, DispatchError> {
    let parsed = Url::parse(uri).map_err(|error| {
        DispatchError::invalid_arguments(format!("invalid URI '{uri}': {error}"))
    })?;
    if parsed.scheme() != "file" {
        return Err(DispatchError::invalid_arguments(format!(
            "unsupported URI scheme '{}': expected file",
            parsed.scheme()
        )));
    }

    parsed.to_file_path().map_err(|_| {
        DispatchError::invalid_arguments(format!("URI is not a valid file path: {uri}"))
    })
}

fn map_extraction_error(
    error: CardExtractionError,
    detail: weaver_cards::DetailLevel,
) -> Result<GetCardResponse, DispatchError> {
    match error {
        CardExtractionError::UnsupportedLanguage { path } => Ok(GetCardResponse::Refusal {
            refusal: CardRefusal {
                reason: RefusalReason::UnsupportedLanguage,
                message: format!(
                    "observe get-card: unsupported language for path {}",
                    path.display()
                ),
                requested_detail: detail,
            },
        }),
        CardExtractionError::NoSymbolAtPosition { line, column } => Ok(GetCardResponse::Refusal {
            refusal: CardRefusal {
                reason: RefusalReason::NoSymbolAtPosition,
                message: format!("observe get-card: no symbol found at {line}:{column}"),
                requested_detail: detail,
            },
        }),
        CardExtractionError::PositionOutOfRange { line, column } => Ok(GetCardResponse::Refusal {
            refusal: CardRefusal {
                reason: RefusalReason::NoSymbolAtPosition,
                message: format!("observe get-card: no symbol found at {line}:{column}"),
                requested_detail: detail,
            },
        }),
        CardExtractionError::Parse { language, message } => Err(DispatchError::internal(format!(
            "Tree-sitter parse failed for {language}: {message}"
        ))),
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use tempfile::TempDir;

    use super::*;
    use crate::dispatch::request::CommandRequest;

    fn temp_dir() -> TempDir {
        TempDir::new().expect("temp dir")
    }

    fn write_source(temp_dir: &TempDir, name: &str, source: &str) -> PathBuf {
        let path = temp_dir.path().join(name);
        fs::write(&path, source).expect("write source");
        path
    }

    fn make_request(uri: &str, line: u32, column: u32, detail: &str) -> CommandRequest {
        CommandRequest::parse(
            format!(
                concat!(
                    "{{\"command\":{{\"domain\":\"observe\",\"operation\":\"get-card\"}},",
                    "\"arguments\":[\"--uri\",\"{uri}\",\"--position\",\"{line}:{column}\",",
                    "\"--detail\",\"{detail}\"]}}"
                ),
                uri = uri,
                line = line,
                column = column,
                detail = detail,
            )
            .as_bytes(),
        )
        .expect("request")
    }

    fn response_text(output: Vec<u8>) -> String {
        String::from_utf8(output).expect("utf8")
    }

    fn response_payload(output: Vec<u8>) -> serde_json::Value {
        let response = response_text(output);
        let stream_line = response.lines().next().expect("stream line");
        let envelope: serde_json::Value = serde_json::from_str(stream_line).expect("envelope");
        let data = envelope["data"].as_str().expect("stdout data");
        serde_json::from_str(data).expect("payload")
    }

    fn assert_refusal_response(filename: &str, source: &str, expected_reason: &str) {
        let temp_dir = temp_dir();
        let path = write_source(&temp_dir, filename, source);
        let uri = Url::from_file_path(&path).expect("file uri").to_string();
        let request = make_request(&uri, 1, 1, "structure");
        let mut output = Vec::new();
        let mut writer = ResponseWriter::new(&mut output);

        let result = handle(&request, &mut writer).expect("handler should succeed");

        assert_eq!(result.status, 1);
        let payload = response_payload(output);
        assert_eq!(payload["status"], "refusal");
        assert_eq!(payload["refusal"]["reason"], expected_reason);
    }

    #[test]
    fn handle_returns_success_for_supported_rust_symbol() {
        let temp_dir = temp_dir();
        let path = write_source(
            &temp_dir,
            "card.rs",
            "/// Greets callers.\nfn greet(name: &str) -> usize {\n    let count = name.len();\n    count\n}\n",
        );
        let uri = Url::from_file_path(&path).expect("file uri").to_string();
        let request = make_request(&uri, 2, 4, "structure");
        let mut output = Vec::new();
        let mut writer = ResponseWriter::new(&mut output);

        let result = handle(&request, &mut writer).expect("handler should succeed");

        assert_eq!(result.status, 0);
        let payload = response_payload(output);
        assert_eq!(payload["status"], "success");
        assert_eq!(payload["card"]["symbol"]["ref"]["name"], "greet");
    }

    #[test]
    fn handle_returns_unsupported_language_refusal() {
        assert_refusal_response("notes.txt", "plain text", "unsupported_language");
    }

    #[test]
    fn handle_returns_no_symbol_refusal_for_empty_supported_file() {
        assert_refusal_response("empty.py", "", "no_symbol_at_position");
    }

    #[test]
    fn handle_rejects_non_file_uri() {
        let request = make_request("https://example.com/demo.rs", 1, 1, "minimal");
        let mut output = Vec::new();
        let mut writer = ResponseWriter::new(&mut output);

        let error = match handle(&request, &mut writer) {
            Ok(result) => panic!("handler unexpectedly succeeded: {}", result.status),
            Err(error) => error,
        };

        assert!(matches!(error, DispatchError::InvalidArguments { .. }));
        assert!(error.to_string().contains("unsupported URI scheme"));
    }
}
