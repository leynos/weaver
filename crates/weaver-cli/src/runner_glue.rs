//! Runtime glue for daemon command execution.
//!
//! The CLI runner keeps argument parsing and configuration orchestration in
//! `lib.rs`; this module owns the daemon transport path and request building so
//! the top-level runtime stays small enough to scan.

use std::{
    io::{Error, ErrorKind, Read, Write},
    process::ExitCode,
};

use weaver_daemon_types::JSONL_REQUEST_MAX_LINE_BYTES;

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

/// Maximum patch size accepted from stdin.
///
/// Mirrors the JSON Lines request line-size budget so apply-patch requests do
/// not exceed the daemon transport limit. Patches that exceed this limit return
/// an IO error with `ErrorKind::UnexpectedEof`.
const MAX_PATCH_BYTES: u64 = JSONL_REQUEST_MAX_LINE_BYTES as u64;

/// Executes a daemon-backed command end-to-end.
///
/// Builds a [`CommandRequest`] from `invocation`, connects to the daemon socket
/// (auto-starting the daemon if it is not running), writes the request as JSON
/// Lines over the connection, and consumes daemon response messages,
/// translating the final status into an [`ExitCode`].
///
/// Writes a human-readable error message to `io.stderr` and returns
/// [`ExitCode::FAILURE`] on any transport or IO error.
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
    tracing::debug!(
        domain = %invocation.domain,
        operation = %invocation.operation,
        "executing daemon command"
    );
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
    tracing::debug!("connected to daemon socket");

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
        Err(error) if is_daemon_not_running(&error) => {
            tracing::debug!("daemon not running; attempting auto-start");
            start_and_retry_daemon(context, stderr)
        }
        Err(error) => Err(write_error_and_fail(stderr, error)),
    }
}

/// Builds a [`CommandRequest`] from `invocation`.
///
/// For `apply-patch` operations, reads patch content from `stdin` and returns
/// [`AppError::MissingPatchInput`] if the content is empty after trimming. For
/// all other operations, constructs the request directly from the invocation
/// without reading stdin.
pub(crate) fn build_request<R: Read>(
    invocation: CommandInvocation,
    stdin: &mut R,
) -> Result<CommandRequest, AppError> {
    if invocation.is_apply_patch() {
        let mut patch = String::new();
        stdin
            .take(MAX_PATCH_BYTES + 1)
            .read_to_string(&mut patch)
            .map_err(AppError::ReadPatch)?;
        if patch.len() as u64 > MAX_PATCH_BYTES {
            return Err(AppError::ReadPatch(Error::new(
                ErrorKind::UnexpectedEof,
                "patch input exceeds maximum size",
            )));
        }
        if patch.trim().is_empty() {
            return Err(AppError::MissingPatchInput);
        }
        let request = CommandRequest::with_patch(invocation, patch);
        enforce_request_line_limit(&request)?;
        Ok(request)
    } else {
        let request = CommandRequest::from(invocation);
        enforce_request_line_limit(&request)?;
        Ok(request)
    }
}

fn enforce_request_line_limit(request: &CommandRequest) -> Result<(), AppError> {
    let json_len = serde_json::to_vec(request)
        .map_err(AppError::SerialiseRequest)?
        .len();
    if json_len + 1 > JSONL_REQUEST_MAX_LINE_BYTES {
        return Err(AppError::ReadPatch(Error::new(
            ErrorKind::UnexpectedEof,
            "command request exceeds maximum JSONL request size",
        )));
    }
    Ok(())
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
    tracing::debug!("retrying socket connection after daemon startup");
    connect_with_retry(
        context.config.daemon_socket(),
        transport::CONNECTION_TIMEOUT,
    )
    .map_err(|error| {
        tracing::warn!(error = %error, "failed to connect after daemon startup");
        write_error_and_fail(stderr, error)
    })
}

#[cfg(test)]
mod tests {
    //! Tests for daemon request construction helpers.

    use std::io::Cursor;

    use super::{MAX_PATCH_BYTES, build_request};
    use crate::{AppError, CommandInvocation, CommandRequest};

    enum ExpectedPatchRequest {
        Ok,
        MissingPatchInput,
        Oversized,
    }

