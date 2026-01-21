//! Stdio transport layer with LSP header framing.
//!
//! LSP uses a simple framing protocol over stdio:
//! ```text
//! Content-Length: <length>\r\n
//! \r\n
//! <payload>
//! ```

use std::io::{BufRead, BufReader, BufWriter, Read, Write};
use std::process::{ChildStdin, ChildStdout};

use super::error::TransportError;

/// Writes an LSP-framed message to any writer.
fn write_framed<W: Write>(mut writer: W, message: &[u8]) -> Result<(), TransportError> {
    let header = format!("Content-Length: {}\r\n\r\n", message.len());
    writer.write_all(header.as_bytes())?;
    writer.write_all(message)?;
    writer.flush()?;
    Ok(())
}

/// Reads headers and extracts the Content-Length value from any reader.
fn read_content_length<R: BufRead>(reader: &mut R) -> Result<usize, TransportError> {
    let mut content_length: Option<usize> = None;

    loop {
        let mut line = String::new();
        let bytes_read = reader.read_line(&mut line)?;
        if bytes_read == 0 {
            return Err(TransportError::Io(std::io::Error::new(
                std::io::ErrorKind::UnexpectedEof,
                "connection closed while reading headers",
            )));
        }

        let trimmed = line.trim();
        if trimmed.is_empty() {
            break;
        }

        if let Some(value) = trimmed.strip_prefix("Content-Length: ") {
            content_length = Some(value.parse().map_err(|_| TransportError::InvalidHeader)?);
        }
    }

    content_length.ok_or(TransportError::MissingContentLength)
}

/// Reads and writes LSP-framed messages over process stdio.
///
/// The transport handles the LSP header framing protocol, which prefixes
/// each message with a `Content-Length` header.
pub struct StdioTransport {
    reader: BufReader<ChildStdout>,
    writer: BufWriter<ChildStdin>,
}

impl StdioTransport {
    /// Creates a new transport from process handles.
    #[must_use]
    pub fn new(stdout: ChildStdout, stdin: ChildStdin) -> Self {
        Self {
            reader: BufReader::new(stdout),
            writer: BufWriter::new(stdin),
        }
    }

    /// Sends an LSP-framed message.
    ///
    /// # Errors
    ///
    /// Returns `TransportError::Io` if writing to the process fails.
    pub fn send(&mut self, message: &[u8]) -> Result<(), TransportError> {
        write_framed(&mut self.writer, message)
    }

    /// Receives an LSP-framed message (blocks until complete).
    ///
    /// # Errors
    ///
    /// Returns `TransportError::MissingContentLength` if no Content-Length header is found.
    /// Returns `TransportError::Io` if reading from the process fails.
    pub fn receive(&mut self) -> Result<Vec<u8>, TransportError> {
        let content_length = read_content_length(&mut self.reader)?;
        let mut content = vec![0u8; content_length];
        self.reader.read_exact(&mut content)?;
        Ok(content)
    }
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use rstest::rstest;

    use super::*;

    /// A mock transport for testing that uses in-memory buffers.
    struct MockTransport {
        read_buffer: Cursor<Vec<u8>>,
        write_buffer: Vec<u8>,
    }

    impl MockTransport {
        fn new() -> Self {
            Self {
                read_buffer: Cursor::new(Vec::new()),
                write_buffer: Vec::new(),
            }
        }

        fn with_input(input: &[u8]) -> Self {
            Self {
                read_buffer: Cursor::new(input.to_vec()),
                write_buffer: Vec::new(),
            }
        }

        fn send(&mut self, message: &[u8]) -> Result<(), TransportError> {
            write_framed(&mut self.write_buffer, message)
        }

        fn receive(&mut self) -> Result<Vec<u8>, TransportError> {
            let content_length = read_content_length(&mut self.read_buffer)?;
            let mut content = vec![0u8; content_length];
            self.read_buffer.read_exact(&mut content)?;
            Ok(content)
        }

        fn written_bytes(&self) -> &[u8] {
            &self.write_buffer
        }
    }

    #[rstest]
    fn sends_lsp_framed_message() {
        let mut transport = MockTransport::new();
        let message = b"test payload";

        transport.send(message).expect("send failed");

        let written = String::from_utf8(transport.written_bytes().to_vec()).expect("invalid utf8");
        assert!(written.starts_with("Content-Length: 12\r\n\r\n"));
        assert!(written.ends_with("test payload"));
    }

    #[rstest]
    fn sends_empty_message() {
        let mut transport = MockTransport::new();

        transport.send(b"").expect("send failed");

        let written = String::from_utf8(transport.written_bytes().to_vec()).expect("invalid utf8");
        assert_eq!(written, "Content-Length: 0\r\n\r\n");
    }

    #[rstest]
    fn receives_lsp_framed_message() {
        let input = b"Content-Length: 5\r\n\r\nhello";
        let mut transport = MockTransport::with_input(input);

        let received = transport.receive().expect("receive failed");

        assert_eq!(received, b"hello");
    }

    #[rstest]
    fn receives_message_with_multiple_headers() {
        let input = b"Content-Length: 4\r\nContent-Type: application/json\r\n\r\ntest";
        let mut transport = MockTransport::with_input(input);

        let received = transport.receive().expect("receive failed");

        assert_eq!(received, b"test");
    }

    #[rstest]
    fn handles_missing_content_length() {
        let input = b"Content-Type: application/json\r\n\r\ntest";
        let mut transport = MockTransport::with_input(input);

        let result = transport.receive();

        assert!(matches!(result, Err(TransportError::MissingContentLength)));
    }

    #[rstest]
    fn handles_invalid_content_length() {
        let input = b"Content-Length: invalid\r\n\r\ntest";
        let mut transport = MockTransport::with_input(input);

        let result = transport.receive();

        assert!(matches!(result, Err(TransportError::InvalidHeader)));
    }

    #[rstest]
    fn handles_eof_during_headers() {
        let input = b"Content-Length: 10";
        let mut transport = MockTransport::with_input(input);

        let result = transport.receive();

        assert!(matches!(result, Err(TransportError::Io(_))));
    }

    #[rstest]
    fn round_trips_json_message() {
        let json = r#"{"jsonrpc":"2.0","id":1,"method":"test"}"#;
        let mut transport = MockTransport::new();

        transport.send(json.as_bytes()).expect("send failed");

        // Create a new transport with the written data as input
        let mut receiving = MockTransport::with_input(transport.written_bytes());
        let received = receiving.receive().expect("receive failed");

        assert_eq!(received, json.as_bytes());
    }
}
