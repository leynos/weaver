//! High-level orchestration for daemon lifecycle commands.
//!
//! This module wires the start/stop/status flows together using the helpers in
//! `types` and `utils`, ensuring the CLI drives a single entrypoint when
//! interacting with `weaverd`.

use std::{io::Write, process::ExitCode, time::SystemTime};

use weaver_config::{RuntimePaths, RuntimePathsError, SocketEndpoint};

use super::{
    error::LifecycleError,
    monitoring::{HEALTH_FILENAME, PID_FILENAME, read_health, read_pid, wait_for_ready},
    shutdown::{signal_daemon, wait_for_shutdown},
    socket::{ensure_socket_available, socket_is_reachable},
    spawning::spawn_daemon,
    types::{LifecycleCommand, LifecycleContext, LifecycleInvocation, LifecycleOutput},
    utils::{
        STARTUP_TIMEOUT,
        ensure_no_extra_arguments,
        open_runtime_dir,
        prepare_runtime,
        write_startup_banner,
    },
};

#[derive(Clone, Copy, Debug)]
struct RuntimeProbe {
    reachable: bool,
    pid: Option<u32>,
}

#[derive(Clone, Copy, Debug)]
struct RuntimeStatusContext<'a> {
    paths: &'a RuntimePaths,
    endpoint: &'a SocketEndpoint,
}

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
        let dir = open_runtime_dir(&paths)?;
        let pid = read_pid(&dir, PID_FILENAME, paths.pid_path())?;
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

    /// Checks if the daemon is running by attempting to read runtime paths.
    fn check_daemon_paths(
        &self,
        config: &weaver_config::Config,
    ) -> Result<Option<RuntimePaths>, LifecycleError> {
        match RuntimePaths::from_config_readonly(config) {
            Ok(paths) => Ok(Some(paths)),
            Err(RuntimePathsError::MissingSocketParent { .. }) => Ok(None),
            Err(error) => Err(error.into()),
        }
    }

    /// Reports daemon status when a valid health snapshot is available.
    fn report_healthy_status<W: Write, E: Write>(
        &self,
        snapshot: &super::monitoring::HealthSnapshot,
        context: &LifecycleContext<'_>,
        output: &mut LifecycleOutput<W, E>,
    ) -> Result<(), LifecycleError> {
        output.stdout_line(format_args!(
            "daemon status: {} (pid {}) via {}",
            snapshot.status,
            snapshot.pid,
            context.config.daemon_socket()
        ))
    }

    /// Reports status when PID is present but health snapshot is missing.
    fn report_missing_health<W: Write, E: Write>(
        &self,
        pid: u32,
        paths: &RuntimePaths,
        output: &mut LifecycleOutput<W, E>,
    ) -> Result<(), LifecycleError> {
        output.stdout_line(format_args!(
            "daemon recorded pid {pid} but health snapshot is missing; check {}",
            paths.health_path().display()
        ))
    }

    /// Reports status when socket is reachable but PID file is missing.
    fn report_socket_without_pid<W: Write, E: Write>(
        &self,
        runtime: RuntimeStatusContext<'_>,
        output: &mut LifecycleOutput<W, E>,
    ) -> Result<(), LifecycleError> {
        output.stdout_line(format_args!(
            concat!(
                "daemon socket {} is listening but runtime files are missing; consider 'weaver ",
                "daemon ",
                "stop' or removing {}"
            ),
            runtime.endpoint,
            runtime.paths.runtime_dir().display()
        ))
    }

    /// Reports that the daemon is not running.
    fn report_not_running<W: Write, E: Write>(
        &self,
        output: &mut LifecycleOutput<W, E>,
    ) -> Result<(), LifecycleError> {
        output.stdout_line(format_args!(
            "daemon is not running; use 'weaver daemon start' to launch it."
        ))
    }

    /// Reports daemon status when health snapshot is missing but runtime exists.
    fn report_degraded_status<W: Write, E: Write>(
        &self,
        probe: RuntimeProbe,
        runtime: RuntimeStatusContext<'_>,
        output: &mut LifecycleOutput<W, E>,
    ) -> Result<(), LifecycleError> {
        if let Some(pid) = probe.pid {
            return self.report_missing_health(pid, runtime.paths, output);
        }

        if probe.reachable {
            return self.report_socket_without_pid(runtime, output);
        }

        self.report_not_running(output)
    }

    fn status<W: Write, E: Write>(
        &mut self,
        invocation: &LifecycleInvocation,
        context: LifecycleContext<'_>,
        output: &mut LifecycleOutput<W, E>,
    ) -> Result<ExitCode, LifecycleError> {
        ensure_no_extra_arguments(invocation)?;

        let paths = match self.check_daemon_paths(context.config)? {
            Some(paths) => paths,
            None => {
                self.report_not_running(output)?;
                return Ok(ExitCode::SUCCESS);
            }
        };

        if !paths.runtime_dir().exists() {
            self.report_not_running(output)?;
            return Ok(ExitCode::SUCCESS);
        }

        let dir = open_runtime_dir(&paths)?;
        let snapshot = read_health(&dir, HEALTH_FILENAME, paths.health_path())?;
        let runtime = RuntimeStatusContext {
            paths: &paths,
            endpoint: context.config.daemon_socket(),
        };

        if let Some(snapshot) = snapshot {
            self.report_healthy_status(&snapshot, &context, output)?;
            return Ok(ExitCode::SUCCESS);
        }

        let pid = read_pid(&dir, PID_FILENAME, paths.pid_path())?;
        let reachable = socket_is_reachable(context.config.daemon_socket())?;
        self.report_degraded_status(RuntimeProbe { reachable, pid }, runtime, output)?;
        Ok(ExitCode::SUCCESS)
    }
}
