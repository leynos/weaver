//! Response serialization helpers for the dispatch loop.
//!
//! This module provides the `DaemonMessage` type and `ResponseWriter` helper
//! for streaming JSONL responses back to clients. The message format matches
//! the protocol expected by `weaver-cli`.

use std::io::Write;

use serde::Serialize;

use super::errors::DispatchError;

/// Target stream for output messages.
#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum StreamTarget {
    /// Standard error stream.
    Stderr,
}

/// Response messages sent to clients.
///
/// Each message is serialized as a single JSONL line. The client reads these
/// lines until it receives an `Exit` message, which signals the end of the
/// response stream.
#[derive(Debug, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum DaemonMessage {
    /// Streamed output data directed to stdout or stderr.
    Stream {
        /// Target stream on the client side.
        stream: StreamTarget,
        /// Text payload to write.
        data: String,
    },
    /// Terminal message signalling completion with an exit status.
    Exit {
        /// Exit status code (0 for success, non-zero for failure).
        status: i32,
    },
}

impl DaemonMessage {
    /// Creates a stderr stream message.
    pub fn stderr(data: impl Into<String>) -> Self {
        Self::Stream {
            stream: StreamTarget::Stderr,
            data: data.into(),
        }
    }

    /// Creates an exit message with the given status code.
    pub fn exit(status: i32) -> Self {
        Self::Exit { status }
    }
}

/// Writer that serializes daemon messages to a stream.
///
/// The writer handles JSONL framing (appending newlines) and provides
/// convenience methods for common message patterns.
pub struct ResponseWriter<W> {
    writer: W,
}

impl<W: Write> ResponseWriter<W> {
    /// Creates a new response writer wrapping the given output stream.
    pub fn new(writer: W) -> Self {
        Self { writer }
    }

    /// Writes a daemon message as a JSONL line.
    ///
    /// # Errors
    ///
    /// Returns an error if serialization or writing fails.
    pub fn write_message(&mut self, message: &DaemonMessage) -> Result<(), DispatchError> {
        serde_json::to_writer(&mut self.writer, message)?;
        self.writer.write_all(b"\n")?;
        Ok(())
    }

    /// Writes a stream message to stderr.
    ///
    /// # Errors
    ///
    /// Returns an error if writing fails.
    pub fn write_stderr(&mut self, data: impl Into<String>) -> Result<(), DispatchError> {
        self.write_message(&DaemonMessage::stderr(data))
    }

    /// Writes an exit message and flushes the stream.
    ///
    /// # Errors
    ///
    /// Returns an error if writing or flushing fails.
    pub fn write_exit(&mut self, status: i32) -> Result<(), DispatchError> {
        self.write_message(&DaemonMessage::exit(status))?;
        self.writer.flush()?;
        Ok(())
    }

    /// Writes an error message to stderr followed by an exit message.
    ///
    /// The error's display representation is written to stderr, then an exit
    /// message with the error's status code is sent.
    ///
    /// # Errors
    ///
    /// Returns an error if writing fails.
    pub fn write_error(&mut self, error: &DispatchError) -> Result<(), DispatchError> {
        self.write_stderr(format!("error: {error}\n"))?;
        self.write_exit(error.exit_status())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn writes_exit_message() {
        let mut output = Vec::new();
        let mut writer = ResponseWriter::new(&mut output);
        writer.write_exit(0).expect("write exit");

        let response = String::from_utf8(output).expect("valid utf8");
        assert!(response.contains(r#""kind":"exit""#));
        assert!(response.contains(r#""status":0"#));
        assert!(response.ends_with('\n'));
    }

    #[test]
    fn writes_stderr_stream() {
        let mut output = Vec::new();
        let mut writer = ResponseWriter::new(&mut output);
        writer.write_stderr("error text").expect("write stderr");

        let response = String::from_utf8(output).expect("valid utf8");
        assert!(response.contains(r#""stream":"stderr""#));
        assert!(response.contains(r#""data":"error text""#));
    }

    #[test]
    fn write_error_includes_status() {
        let mut output = Vec::new();
        let mut writer = ResponseWriter::new(&mut output);
        let error = DispatchError::unknown_domain("bogus");
        writer.write_error(&error).expect("write error");

        let response = String::from_utf8(output).expect("valid utf8");
        assert!(response.contains("unknown domain"));
        assert!(response.contains(r#""status":1"#));
    }
}
