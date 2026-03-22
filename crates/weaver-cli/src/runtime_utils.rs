//! Runtime helpers for the CLI entrypoints.

use std::io::{Read, Write};
use std::process::ExitCode;

use weaver_config::{CapabilityMatrix, Config};

use crate::{AppError, Cli, IoStreams};

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

pub(crate) fn exit_code_from_status(status: i32) -> ExitCode {
    if status >= 0 && status <= u8::MAX as i32 {
        ExitCode::from(status as u8)
    } else {
        ExitCode::FAILURE
    }
}

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
