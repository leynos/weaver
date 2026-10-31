//! Stdio transport layer with LSP header framing.
//!
//! LSP uses a simple framing protocol over stdio:
//! ```text
//! Content-Length: <length>\r\n
//! \r\n
//! <payload>
//! ```

use std::{
    io::{self, BufRead, BufReader, BufWriter, Write},
    process::{ChildStdin, ChildStdout},
    sync::{
        Arc,
        mpsc::{self, Receiver, RecvTimeoutError, Sender},
    },
    thread,
    time::{Duration, Instant},
};

use super::{
    clock::{Clock, Deadline, SystemClock},
    error::TransportError,
};

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

/// How long to wait for a language server to answer before giving up on it.
///
/// A language server that stops responding (wedged on a lock, starved of CPU,
/// or stuck loading a workspace) would otherwise block the daemon's dispatch
/// thread for ever, because a pipe read has no deadline of its own. The value
/// sits well above a healthy server's worst start-up on a loaded host and well
/// below the multi-minute kill limits that test runners apply.
pub const DEFAULT_RESPONSE_TIMEOUT: Duration = Duration::from_secs(30);

/// Reads and writes LSP-framed messages over process stdio.
///
/// The transport handles the LSP header framing protocol, which prefixes
/// each message with a `Content-Length` header. Reads happen on a dedicated
/// thread that forwards whole frames over a channel, so callers can wait for a
/// frame with a deadline ([`Self::receive_before`]) instead of blocking on the
/// pipe. The thread ends when the server's stdout closes, which happens when
/// the process exits or is killed.
pub struct StdioTransport {
    frames: Receiver<Result<Vec<u8>, TransportError>>,
    writer: BufWriter<ChildStdin>,
    response_timeout: Duration,
    clock: Arc<dyn Clock>,
}

impl StdioTransport {
    /// Creates a new transport from process handles, using
    /// [`DEFAULT_RESPONSE_TIMEOUT`].
    #[must_use]
    pub fn new(stdout: ChildStdout, stdin: ChildStdin) -> Self {
        let (sender, frames) = mpsc::channel();
        thread::spawn(move || forward_frames(BufReader::new(stdout), &sender));
        Self {
            frames,
            writer: BufWriter::new(stdin),
            response_timeout: DEFAULT_RESPONSE_TIMEOUT,
            clock: Arc::new(SystemClock),
        }
    }

    /// Replaces the clock that decides how much of a deadline is left.
    #[must_use]
    pub fn with_clock(mut self, clock: Arc<dyn Clock>) -> Self {
        self.clock = clock;
        self
    }

    /// Returns the current instant according to the transport's clock.
    #[must_use]
    pub fn now(&self) -> Instant { self.clock.now() }

    /// Sets how long callers wait for the response to one request.
    #[must_use]
    pub const fn with_response_timeout(mut self, response_timeout: Duration) -> Self {
        self.response_timeout = response_timeout;
        self
    }

    /// Returns how long a caller should wait for the response to one request.
    #[must_use]
    pub const fn response_timeout(&self) -> Duration { self.response_timeout }

    /// Sends an LSP-framed message.
    ///
    /// # Errors
    ///
    /// Returns `TransportError::Io` if writing to the process fails.
    pub fn send(&mut self, message: &[u8]) -> Result<(), TransportError> {
        write_framed(&mut self.writer, message)
    }

    /// Receives the next LSP-framed message, waiting until `deadline` at most.
    ///
    /// # Errors
    ///
    /// Returns `TransportError::TimedOut` if no complete frame arrives before
    /// `deadline`. Returns `TransportError::MissingContentLength`,
    /// `TransportError::InvalidHeader` or `TransportError::Io` if the server
    /// sends a malformed frame or closes its stdout.
    pub fn receive_before(&mut self, deadline: Deadline) -> Result<Vec<u8>, TransportError> {
        let remaining = deadline.remaining(self.clock.now());
        match self.frames.recv_timeout(remaining) {
            Ok(frame) => frame,
            Err(RecvTimeoutError::Timeout) => Err(TransportError::TimedOut),
            Err(RecvTimeoutError::Disconnected) => Err(TransportError::Io(io::Error::new(
                io::ErrorKind::UnexpectedEof,
                "language server closed its output",
            ))),
        }
    }
}

