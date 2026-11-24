//! Cleanup for the short-lived rust-analyzer child process and reader task.

use std::process::Child;

use super::RustAnalyzerProcess;
use crate::RustAnalyzerAdapterError;

/// Shuts the server down cleanly, falling back to termination if the shutdown
/// handshake fails.
pub(super) fn close_session(
    mut process: RustAnalyzerProcess,
) -> Result<(), RustAnalyzerAdapterError> {
    if let Err(error) = super::shutdown_session(&mut process) {
        terminate_session(process);
        return Err(error);
    }

    finish_session(process)
}

/// Abandons a session, closing its input, killing the server, and joining the
/// reader so no background work outlives the process it reads.
pub(super) fn terminate_session(process: RustAnalyzerProcess) {
    let RustAnalyzerProcess {
        mut child,
        reader,
        reader_thread,
        writer,
    } = process;
    drop(writer);
    force_terminate_process(&mut child);
    drop(reader);
    reader_thread.join().ok();
}

/// Closes input, waits for the server, and reports a non-zero exit as an
/// engine failure.
fn finish_session(process: RustAnalyzerProcess) -> Result<(), RustAnalyzerAdapterError> {
    let RustAnalyzerProcess {
        mut child,
        reader,
        reader_thread,
        writer,
    } = process;
    drop(writer);

    let status = match child.wait() {
        Ok(status) => status,
        Err(source) => {
            force_terminate_process(&mut child);
            drop(reader);
            reader_thread.join().ok();
            return Err(RustAnalyzerAdapterError::EngineFailed {
                message: format!("failed to wait for rust-analyzer process: {source}"),
            });
        }
    };
    drop(reader);
    reader_thread
        .join()
        .map_err(|_| RustAnalyzerAdapterError::EngineFailed {
            message: String::from("rust-analyzer LSP reader thread panicked"),
        })?;

    if !status.success() {
        return Err(RustAnalyzerAdapterError::EngineFailed {
            message: format!("rust-analyzer exited with status {status}"),
        });
    }

    Ok(())
}

/// Kills the child and reaps it, ignoring errors because the process may
/// already have exited.
fn force_terminate_process(child: &mut Child) {
    child.kill().ok();
    child.wait().ok();
}
