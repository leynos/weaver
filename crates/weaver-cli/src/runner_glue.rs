//! Runtime glue for daemon command execution.
//!
//! The CLI runner keeps argument parsing and configuration orchestration in
//! `lib.rs`; this module owns the daemon transport path and request building so
//! the top-level runtime stays small enough to scan.

use std::{
    io::{Read, Write},
    process::ExitCode,
};

use crate::{
    AppError,
    CommandInvocation,
    CommandRequest,
    IoStreams,
    OutputContext,
    ResolvedOutputFormat,
    actionable_guidance,
    daemon_output::{OutputSettings, read_daemon_messages},
    errors::is_daemon_not_running,
    exit_code_from_status,
    lifecycle::{LifecycleContext, try_auto_start_daemon},
    transport::{self, Connection, connect, connect_with_retry},
};

pub(crate) fn execute_daemon_command<R, W, E>(
    invocation: CommandInvocation,
    context: LifecycleContext<'_>,
    io: &mut IoStreams<'_, R, W, E>,
    output_format: ResolvedOutputFormat,
) -> ExitCode
where
    R: Read,
    W: Write,
    E: Write,
{
    let output_context = OutputContext::new(
        invocation.domain.clone(),
        invocation.operation.clone(),
        invocation.arguments.clone(),
    );
    let request = match build_request(invocation, &mut *io.stdin) {
        Ok(request) => request,
        Err(error) => return write_error_and_fail(&mut *io.stderr, error),
    };
    let mut connection = match connect_or_start_daemon(context, &mut *io.stderr) {
        Ok(connection) => connection,
        Err(exit_code) => return exit_code,
    };

    if let Err(error) = request.write_jsonl(&mut connection) {
        return write_error_and_fail(&mut *io.stderr, error);
    }

    match read_daemon_messages(
        &mut connection,
        io,
        OutputSettings {
            format: output_format,
            context: &output_context,
        },
    ) {
        Ok(status) => exit_code_from_status(status),
        Err(error) => write_error_and_fail(&mut *io.stderr, error),
    }
}

fn connect_or_start_daemon<E: Write>(
    context: LifecycleContext<'_>,
    stderr: &mut E,
) -> Result<Connection, ExitCode> {
    match connect(context.config.daemon_socket()) {
        Ok(connection) => Ok(connection),
        Err(error) if is_daemon_not_running(&error) => start_and_retry_daemon(context, stderr),
        Err(error) => Err(write_error_and_fail(stderr, error)),
    }
}

pub(crate) fn build_request<R: Read>(
    invocation: CommandInvocation,
    stdin: &mut R,
) -> Result<CommandRequest, AppError> {
    if invocation.is_apply_patch() {
        let mut patch = String::new();
        stdin
            .read_to_string(&mut patch)
            .map_err(AppError::ReadPatch)?;
        if patch.trim().is_empty() {
            return Err(AppError::MissingPatchInput);
        }
        Ok(CommandRequest::with_patch(invocation, patch))
    } else {
        Ok(CommandRequest::from(invocation))
    }
}

fn write_error_and_fail<W: Write>(stderr: &mut W, error: impl std::fmt::Display) -> ExitCode {
    writeln!(stderr, "{error}").ok();
    ExitCode::FAILURE
}

fn start_and_retry_daemon<E: Write>(
    context: LifecycleContext<'_>,
    stderr: &mut E,
) -> Result<Connection, ExitCode> {
    if let Err(error) = try_auto_start_daemon(context, stderr) {
        actionable_guidance::write_startup_guidance(stderr, &error).ok();
        return Err(ExitCode::FAILURE);
    }

    // Retry briefly after daemon startup to tolerate socket-bind lag.
    connect_with_retry(
        context.config.daemon_socket(),
        transport::CONNECTION_TIMEOUT,
    )
    .map_err(|error| write_error_and_fail(stderr, error))
}
