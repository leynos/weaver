//! High-level orchestration for daemon lifecycle commands.
//!
//! This module wires the start/stop/status flows together using the helpers in
//! `types` and `utils`, ensuring the CLI drives a single entrypoint when
//! interacting with `weaverd`.

use std::{io::Write, process::ExitCode, time::SystemTime};

use mockable::DefaultEnv;
use weaver_config::{Config, RuntimePaths, SocketEndpoint};

use super::{
    error::LifecycleError,
    monitoring::{HealthSnapshot, read_health, read_pid, wait_for_ready},
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

/// Observed liveness of a daemon that has no readable health snapshot.
#[derive(Clone, Copy, Debug)]
struct RuntimeProbe {
    /// Whether the configured daemon socket accepts connections.
    reachable: bool,
    /// Process identifier recorded in the PID file, when one exists.
    pid: Option<u32>,
}

/// Runtime paths and socket endpoint shared by the status-reporting helpers.
#[derive(Clone, Copy, Debug)]
struct RuntimeStatusContext<'a> {
    /// Paths derived from the daemon configuration.
    paths: &'a RuntimePaths,
    /// Endpoint the daemon would listen on.
    endpoint: &'a SocketEndpoint,
}

/// Production lifecycle controller.
///
/// The controller is stateless: every entry point is an associated function so
/// the CLI can drive it without threading an instance through the runtime.
#[derive(Debug, Default)]
pub struct SystemLifecycle;

impl SystemLifecycle {
    /// Executes the lifecycle command carried by `invocation`.
    ///
    /// # Errors
    ///
    /// Returns [`LifecycleError`] when the requested command fails.
    pub fn handle<W: Write, E: Write>(
        invocation: &LifecycleInvocation,
        context: LifecycleContext<'_>,
        output: &mut LifecycleOutput<W, E>,
    ) -> Result<ExitCode, LifecycleError> {
        match invocation.command {
            LifecycleCommand::Start => Self::start(invocation, context, output),
            LifecycleCommand::Stop => Self::stop(invocation, context, output),
            LifecycleCommand::Status => Self::status(invocation, context, output),
        }
    }

    /// Starts the daemon and waits for it to report readiness.
    fn start<W: Write, E: Write>(
        invocation: &LifecycleInvocation,
        context: LifecycleContext<'_>,
        output: &mut LifecycleOutput<W, E>,
    ) -> Result<ExitCode, LifecycleError> {
        ensure_no_extra_arguments(invocation)?;
        ensure_socket_available(context.config.daemon_socket())?;
        let paths = prepare_runtime(context)?;
        let mut child = spawn_daemon(
            context.config_arguments,
            context.daemon_binary,
            paths.runtime_dir(),
            &DefaultEnv,
        )?;
        let started_at = SystemTime::now();
        let snapshot = wait_for_ready(&paths, &mut child, started_at, STARTUP_TIMEOUT)?;
        write_startup_banner(output, context, &snapshot, &paths)?;
        Ok(ExitCode::SUCCESS)
    }

    /// Stops the daemon recorded in the runtime directory.
    fn stop<W: Write, E: Write>(
        invocation: &LifecycleInvocation,
        context: LifecycleContext<'_>,
        output: &mut LifecycleOutput<W, E>,
    ) -> Result<ExitCode, LifecycleError> {
        ensure_no_extra_arguments(invocation)?;
        let paths = prepare_runtime(context)?;
        let dir = open_runtime_dir(&paths)?;

        match read_pid(&dir, paths.pid_file_name(), paths.pid_path())? {
            Some(pid) => stop_daemon(pid, &paths, context, output),
            None => report_missing_pid(&paths, context, output),
        }
    }

    /// Returns the daemon runtime paths from the current configuration.
    fn check_daemon_paths(config: &Config) -> Result<RuntimePaths, LifecycleError> {
        RuntimePaths::from_config_readonly(config).map_err(Into::into)
    }

    /// Reports daemon status when a valid health snapshot is available.
    fn report_healthy_status<W: Write, E: Write>(
        snapshot: &HealthSnapshot,
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
        runtime: RuntimeStatusContext<'_>,
        output: &mut LifecycleOutput<W, E>,
    ) -> Result<(), LifecycleError> {
        output.stdout_line(format_args!(
            concat!(
                "daemon socket {} is listening but endpoint-scoped runtime files are missing \
                 under ",
                "{}; if this daemon predates endpoint-scoped filenames, stop it with the previous ",
                "version or its known PID before upgrading"
            ),
            runtime.endpoint,
            runtime.paths.runtime_dir().display()
        ))
    }

