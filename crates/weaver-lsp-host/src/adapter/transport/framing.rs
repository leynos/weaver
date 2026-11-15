//! LSP header framing for the stdio transport.
//!
//! LSP frames a message as `Content-Length: <n>\r\n\r\n` followed by exactly
//! `n` bytes. The helpers here read and write only that envelope, so the
//! transport can concentrate on its threads, deadlines and channels.

use std::io::{BufRead, Write};

use super::TransportError;

/// Writes an LSP-framed message to any writer.
pub(super) fn write_framed<W: Write>(mut writer: W, message: &[u8]) -> Result<(), TransportError> {
    let header = format!("Content-Length: {}\r\n\r\n", message.len());
    writer.write_all(header.as_bytes())?;
    writer.write_all(message)?;
    writer.flush()?;
    Ok(())
}

/// Reads headers and extracts the Content-Length value from any reader.
pub(super) fn read_content_length<R: BufRead>(reader: &mut R) -> Result<usize, TransportError> {
    let mut content_length: Option<usize> = None;

    while let Some(header) = read_header_line(reader)? {
        // A later duplicate header wins, matching the last-value-wins reading
        // callers expect from LSP header blocks.
        content_length = parse_content_length(&header)?.or(content_length);
    }

    content_length.ok_or(TransportError::MissingContentLength)
}

/// Reads one header line, ending the block at its terminating blank line.
///
/// Returns `None` for the blank line that closes the header block, so the
/// caller can stop without treating it as a header of its own.
fn read_header_line<R: BufRead>(reader: &mut R) -> Result<Option<String>, TransportError> {
    let mut line = String::new();
    let bytes_read = reader.read_line(&mut line)?;
    if bytes_read == 0 {
        return Err(TransportError::Io(std::io::Error::new(
            std::io::ErrorKind::UnexpectedEof,
            "connection closed while reading headers",
        )));
    }

    let trimmed = line.trim();
    Ok((!trimmed.is_empty()).then(|| trimmed.to_owned()))
}

/// Extracts a `Content-Length` value, ignoring headers with other names.
fn parse_content_length(header: &str) -> Result<Option<usize>, TransportError> {
    let Some(value) = header.strip_prefix("Content-Length: ") else {
        return Ok(None);
    };

    value
        .parse()
        .map(Some)
        .map_err(|_| TransportError::InvalidHeader)
}