/// Reads frames from `reader` and sends them until the stream ends, a frame is
/// malformed, or the receiving side is dropped.
fn forward_frames<R: BufRead>(mut reader: R, sender: &Sender<Result<Vec<u8>, TransportError>>) {
    loop {
        let frame = read_frame(&mut reader);
        let is_error = frame.is_err();
        if sender.send(frame).is_err() || is_error {
            return;
        }
    }
}

/// Reads one complete LSP frame (headers, then exactly `Content-Length` bytes).
fn read_frame<R: BufRead>(reader: &mut R) -> Result<Vec<u8>, TransportError> {
    let content_length = read_content_length(reader)?;
    let mut content = vec![0u8; content_length];
    reader.read_exact(&mut content)?;
    Ok(content)
}

#[cfg(test)]
mod tests {
    //! Unit tests for LSP transport layer and message framing.

    use std::io::{Cursor, Read};

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

        fn written_bytes(&self) -> &[u8] { &self.write_buffer }
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

    #[cfg(unix)]
    mod stdio {
        //! Tests that exercise the real pipe-backed transport against `cat`.

        use std::process::{Child, Command, Stdio};

        use super::*;
        use crate::adapter::ManualClock;

        /// Returns a deadline `timeout` from now on the real clock.
        fn deadline_in(timeout: Duration) -> Deadline {
            Deadline::after(Instant::now(), timeout).expect("test timeouts fit the clock")
        }

        /// Spawns `command` with piped stdio and wraps its pipes in a transport.
        fn spawn_transport(command: &str, args: &[&str]) -> (Child, StdioTransport) {
            let mut child = Command::new(command)
                .args(args)
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .spawn()
                .expect("spawn helper process");
            let stdin = child.stdin.take().expect("piped stdin");
            let stdout = child.stdout.take().expect("piped stdout");
            (child, StdioTransport::new(stdout, stdin))
        }

        #[rstest]
        fn receive_before_returns_a_frame_the_peer_sent() {
            // `cat` echoes our framed bytes back, standing in for a server reply.
            let (mut child, mut transport) = spawn_transport("cat", &[]);
            transport.send(b"hello").expect("send failed");

            let received = transport
                .receive_before(deadline_in(Duration::from_secs(20)))
                .expect("echoed frame should arrive");

            assert_eq!(received, b"hello");
            child.kill().expect("kill helper");
            child.wait().expect("reap helper");
        }

        #[rstest]
        fn receive_before_times_out_on_a_silent_peer() {
            let (mut child, mut transport) = spawn_transport("sh", &["-c", "cat > /dev/null"]);
            let started = Instant::now();

            let result = transport.receive_before(deadline_in(Duration::from_millis(100)));

            assert!(matches!(result, Err(TransportError::TimedOut)));
            assert!(started.elapsed() < Duration::from_secs(20));
            child.kill().expect("kill helper");
            child.wait().expect("reap helper");
        }

        #[rstest]
        fn receive_before_expires_when_the_injected_clock_passes_the_deadline() {
            // An hour-long deadline on a clock that then jumps two hours: the
            // transport must answer from the clock, not wait the hour out.
            let clock = Arc::new(ManualClock::new());
            let (mut child, transport) = spawn_transport("sh", &["-c", "cat > /dev/null"]);
            let mut transport = transport.with_clock(Arc::clone(&clock) as Arc<dyn Clock>);
            let deadline = Deadline::after(transport.now(), Duration::from_secs(3600))
                .expect("an hour fits the clock");
            clock.advance(Duration::from_secs(7200));
            let started = Instant::now();

            let result = transport.receive_before(deadline);

            assert!(matches!(result, Err(TransportError::TimedOut)));
            assert!(started.elapsed() < Duration::from_secs(5));
            child.kill().expect("kill helper");
            child.wait().expect("reap helper");
        }

        #[rstest]
        fn receive_before_reports_a_closed_peer_as_eof() {
            let (mut child, mut transport) = spawn_transport("true", &[]);
            child.wait().expect("reap helper");

            let result = transport.receive_before(deadline_in(Duration::from_secs(20)));

            assert!(matches!(result, Err(TransportError::Io(_))));
        }
    }
}
