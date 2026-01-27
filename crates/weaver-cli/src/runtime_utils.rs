//! Runtime helpers for the CLI entrypoints.

use std::io::Write;
use std::process::ExitCode;

use weaver_config::{CapabilityMatrix, Config};

use crate::AppError;

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
    if (0..=255).contains(&status) {
        ExitCode::from(status as u8)
    } else {
        ExitCode::FAILURE
    }
}