    fn observe_status_invocation() -> CommandInvocation {
        CommandInvocation {
            domain: "observe".to_owned(),
            operation: "status".to_owned(),
            arguments: Vec::new(),
        }
    }

    fn observe_status_invocation_with_argument(argument: String) -> CommandInvocation {
        CommandInvocation {
            arguments: vec![argument],
            ..observe_status_invocation()
        }
    }

    fn apply_patch_invocation() -> CommandInvocation {
        CommandInvocation {
            domain: "act".to_owned(),
            operation: "apply-patch".to_owned(),
            arguments: Vec::new(),
        }
    }

    fn patch_with_jsonl_len(len: usize) -> Vec<u8> {
        let envelope_len = request_jsonl_len(&CommandRequest::with_patch(
            apply_patch_invocation(),
            String::new(),
        ));
        assert!(
            len > envelope_len,
            "requested JSONL line length must fit patch bytes"
        );
        vec![b'a'; len - envelope_len]
    }

    fn argument_with_jsonl_len(len: usize) -> String {
        let envelope_len = request_jsonl_len(&CommandRequest::from(
            observe_status_invocation_with_argument(String::new()),
        ));
        assert!(
            len > envelope_len,
            "requested JSONL line length must fit argument bytes"
        );
        "a".repeat(len - envelope_len)
    }

    fn request_jsonl_len(request: &CommandRequest) -> usize {
        match serde_json::to_vec(request) {
            Ok(bytes) => bytes.len() + 1,
            Err(error) => panic!("request must serialise: {error}"),
        }
    }

    #[test]
    fn non_patch_invocation_does_not_read_stdin() {
        let mut stdin = Cursor::new(b"should not be read".to_vec());
        let result = build_request(observe_status_invocation(), &mut stdin);
        assert!(result.is_ok());
    }

    #[test]
    fn non_patch_invocation_rejects_oversized_jsonl_request() {
        let invocation = observe_status_invocation_with_argument(argument_with_jsonl_len(
            MAX_PATCH_BYTES as usize + 1,
        ));
        let mut stdin = Cursor::new(b"should not be read".to_vec());
        let result = build_request(invocation, &mut stdin);

        assert!(matches!(
            result,
            Err(AppError::ReadPatch(error))
                if error.kind() == std::io::ErrorKind::UnexpectedEof
        ));
    }

    #[rstest::rstest]
    #[case::reads_patch_from_stdin(
        b"--- a/foo\n+++ b/foo\n@@ -1 +1 @@\n-old\n+new\n".to_vec(),
        ExpectedPatchRequest::Ok
    )]
    #[case::returns_error_for_empty_stdin(
        b"   \n".to_vec(),
        ExpectedPatchRequest::MissingPatchInput
    )]
    #[case::accepts_request_at_jsonl_size_limit(
        patch_with_jsonl_len(MAX_PATCH_BYTES as usize),
        ExpectedPatchRequest::Ok
    )]
    #[case::returns_error_for_oversized_jsonl_request(
        patch_with_jsonl_len(MAX_PATCH_BYTES as usize + 1),
        ExpectedPatchRequest::Oversized
    )]
    fn apply_patch_stdin_cases(#[case] input: Vec<u8>, #[case] expected: ExpectedPatchRequest) {
        let expected_patch = String::from_utf8(input.clone()).expect("patch input is UTF-8");
        let mut stdin = Cursor::new(input);
        let result = build_request(apply_patch_invocation(), &mut stdin);

        match expected {
            ExpectedPatchRequest::Ok => {
                let request = result.expect("patch input should build a request");
                assert_eq!(request.patch.as_deref(), Some(expected_patch.as_str()));
                assert!(request_jsonl_len(&request) <= MAX_PATCH_BYTES as usize);
            }
            ExpectedPatchRequest::MissingPatchInput => {
                assert!(matches!(result, Err(AppError::MissingPatchInput)));
            }
            ExpectedPatchRequest::Oversized => {
                assert!(matches!(
                    result,
                    Err(AppError::ReadPatch(error))
                        if error.kind() == std::io::ErrorKind::UnexpectedEof
                ));
            }
        }
    }
}
