//! Runtime helpers for the CLI entrypoints.

use std::{
    io::{Read, Write},
    process::ExitCode,
};

use weaver_config::{CapabilityMatrix, Config};

use crate::{AppError, Cli, IoStreams};

/// Writes the resolved capability matrix to `stdout` as pretty-printed JSON.
///
/// # Errors
///
/// Returns [`AppError::SerialiseCapabilities`] when serialisation fails and
/// [`AppError::EmitCapabilities`] when writing or flushing the stream fails.
pub(crate) fn emit_capabilities<W>(config: &Config, stdout: &mut W) -> Result<(), AppError>
where
    W: Write,
{
    let matrix: CapabilityMatrix = config.capability_matrix();
    serde_json::to_writer_pretty(&mut *stdout, &matrix).map_err(AppError::SerialiseCapabilities)?;
    stdout
        .write_all(b"\n")
        .map_err(AppError::EmitCapabilities)?;
    stdout.flush().map_err(AppError::EmitCapabilities)
}

/// Converts a daemon exit status into an [`ExitCode`].
///
/// Statuses outside the range representable by `u8` map to
/// [`ExitCode::FAILURE`].
pub(crate) fn exit_code_from_status(status: i32) -> ExitCode {
    u8::try_from(status).map_or(ExitCode::FAILURE, ExitCode::from)
}

/// Emits capabilities when `--capabilities` was requested and reports the exit.
///
/// Returns `None` when capability emission was not requested, leaving the
/// caller to continue with normal command handling.
pub(crate) fn handle_capabilities_mode<R, W, E>(
    cli: &Cli,
    config: &Config,
    io: &mut IoStreams<'_, R, W, E>,
) -> Option<ExitCode>
where
    R: Read,
    W: Write,
    E: Write,
{
    if !cli.capabilities {
        return None;
    }

    match emit_capabilities(config, io.stdout) {
        Ok(()) => Some(ExitCode::SUCCESS),
        Err(error) => {
            writeln!(io.stderr, "{error}").ok();
            Some(ExitCode::FAILURE)
        }
    }
}
