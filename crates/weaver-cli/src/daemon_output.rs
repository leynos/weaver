//! Daemon response handling and output rendering.
//!
//! Owns parsing daemon messages and forwarding rendered output to the CLI
//! streams.

use std::io::{self, Read, Write};

use serde::Deserialize;

use crate::{
    AppError,
    EMPTY_LINE_LIMIT,
    IoStreams,
    OutputContext,
    ResolvedOutputFormat,
    render_human_output,
};

/// Settings for rendering daemon output.
///
/// Both fields are cheap to copy, so handlers receive the settings by value
/// without disturbing the borrow of the command context.
#[derive(Clone, Copy)]
pub(crate) struct OutputSettings<'a> {
    /// Output format used to decide whether payloads are rendered for humans.
    pub(crate) format: ResolvedOutputFormat,
    /// Command context consulted when rendering human-readable output.
    pub(crate) context: &'a OutputContext,
}

/// Processes a single daemon message, writing output to the appropriate stream.
fn process_message<W, E, S>(
    message: DaemonMessage,
    io: &mut IoStreams<'_, S, W, E>,
    settings: OutputSettings<'_>,
) -> Result<(), AppError>
where
    S: Read,
    W: Write,
    E: Write,
{
    let DaemonMessage::Stream { stream, data } = message else {
        return Ok(());
    };
    let rendered = render_stream_payload(settings, &data);
    forward_stream_payload(&stream, rendered.as_deref().unwrap_or(&data), io)?;
    Ok(())
}

/// Renders `data` as human-readable output when the format requests it.
///
/// Returns `None` for JSON output, which forwards the payload unchanged.
fn render_stream_payload(settings: OutputSettings<'_>, data: &str) -> Option<String> {
    match settings.format {
        ResolvedOutputFormat::Human => render_human_output(settings.context, data),
        ResolvedOutputFormat::Json => None,
    }
}

/// Writes `payload` to the stream selected by `stream`.
///
/// # Errors
///
/// Returns [`AppError::ForwardResponse`] when writing to the target stream
/// fails.
fn forward_stream_payload<W, E, S>(
    stream: &StreamTarget,
    payload: &str,
    io: &mut IoStreams<'_, S, W, E>,
) -> Result<(), AppError>
where
    S: Read,
    W: Write,
    E: Write,
{
    match stream {
        StreamTarget::Stdout => io.stdout.write_all(payload.as_bytes()),
        StreamTarget::Stderr => io.stderr.write_all(payload.as_bytes()),
    }
    .map_err(AppError::ForwardResponse)
}

/// Checks if the empty line limit has been reached and writes a warning if so.
fn check_empty_line_limit<W, E, S>(
    consecutive_empty_lines: usize,
    io: &mut IoStreams<'_, S, W, E>,
) -> Result<bool, AppError>
where
    S: Read,
    W: Write,
    E: Write,
{
    if consecutive_empty_lines >= EMPTY_LINE_LIMIT {
        writeln!(
            io.stderr,
            "Warning: received {EMPTY_LINE_LIMIT} consecutive empty lines from daemon; aborting."
        )
        .map_err(AppError::ForwardResponse)?;
        return Ok(true);
    }
    Ok(false)
}

/// Reads JSON Lines daemon messages until the exit message arrives.
///
/// Payloads are rendered or forwarded as they arrive, and the daemon's exit
/// status is returned once the stream ends.
///
/// # Errors
///
/// Returns [`AppError::ReadResponse`] when reading from the connection fails,
/// [`AppError::ParseMessage`] when a line is not a valid message,
/// [`AppError::ForwardResponse`] when writing or flushing output fails, and
/// [`AppError::MissingExit`] when the stream ends without an exit message.
pub(crate) fn read_daemon_messages<R, W, E, S>(
    connection: &mut R,
    io: &mut IoStreams<'_, S, W, E>,
    settings: OutputSettings<'_>,
) -> Result<i32, AppError>
where
    R: io::Read,
    S: Read,
    W: Write,
    E: Write,
{
    use std::io::BufRead;

    let mut reader = io::BufReader::new(connection);
    let mut line = String::new();
    let mut exit_status: Option<i32> = None;
    let mut consecutive_empty_lines = 0;

    while reader
        .read_line(&mut line)
        .map_err(AppError::ReadResponse)?
        != 0
    {
        if line.trim().is_empty() {
            consecutive_empty_lines += 1;
            if check_empty_line_limit(consecutive_empty_lines, io)? {
                break;
            }
            line.clear();
            continue;
        }
        consecutive_empty_lines = 0;
        let message: DaemonMessage = serde_json::from_str(&line).map_err(AppError::ParseMessage)?;
        if let DaemonMessage::Exit { status } = &message {
            exit_status = Some(*status);
        }
        process_message(message, io, settings)?;
        line.clear();
    }

    io.stdout.flush().map_err(AppError::ForwardResponse)?;
    io.stderr.flush().map_err(AppError::ForwardResponse)?;

    exit_status.ok_or(AppError::MissingExit)
}

/// One JSON Lines message emitted by the daemon.
#[derive(Debug, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum DaemonMessage {
    /// A chunk of output that belongs on a stream.
    Stream {
        /// Destination stream for `data`.
        stream: StreamTarget,
        /// Payload text forwarded or rendered before writing.
        data: String,
    },
    /// The daemon's final exit status.
    Exit {
        /// Process exit status reported by the daemon.
        status: i32,
    },
}

/// Destination stream named by a daemon stream message.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
enum StreamTarget {
    /// Standard output of the CLI process.
    Stdout,
    /// Standard error of the CLI process.
    Stderr,
}
