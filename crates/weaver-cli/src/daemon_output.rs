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
pub(crate) struct OutputSettings<'a> {
    pub(crate) format: ResolvedOutputFormat,
    pub(crate) context: &'a OutputContext,
}

/// Processes a single daemon message, writing output to the appropriate stream.
fn process_message<W, E, S>(
    message: DaemonMessage,
    io: &mut IoStreams<'_, S, W, E>,
    settings: &OutputSettings<'_>,
) -> Result<(), AppError>
where
    S: Read,
    W: Write,
    E: Write,
{
    match message {
        DaemonMessage::Stream { stream, data } => {
            let rendered = match settings.format {
                ResolvedOutputFormat::Human => render_human_output(settings.context, &data),
                ResolvedOutputFormat::Json => None,
            };
            let payload = rendered.as_deref().unwrap_or(&data);
            match stream {
                StreamTarget::Stdout => io.stdout.write_all(payload.as_bytes()),
                StreamTarget::Stderr => io.stderr.write_all(payload.as_bytes()),
            }
            .map_err(AppError::ForwardResponse)?;
        }
        DaemonMessage::Exit { .. } => {}
    }
    Ok(())
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
            "Warning: received {EMPTY_LINE_LIMIT} consecutive empty lines from daemon; \
             aborting."
        )
        .map_err(AppError::ForwardResponse)?;
        return Ok(true);
    }
    Ok(false)
}

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
        process_message(message, io, &settings)?;
        line.clear();
    }

    io.stdout.flush().map_err(AppError::ForwardResponse)?;
    io.stderr.flush().map_err(AppError::ForwardResponse)?;

    exit_status.ok_or(AppError::MissingExit)
}

#[derive(Debug, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum DaemonMessage {
    Stream { stream: StreamTarget, data: String },
    Exit { status: i32 },
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
enum StreamTarget {
    Stdout,
    Stderr,
}
