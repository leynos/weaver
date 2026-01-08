//! High-level orchestration for daemon lifecycle commands.
//!
//! This module wires the start/stop/status flows together using the helpers in
//! `types` and `utils`, ensuring the CLI drives a single entrypoint when
//! interacting with `weaverd`.

use std::io::Write;
use std::process::ExitCode;
use std::time::SystemTime;

use weaver_config::RuntimePaths;

use super::error::LifecycleError;
use super::monitoring::{read_health, read_pid, wait_for_ready};
use super::shutdown::{signal_daemon, wait_for_shutdown};
use super::socket::{ensure_socket_available, socket_is_reachable};
use super::spawning::spawn_daemon;
use super::types::{LifecycleCommand, LifecycleContext, LifecycleInvocation, LifecycleOutput};
use super::utils::{
    STARTUP_TIMEOUT, ensure_no_extra_arguments, prepare_runtime, write_startup_banner,
};

/// Production lifecycle controller.
#[derive(Debug, Default)]
pub struct SystemLifecycle;

impl SystemLifecycle {
    pub fn handle<W: Write, E: Write>(
        &mut self,
        invocation: LifecycleInvocation,
        context: LifecycleContext<'_>,
        output: &mut LifecycleOutput<W, E>,
    ) -> Result<ExitCode, LifecycleError> {
        match invocation.command {
            LifecycleCommand::Start => self.start(&invocation, context, output),
            LifecycleCommand::Stop => self.stop(&invocation, context, output),
            LifecycleCommand::Status => self.status(&invocation, context, output),
        }
    }

    fn start<W: Write, E: Write>(
        &mut self,
        invocation: &LifecycleInvocation,
        context: LifecycleContext<'_>,
        output: &mut LifecycleOutput<W, E>,
    ) -> Result<ExitCode, LifecycleError> {
        ensure_no_extra_arguments(invocation)?;
        ensure_socket_available(context.config.daemon_socket())?;
        let paths = prepare_runtime(context)?;
        let mut child = spawn_daemon(context.config_arguments, context.daemon_binary)?;
        let started_at = SystemTime::now();
        let snapshot = wait_for_ready(&paths, &mut child, started_at, STARTUP_TIMEOUT)?;
        write_startup_banner(output, context, &snapshot, &paths)?;
        Ok(ExitCode::SUCCESS)
    }

    fn stop<W: Write, E: Write>(
        &mut self,
        invocation: &LifecycleInvocation,
        context: LifecycleContext<'_>,
        output: &mut LifecycleOutput<W, E>,
    ) -> Result<ExitCode, LifecycleError> {
        ensure_no_extra_arguments(invocation)?;
        let paths = prepare_runtime(context)?;
        let pid = read_pid(paths.pid_path())?;
        let Some(pid) = pid else {
            if socket_is_reachable(context.config.daemon_socket())? {
                return Err(LifecycleError::MissingPidWithSocket {
                    path: paths.pid_path().to_path_buf(),
                    endpoint: context.config.daemon_socket().to_string(),
                });
            }
            output.stdout_line(format_args!(
                "daemon is not running (pid file missing at {})",
                paths.pid_path().display()
            ))?;
            return Ok(ExitCode::SUCCESS);
        };
        signal_daemon(pid)?;
        wait_for_shutdown(&paths, context.config.daemon_socket())?;
        output.stdout_line(format_args!("daemon pid {pid} stopped cleanly"))?;
        output.stderr_line(format_args!(
            "removed runtime artefacts from {}",
            paths.runtime_dir().display()
        ))?;
        Ok(ExitCode::SUCCESS)
    }

    fn status<W: Write, E: Write>(
        &mut self,
        invocation: &LifecycleInvocation,
        context: LifecycleContext<'_>,
        output: &mut LifecycleOutput<W, E>,
    ) -> Result<ExitCode, LifecycleError> {
        ensure_no_extra_arguments(invocation)?;
        let paths = match RuntimePaths::from_config_readonly(context.config) {
            Ok(paths) => paths,
            Err(_) => {
                output.stdout_line(format_args!(
                    "daemon is not running; use 'weaver daemon start' to launch it."
                ))?;
                return Ok(ExitCode::SUCCESS);
            }
        };
        if !paths.runtime_dir().exists() {
            output.stdout_line(format_args!(
                "daemon is not running; use 'weaver daemon start' to launch it."
            ))?;
            return Ok(ExitCode::SUCCESS);
        }
        let snapshot = read_health(paths.health_path())?;
        if let Some(snapshot) = snapshot {
            output.stdout_line(format_args!(
                "daemon status: {} (pid {}) via {}",
                snapshot.status,
                snapshot.pid,
                context.config.daemon_socket()
            ))?;
            return Ok(ExitCode::SUCCESS);
        }
        let reachable = socket_is_reachable(context.config.daemon_socket())?;
        let pid = read_pid(paths.pid_path())?;
        match pid {
            Some(pid) => {
                output.stdout_line(format_args!(
                    "daemon recorded pid {pid} but health snapshot is missing; check {}",
                    paths.health_path().display()
                ))?;
            }
            None if reachable => {
                output.stdout_line(format_args!(
                    "daemon socket {} is listening but runtime files are missing; consider 'weaver daemon stop' or removing {}",
                    context.config.daemon_socket(),
                    paths.runtime_dir().display()
                ))?;
            }
            None => {
                output.stdout_line(format_args!(
                    "daemon is not running; use 'weaver daemon start' to launch it."
                ))?;
            }
        }
        Ok(ExitCode::SUCCESS)
    }
}
