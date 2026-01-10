//! Fake daemon utilities for behavioural tests.
//!
//! Provides a mock TCP server that simulates daemon responses, allowing CLI
//! integration tests to verify request/response behaviour without a real daemon.

use std::io::{self, BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

#[cfg(unix)]
use std::os::unix::net::UnixStream;

use anyhow::{Context, Result, anyhow};

/// A mock daemon server that accepts a single connection and streams canned responses.
pub(in crate::tests) struct FakeDaemon {
    port: u16,
    requests: Arc<Mutex<Vec<String>>>,
    result: Arc<Mutex<Option<Result<()>>>>,
    handle: Option<thread::JoinHandle<()>>,
}

impl FakeDaemon {
    /// Spawns a fake daemon listening on an ephemeral TCP port.
    ///
    /// The daemon accepts one connection, records the request, and streams the
    /// provided lines as the response.
    pub fn spawn(lines: Vec<String>) -> Result<Self> {
        let listener = TcpListener::bind(("127.0.0.1", 0)).context("bind fake daemon")?;
        listener
            .set_nonblocking(true)
            .context("fake daemon nonblocking")?;
        let port = listener.local_addr().context("local addr")?.port();
        let requests: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
        let result: Arc<Mutex<Option<Result<()>>>> = Arc::new(Mutex::new(None));
        let requests_clone = Arc::clone(&requests);
        let result_clone = Arc::clone(&result);
        let handle = thread::spawn(move || {
            let outcome = Self::serve_client(listener, lines, requests_clone);
            if let Ok(mut guard) = result_clone.lock() {
                *guard = Some(outcome);
            }
        });
        Ok(Self {
            port,
            requests,
            result,
            handle: Some(handle),
        })
    }

    pub fn port(&self) -> u16 {
        self.port
    }

    /// Waits for the daemon thread to complete and returns all recorded requests.
    pub fn take_requests(&mut self) -> Result<Vec<String>> {
        if let Some(handle) = self.handle.take() {
            handle
                .join()
                .map_err(|_| anyhow!("fake daemon thread panicked"))?;
        }
        if let Some(outcome) = self
            .result
            .lock()
            .map_err(|error| anyhow!("lock fake daemon result: {error}"))?
            .take()
        {
            outcome.context("fake daemon failed")?;
        }
        let requests = self
            .requests
            .lock()
            .map_err(|error| anyhow!("lock requests: {error}"))?;
        Ok(requests.clone())
    }

    fn serve_client(
        listener: TcpListener,
        lines: Vec<String>,
        requests: Arc<Mutex<Vec<String>>>,
    ) -> Result<()> {
        let deadline = Instant::now() + Duration::from_secs(2);
        loop {
            match listener.accept() {
                Ok((stream, _)) => {
                    Self::record_request(&stream, &requests)?;
                    return Self::stream_responses(stream, &lines);
                }
                Err(ref error)
                    if error.kind() == io::ErrorKind::WouldBlock && Instant::now() < deadline =>
                {
                    thread::sleep(Duration::from_millis(10));
                }
                Err(ref error) if error.kind() == io::ErrorKind::WouldBlock => {
                    // No connection arrived; exit cleanly so tests do not hang when the CLI
                    // aborts before connecting (e.g. capabilities mode exiting early).
                    return Ok(());
                }
                Err(error) => return Err(error).context("accept connection"),
            }
        }
    }

    fn record_request(stream: &TcpStream, requests: &Arc<Mutex<Vec<String>>>) -> Result<()> {
        let mut line = String::new();
        let mut reader = BufReader::new(stream.try_clone().context("clone stream")?);
        if reader
            .read_line(&mut line)
            .context("read command request")?
            == 0
        {
            return Ok(());
        }
        let mut guard = requests
            .lock()
            .map_err(|error| anyhow!("lock requests: {error}"))?;
        guard.push(line);
        Ok(())
    }

    fn stream_responses(mut stream: TcpStream, lines: &[String]) -> Result<()> {
        write_lines(&mut stream, lines).context("write response lines")?;
        Ok(())
    }
}

impl Drop for FakeDaemon {
    fn drop(&mut self) {
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }
}

// ── Stream utilities ───────────────────────────────────────────────────────────

/// Trait for streams that can be cloned for concurrent read/write.
pub(in crate::tests) trait TryCloneStream: Write {
    type Owned: Read + Write + Send + 'static;

    fn try_clone(&self) -> io::Result<Self::Owned>;
}

impl TryCloneStream for TcpStream {
    type Owned = TcpStream;

    fn try_clone(&self) -> io::Result<Self::Owned> {
        TcpStream::try_clone(self)
    }
}

#[cfg(unix)]
impl TryCloneStream for UnixStream {
    type Owned = UnixStream;

    fn try_clone(&self) -> io::Result<Self::Owned> {
        UnixStream::try_clone(self)
    }
}

/// Reads a request line from a stream and writes response lines.
pub(in crate::tests) fn respond_to_request<T>(mut stream: T, lines: &[String]) -> Result<()>
where
    T: TryCloneStream,
{
    let mut buffer = String::new();
    {
        let clone = stream.try_clone().context("clone stream")?;
        let mut reader = BufReader::new(clone);
        let _ = reader.read_line(&mut buffer).context("read request")?;
    }
    write_lines(&mut stream, lines).context("write response lines")
}

/// Writes lines to a stream, appending newlines and flushing.
pub(in crate::tests) fn write_lines(stream: &mut impl Write, lines: &[String]) -> io::Result<()> {
    for line in lines {
        stream.write_all(line.as_bytes())?;
        stream.write_all(b"\n")?;
    }
    stream.flush()
}

/// Accepts a TCP connection and responds with the given lines.
pub(in crate::tests) fn accept_tcp_connection(
    listener: TcpListener,
    lines: Vec<String>,
) -> Result<()> {
    let (stream, _) = listener.accept().context("accept tcp connection")?;
    respond_to_request(stream, &lines)
}

/// Accepts a Unix socket connection and responds with the given lines.
#[cfg(unix)]
pub(in crate::tests) fn accept_unix_connection(
    listener: std::os::unix::net::UnixListener,
    lines: Vec<String>,
) -> Result<()> {
    let (stream, _) = listener.accept().context("accept unix connection")?;
    respond_to_request(stream, &lines)
}