    /// Reports that the daemon is not running.
    fn report_not_running<W: Write, E: Write>(
        output: &mut LifecycleOutput<W, E>,
    ) -> Result<(), LifecycleError> {
        output.stdout_line(format_args!(
            "daemon is not running; use 'weaver daemon start' to launch it."
        ))
    }

    /// Reports daemon status when health snapshot is missing but runtime exists.
    fn report_degraded_status<W: Write, E: Write>(
        probe: RuntimeProbe,
        runtime: RuntimeStatusContext<'_>,
        output: &mut LifecycleOutput<W, E>,
    ) -> Result<(), LifecycleError> {
        if let Some(pid) = probe.pid {
            return Self::report_missing_health(pid, runtime.paths, output);
        }

        if probe.reachable {
            return Self::report_socket_without_pid(runtime, output);
        }

        Self::report_not_running(output)
    }

    /// Reports the daemon's current status.
    fn status<W: Write, E: Write>(
        invocation: &LifecycleInvocation,
        context: LifecycleContext<'_>,
        output: &mut LifecycleOutput<W, E>,
    ) -> Result<ExitCode, LifecycleError> {
        ensure_no_extra_arguments(invocation)?;

        let paths = Self::check_daemon_paths(context.config)?;

        if !paths
            .runtime_dir()
            .try_exists()
            .map_err(LifecycleError::Io)?
        {
            Self::report_not_running(output)?;
            return Ok(ExitCode::SUCCESS);
        }

        let dir = open_runtime_dir(&paths)?;
        let health = read_health(&dir, paths.health_file_name(), paths.health_path())?;
        let runtime = RuntimeStatusContext {
            paths: &paths,
            endpoint: context.config.daemon_socket(),
        };

        if let Some(snapshot) = health {
            Self::report_healthy_status(&snapshot, &context, output)?;
            return Ok(ExitCode::SUCCESS);
        }

        let pid = read_pid(&dir, paths.pid_file_name(), paths.pid_path())?;
        let reachable = socket_is_reachable(context.config.daemon_socket())?;
        Self::report_degraded_status(RuntimeProbe { reachable, pid }, runtime, output)?;
        Ok(ExitCode::SUCCESS)
    }
}

/// Signals the daemon recorded by `pid` and reports the shutdown.
fn stop_daemon<W: Write, E: Write>(
    pid: u32,
    paths: &RuntimePaths,
    context: LifecycleContext<'_>,
    output: &mut LifecycleOutput<W, E>,
) -> Result<ExitCode, LifecycleError> {
    signal_daemon(pid)?;
    wait_for_shutdown(paths, context.config.daemon_socket())?;
    output.stdout_line(format_args!("daemon pid {pid} stopped cleanly"))?;
    output.stderr_line(format_args!(
        "retained persistent runtime lock at {}",
        paths.lock_path().display()
    ))?;
    Ok(ExitCode::SUCCESS)
}

/// Reports the stop outcome when no PID file records a daemon.
///
/// A reachable socket without a PID file is an inconsistent runtime state the
/// operator has to resolve, so it stays an error rather than a plain message.
fn report_missing_pid<W: Write, E: Write>(
    paths: &RuntimePaths,
    context: LifecycleContext<'_>,
    output: &mut LifecycleOutput<W, E>,
) -> Result<ExitCode, LifecycleError> {
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
    Ok(ExitCode::SUCCESS)
}

#[cfg(test)]
mod tests {
    //! Unit tests for lifecycle status reporting.

    use weaver_config::Config;

    use super::*;

    #[test]
    fn status_guides_operators_away_from_unverified_legacy_processes() {
        let config = Config::default();
        let paths = RuntimePaths::from_config_readonly(&config)
            .expect("default runtime paths should derive without filesystem access");
        let runtime = RuntimeStatusContext {
            paths: &paths,
            endpoint: config.daemon_socket(),
        };
        let mut output = LifecycleOutput::new(Vec::new(), Vec::new());

        SystemLifecycle::report_socket_without_pid(runtime, &mut output)
            .expect("status message should be written");

        let stdout = String::from_utf8(output.stdout).expect("status output should be UTF-8");
        assert!(stdout.contains("endpoint-scoped runtime files are missing"));
        assert!(stdout.contains("stop it with the previous version or its known PID"));
        assert!(!stdout.contains("removing"));
    }
}
