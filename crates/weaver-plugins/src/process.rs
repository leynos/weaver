//! Process-based plugin execution using the Weaver sandbox.
//!
//! [`SandboxExecutor`] implements the [`PluginExecutor`] trait by spawning a
//! sandboxed child process, writing the request to stdin as a single JSONL
//! line, reading the response from stdout, and enforcing a timeout. This
//! module is the primary integration point with the `weaver-sandbox` crate.

use std::io::{BufRead, BufReader, Read, Write};
use std::sync::Arc;
use std::time::{Duration, Instant};

use tracing::{debug, warn};

use weaver_sandbox::SandboxProfile;
use weaver_sandbox::process::Stdio;

use crate::error::PluginError;
use crate::manifest::PluginManifest;
use crate::protocol::{PluginRequest, PluginResponse};
use crate::runner::PluginExecutor;

/// Tracing target for plugin process operations.
const PLUGIN_TARGET: &str = "weaver_plugins::process";

/// Executes plugins by spawning sandboxed child processes.
///
/// The executor builds a [`SandboxProfile`] from the manifest, spawns the
/// plugin command with stdin and stdout piped, writes the JSONL request,
/// reads the JSONL response, and waits for exit with a timeout.
///
/// # Example
///
/// ```rust,no_run
/// use weaver_plugins::process::SandboxExecutor;
/// use weaver_plugins::runner::PluginExecutor;
/// use weaver_plugins::{PluginManifest, PluginKind, PluginRequest};
/// use std::path::PathBuf;
///
/// let executor = SandboxExecutor;
/// let manifest = PluginManifest::new(
///     "example", "0.1.0", PluginKind::Actuator,
///     vec!["python".into()], PathBuf::from("/usr/bin/example-plugin"),
/// );
/// let request = PluginRequest::new("rename", vec![]);
/// // let response = executor.execute(&manifest, &request);
/// ```
pub struct SandboxExecutor;

impl PluginExecutor for SandboxExecutor {
    fn execute(
        &self,
        manifest: &PluginManifest,
        request: &PluginRequest,
    ) -> Result<PluginResponse, PluginError> {
        execute_in_sandbox(manifest, request)
    }
}

/// Builds the sandbox profile for a plugin.
fn build_profile(manifest: &PluginManifest) -> SandboxProfile {
    SandboxProfile::new().allow_executable(manifest.executable())
}

/// Spawns the plugin process, writes the request, reads the response.
fn execute_in_sandbox(
    manifest: &PluginManifest,
    request: &PluginRequest,
) -> Result<PluginResponse, PluginError> {
    let name = manifest.name();
    let profile = build_profile(manifest);
    let sandbox = weaver_sandbox::Sandbox::new(profile);

    let mut command = weaver_sandbox::SandboxCommand::new(manifest.executable());
    command.args(manifest.args());
    command.stdin(Stdio::piped());
    command.stdout(Stdio::piped());
    command.stderr(Stdio::piped());

    debug!(
        target: PLUGIN_TARGET,
        plugin = name,
        executable = %manifest.executable().display(),
        "spawning plugin process"
    );

    let mut child = sandbox.spawn(command).map_err(|err| PluginError::Sandbox {
        name: name.to_owned(),
        message: err.to_string(),
    })?;

    let stdin = child.stdin.take().ok_or_else(|| PluginError::SpawnFailed {
        name: name.to_owned(),
        message: String::from("failed to capture stdin"),
        source: None,
    })?;

    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| PluginError::SpawnFailed {
            name: name.to_owned(),
            message: String::from("failed to capture stdout"),
            source: None,
        })?;

    let stderr = child.stderr.take();

    write_request(name, stdin, request)?;
    let response_line = read_response(name, stdout, manifest.timeout_secs())?;
    drain_stderr(name, stderr);
    wait_for_exit(name, &mut child, manifest.timeout_secs())?;
    parse_response(name, &response_line)
}

