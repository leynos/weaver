//! Framing and decoding of inbound LSP responses for the E2E client.

use std::io::BufRead;

use super::LspClientError;
use crate::jsonrpc::Response;

/// Reads one framed LSP response per its `Content-Length` header.
pub(super) fn read_response(reader: &mut impl BufRead) -> Result<Response, LspClientError> {
    let mut content_length: Option<usize> = None;
    loop {
        let mut line_buf = String::new();
        reader
            .read_line(&mut line_buf)
            .map_err(LspClientError::Io)?;

        let trimmed = line_buf.trim();
        if trimmed.is_empty() {
            break;
        }
        if let Some(len_str) = trimmed.strip_prefix("Content-Length: ") {
            content_length = len_str.parse().ok();
        }
    }

    let len = content_length.ok_or_else(|| {
        LspClientError::Io(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "missing Content-Length header",
        ))
    })?;
    let mut buffer = vec![0_u8; len];
    reader.read_exact(&mut buffer).map_err(LspClientError::Io)?;

    let content = String::from_utf8(buffer).map_err(|error| {
        LspClientError::Io(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            format!("invalid UTF-8 in response: {error}"),
        ))
    })?;
    serde_json::from_str(&content).map_err(LspClientError::Json)
}