/// Writes the serialised request to the plugin's stdin and closes it.
fn write_request(
    name: &str,
    mut stdin: impl Write,
    request: &PluginRequest,
) -> Result<(), PluginError> {
    let json = serde_json::to_string(request).map_err(PluginError::SerializeRequest)?;

    debug!(
        target: PLUGIN_TARGET,
        plugin = name,
        request_bytes = json.len(),
        "writing request to plugin stdin"
    );

    stdin
        .write_all(json.as_bytes())
        .map_err(|err| PluginError::Io {
            name: name.to_owned(),
            source: Arc::new(err),
        })?;

    stdin.write_all(b"\n").map_err(|err| PluginError::Io {
        name: name.to_owned(),
        source: Arc::new(err),
    })?;

    stdin.flush().map_err(|err| PluginError::Io {
        name: name.to_owned(),
        source: Arc::new(err),
    })?;

    // Stdin is dropped here, closing the pipe to signal no more input.
    Ok(())
}

/// Reads a single JSONL line from the plugin's stdout.
fn read_response(name: &str, stdout: impl Read, timeout_secs: u64) -> Result<String, PluginError> {
    let start = Instant::now();
    let timeout = Duration::from_secs(timeout_secs);
    let mut reader = BufReader::new(stdout);
    let mut line = String::new();

    // Read until newline or EOF. The read itself may block if the plugin is
    // slow, but the timeout is enforced by the wait_for_exit step.
    let bytes_read = reader.read_line(&mut line).map_err(|err| PluginError::Io {
        name: name.to_owned(),
        source: Arc::new(err),
    })?;

    let elapsed = start.elapsed();
    debug!(
        target: PLUGIN_TARGET,
        plugin = name,
        bytes_read,
        elapsed_ms = u64::try_from(elapsed.as_millis()).unwrap_or(u64::MAX),
        "read response from plugin stdout"
    );

    if bytes_read == 0 {
        return Err(PluginError::InvalidOutput {
            name: name.to_owned(),
            message: String::from("plugin produced no output on stdout"),
        });
    }

    if elapsed > timeout {
        return Err(PluginError::Timeout {
            name: name.to_owned(),
            timeout_secs,
        });
    }

    Ok(line)
}

/// Drains stderr to avoid blocking the child on a full pipe buffer.
fn drain_stderr(name: &str, stderr_handle: Option<impl Read>) {
    let Some(reader) = stderr_handle else {
        return;
    };
    let mut buffer = String::new();
    if BufReader::new(reader).read_to_string(&mut buffer).is_ok() && !buffer.is_empty() {
        debug!(
            target: PLUGIN_TARGET,
            plugin = name,
            stderr = %buffer.trim(),
            "plugin stderr output"
        );
    }
}

/// Waits for the child process to exit, enforcing the timeout.
fn wait_for_exit(
    name: &str,
    child: &mut weaver_sandbox::SandboxChild,
    timeout_secs: u64,
) -> Result<(), PluginError> {
    let start = Instant::now();
    let timeout = Duration::from_secs(timeout_secs);
    let poll_interval = Duration::from_millis(50);

    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                debug!(
                    target: PLUGIN_TARGET,
                    plugin = name,
                    ?status,
                    "plugin process exited"
                );
                if status.success() {
                    return Ok(());
                }
                return Err(PluginError::NonZeroExit {
                    name: name.to_owned(),
                    status: status.code().unwrap_or(-1),
                });
            }
            Ok(None) => {
                if start.elapsed() > timeout {
                    warn!(
                        target: PLUGIN_TARGET,
                        plugin = name,
                        timeout_secs,
                        "plugin timed out, killing process"
                    );
                    drop(child.kill());
                    drop(child.wait());
                    return Err(PluginError::Timeout {
                        name: name.to_owned(),
                        timeout_secs,
                    });
                }
                std::thread::sleep(poll_interval);
            }
            Err(err) => {
                return Err(PluginError::Io {
                    name: name.to_owned(),
                    source: Arc::new(err),
                });
            }
        }
    }
}

/// Parses a JSONL response line into a [`PluginResponse`].
fn parse_response(name: &str, line: &str) -> Result<PluginResponse, PluginError> {
    serde_json::from_str(line.trim()).map_err(|err| PluginError::DeserializeResponse {
        message: format!("plugin '{name}' produced invalid JSON: {err}"),
        source: Some(err),
    })
}
