//! Implements the daemon lifecycle commands exposed by the CLI.
//!
//! The lifecycle controller is responsible for orchestrating `weaverd`
//! start-up, shutdown, and status reporting without speaking the daemon's
//! JSONL protocol. It inspects the shared runtime artefacts written by the
//! daemon supervisor and spawns the daemon binary with the same configuration
//! overrides supplied to the CLI.

use std::ffi::OsString;
use std::fmt;
use std::fs;
use std::io::{self, Write};
use std::net::{SocketAddr, TcpStream, ToSocketAddrs};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, ExitCode, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use serde::Deserialize;
use thiserror::Error;
use weaver_config::{Config, RuntimePaths, RuntimePathsError, SocketEndpoint};

use crate::AppError;

#[cfg(unix)]
use libc::{SIGTERM, kill};
#[cfg(unix)]
use socket2::{Domain, SockAddr, Socket, Type};

const STARTUP_TIMEOUT: Duration = Duration::from_secs(10);
const SHUTDOWN_TIMEOUT: Duration = Duration::from_secs(10);
const POLL_INTERVAL: Duration = Duration::from_millis(200);
const SOCKET_PROBE_TIMEOUT: Duration = Duration::from_secs(5);

/// Lifecycle command recognised by the CLI.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum LifecycleCommand {
    Start,
    Stop,
    Status,
}

impl LifecycleCommand {
    fn as_str(self) -> &'static str {
        match self {
            Self::Start => "start",
            Self::Stop => "stop",
            Self::Status => "status",
        }
    }
}

impl fmt::Display for LifecycleCommand {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

/// Invocation details for a lifecycle command.
#[derive(Debug, Clone)]
pub(crate) struct LifecycleInvocation {
    pub(crate) command: LifecycleCommand,
    pub(crate) arguments: Vec<String>,
}

/// Context shared with lifecycle handlers.
#[derive(Debug, Clone, Copy)]
pub(crate) struct LifecycleContext<'a> {
    pub(crate) config: &'a Config,
    pub(crate) config_arguments: &'a [OsString],
}

/// Wrapper around the CLI output streams used by lifecycle handlers.
pub(crate) struct LifecycleOutput<W, E> {
    pub(crate) stdout: W,
    pub(crate) stderr: E,
}

impl<W: Write, E: Write> LifecycleOutput<W, E> {
    pub(crate) fn new(stdout: W, stderr: E) -> Self {
        Self { stdout, stderr }
    }

    fn stdout_line(&mut self, args: fmt::Arguments<'_>) -> Result<(), LifecycleError> {
        self.stdout.write_fmt(args).map_err(LifecycleError::Io)
    }

    fn stderr_line(&mut self, args: fmt::Arguments<'_>) -> Result<(), LifecycleError> {
        self.stderr.write_fmt(args).map_err(LifecycleError::Io)
    }
}

/// Abstract interface for handling lifecycle commands.
pub(crate) trait LifecycleHandler {
    fn handle<W: Write, E: Write>(
        &self,
        invocation: LifecycleInvocation,
        context: LifecycleContext<'_>,
        output: &mut LifecycleOutput<W, E>,
    ) -> Result<ExitCode, AppError>;
}

/// System lifecycle controller that coordinates with the real daemon.
#[derive(Debug, Default)]
pub(crate) struct SystemLifecycle;

impl LifecycleHandler for SystemLifecycle {
    fn handle<W: Write, E: Write>(
        &self,
        invocation: LifecycleInvocation,
        context: LifecycleContext<'_>,
        output: &mut LifecycleOutput<W, E>,
    ) -> Result<ExitCode, AppError> {
        let result = match invocation.command {
            LifecycleCommand::Start => self.start(&invocation, context, output),
            LifecycleCommand::Stop => self.stop(&invocation, context, output),
            LifecycleCommand::Status => self.status(&invocation, context, output),
        };
        result.map_err(AppError::from)
    }
}

impl SystemLifecycle {
    fn start<W: Write, E: Write>(
        &self,
        invocation: &LifecycleInvocation,
        context: LifecycleContext<'_>,
        output: &mut LifecycleOutput<W, E>,
    ) -> Result<ExitCode, LifecycleError> {
        ensure_no_extra_arguments(invocation)?;
        ensure_socket_available(context.config.daemon_socket())?;
        let paths = RuntimePaths::from_config(context.config)?;
        let mut child = spawn_daemon(context.config_arguments)?;
        let snapshot = wait_for_ready(&paths, &mut child)?;
        output.stdout_line(format_args!(
            "daemon ready (pid {}) on {}\n",
            snapshot.pid,
            context.config.daemon_socket()
        ))?;
        output.stderr_line(format_args!(
            "runtime artefacts stored under {}\n",
            paths.runtime_dir().display()
        ))?;
        Ok(ExitCode::SUCCESS)
    }

    fn stop<W: Write, E: Write>(
        &self,
        invocation: &LifecycleInvocation,
        context: LifecycleContext<'_>,
        output: &mut LifecycleOutput<W, E>,
    ) -> Result<ExitCode, LifecycleError> {
        ensure_no_extra_arguments(invocation)?;
        let paths = RuntimePaths::from_config(context.config)?;
        let pid = read_pid(paths.pid_path())?;
        let Some(pid) = pid else {
            if socket_is_reachable(context.config.daemon_socket())? {
                return Err(LifecycleError::MissingPidWithSocket {
                    path: paths.pid_path().to_path_buf(),
                    endpoint: context.config.daemon_socket().to_string(),
                });
            }
            output.stdout_line(format_args!(
                "daemon is not running (pid file missing at {})\n",
                paths.pid_path().display()
            ))?;
            return Ok(ExitCode::SUCCESS);
        };
        signal_daemon(pid)?;
        wait_for_shutdown(&paths, context.config.daemon_socket())?;
        output.stdout_line(format_args!("daemon pid {pid} stopped cleanly\n"))?;
        output.stderr_line(format_args!(
            "removed runtime artefacts from {}\n",
            paths.runtime_dir().display()
        ))?;
        Ok(ExitCode::SUCCESS)
    }

    fn status<W: Write, E: Write>(
        &self,
        invocation: &LifecycleInvocation,
        context: LifecycleContext<'_>,
        output: &mut LifecycleOutput<W, E>,
    ) -> Result<ExitCode, LifecycleError> {
        ensure_no_extra_arguments(invocation)?;
        let paths = RuntimePaths::from_config(context.config)?;
        let snapshot = read_health(paths.health_path())?;
        let reachable = socket_is_reachable(context.config.daemon_socket())?;
        if let Some(snapshot) = snapshot {
            output.stdout_line(format_args!(
                "daemon status: {} (pid {}) via {}\n",
                snapshot.status,
                snapshot.pid,
                context.config.daemon_socket()
            ))?;
            return Ok(ExitCode::SUCCESS);
        }
        let pid = read_pid(paths.pid_path())?;
        match pid {
            Some(pid) => {
                output.stdout_line(format_args!(
                    "daemon recorded pid {pid} but health snapshot is missing; check {}\n",
                    paths.health_path().display()
                ))?;
            }
            None if reachable => {
                output.stdout_line(format_args!(
                    "daemon socket {} is listening but runtime files are missing; consider 'weaver daemon stop' or removing {}\n",
                    context.config.daemon_socket(),
                    paths.runtime_dir().display()
                ))?;
            }
            None => {
                output.stdout_line(format_args!(
                    "daemon is not running; use 'weaver daemon start' to launch it.\n"
                ))?;
            }
        }
        Ok(ExitCode::SUCCESS)
    }
}

fn ensure_no_extra_arguments(invocation: &LifecycleInvocation) -> Result<(), LifecycleError> {
    if let Some(argument) = invocation.arguments.first() {
        return Err(LifecycleError::UnexpectedArgument {
            command: invocation.command,
            argument: argument.clone(),
        });
    }
    Ok(())
}

fn spawn_daemon(config_arguments: &[OsString]) -> Result<Child, LifecycleError> {
    let binary = daemon_binary();
    let mut command = Command::new(&binary);
    if config_arguments.len() > 1 {
        for arg in &config_arguments[1..] {
            command.arg(arg);
        }
    }
    command.stdout(Stdio::inherit()).stderr(Stdio::inherit());
    command
        .spawn()
        .map_err(|source| LifecycleError::LaunchDaemon { binary, source })
}

fn daemon_binary() -> OsString {
    std::env::var_os("WEAVERD_BIN").unwrap_or_else(|| OsString::from("weaverd"))
}

fn wait_for_ready(
    paths: &RuntimePaths,
    child: &mut Child,
) -> Result<HealthSnapshot, LifecycleError> {
    let deadline = Instant::now() + STARTUP_TIMEOUT;
    while Instant::now() < deadline {
        if let Some(snapshot) = read_health(paths.health_path())? {
            if snapshot.status == "ready" {
                return Ok(snapshot);
            }
            if snapshot.status == "stopping" {
                return Err(LifecycleError::StartupAborted {
                    path: paths.health_path().to_path_buf(),
                });
            }
        }
        if let Some(status) = child
            .try_wait()
            .map_err(|source| LifecycleError::MonitorChild { source })?
            .filter(|status| !status.success())
        {
            return Err(LifecycleError::StartupFailed {
                exit_status: status.code(),
            });
        }
        thread::sleep(POLL_INTERVAL);
    }
    Err(LifecycleError::StartupTimeout {
        health_path: paths.health_path().to_path_buf(),
        timeout_ms: STARTUP_TIMEOUT.as_millis() as u64,
    })
}

fn wait_for_shutdown(
    paths: &RuntimePaths,
    endpoint: &SocketEndpoint,
) -> Result<(), LifecycleError> {
    let deadline = Instant::now() + SHUTDOWN_TIMEOUT;
    while Instant::now() < deadline {
        let pid_exists = paths.pid_path().exists();
        let socket_busy = socket_is_reachable(endpoint)?;
        if !pid_exists && !socket_busy {
            return Ok(());
        }
        thread::sleep(POLL_INTERVAL);
    }
    Err(LifecycleError::ShutdownTimeout {
        pid_path: paths.pid_path().to_path_buf(),
        timeout_ms: SHUTDOWN_TIMEOUT.as_millis() as u64,
    })
}

fn read_health(path: &Path) -> Result<Option<HealthSnapshot>, LifecycleError> {
    match fs::read_to_string(path) {
        Ok(content) => {
            serde_json::from_str(&content)
                .map(Some)
                .map_err(|source| LifecycleError::ParseHealth {
                    path: path.to_path_buf(),
                    source,
                })
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(source) => Err(LifecycleError::ReadHealth {
            path: path.to_path_buf(),
            source,
        }),
    }
}

fn read_pid(path: &Path) -> Result<Option<u32>, LifecycleError> {
    match fs::read_to_string(path) {
        Ok(content) => {
            let trimmed = content.trim();
            if trimmed.is_empty() {
                return Ok(None);
            }
            trimmed
                .parse::<u32>()
                .map(Some)
                .map_err(|source| LifecycleError::ParsePid {
                    path: path.to_path_buf(),
                    source,
                })
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(source) => Err(LifecycleError::ReadPid {
            path: path.to_path_buf(),
            source,
        }),
    }
}

fn ensure_socket_available(endpoint: &SocketEndpoint) -> Result<(), LifecycleError> {
    if socket_is_reachable(endpoint)? {
        return Err(LifecycleError::SocketInUse {
            endpoint: endpoint.to_string(),
        });
    }
    Ok(())
}

fn socket_is_reachable(endpoint: &SocketEndpoint) -> Result<bool, LifecycleError> {
    match try_connect(endpoint) {
        Ok(_) => Ok(true),
        Err(error) if is_socket_available(&error) => Ok(false),
        Err(source) => Err(LifecycleError::SocketProbe {
            endpoint: endpoint.to_string(),
            source,
        }),
    }
}

fn try_connect(endpoint: &SocketEndpoint) -> io::Result<()> {
    match endpoint {
        SocketEndpoint::Tcp { host, port } => {
            let address = resolve_tcp(host, *port)?;
            TcpStream::connect_timeout(&address, SOCKET_PROBE_TIMEOUT).map(|_| ())
        }
        SocketEndpoint::Unix { path } => connect_unix(path.as_str()),
    }
}

fn resolve_tcp(host: &str, port: u16) -> io::Result<SocketAddr> {
    let mut addrs = (host, port).to_socket_addrs()?;
    addrs
        .find(|addr| matches!(addr, SocketAddr::V4(_) | SocketAddr::V6(_)))
        .ok_or_else(|| io::Error::new(io::ErrorKind::AddrNotAvailable, "no resolved address"))
}

#[cfg(unix)]
fn connect_unix(path: &str) -> io::Result<()> {
    let socket = Socket::new(Domain::UNIX, Type::STREAM, None)?;
    let address = SockAddr::unix(path)?;
    socket.connect_timeout(&address, SOCKET_PROBE_TIMEOUT)
}

#[cfg(not(unix))]
fn connect_unix(_path: &str) -> io::Result<()> {
    Err(io::Error::new(
        io::ErrorKind::Unsupported,
        "unix sockets unsupported on this platform",
    ))
}

fn is_socket_available(error: &io::Error) -> bool {
    matches!(
        error.kind(),
        io::ErrorKind::ConnectionRefused
            | io::ErrorKind::ConnectionReset
            | io::ErrorKind::NotFound
            | io::ErrorKind::AddrNotAvailable
    )
}

fn signal_daemon(pid: u32) -> Result<(), LifecycleError> {
    #[cfg(unix)]
    {
        let result = unsafe { kill(pid as libc::pid_t, SIGTERM) };
        if result == 0 {
            Ok(())
        } else {
            Err(LifecycleError::SignalFailed {
                pid,
                source: io::Error::last_os_error(),
            })
        }
    }
    #[cfg(not(unix))]
    {
        let _ = pid;
        Err(LifecycleError::UnsupportedPlatform)
    }
}

#[derive(Debug, Deserialize, PartialEq, Eq)]
struct HealthSnapshot {
    status: String,
    pid: u32,
    timestamp: u64,
}

/// Errors raised by lifecycle operations.
#[derive(Debug, Error)]
pub enum LifecycleError {
    #[error("unexpected argument '{argument}' for 'daemon {command}'")]
    UnexpectedArgument {
        command: LifecycleCommand,
        argument: String,
    },
    #[error(
        "daemon socket {endpoint} is already in use; stop the existing daemon or change --daemon-socket"
    )]
    SocketInUse { endpoint: String },
    #[error("failed to probe daemon socket {endpoint}: {source}")]
    SocketProbe {
        endpoint: String,
        #[source]
        source: io::Error,
    },
    #[error("failed to spawn weaverd binary '{binary:?}': {source}")]
    LaunchDaemon {
        binary: OsString,
        #[source]
        source: io::Error,
    },
    #[error("daemon exited before reporting ready (status: {exit_status:?})")]
    StartupFailed { exit_status: Option<i32> },
    #[error("daemon reported 'stopping' before reaching ready; check health snapshot at {path:?}")]
    StartupAborted { path: PathBuf },
    #[error("timed out waiting for ready snapshot in {timeout_ms} ms at {health_path:?}")]
    StartupTimeout {
        health_path: PathBuf,
        timeout_ms: u64,
    },
    #[error("failed to monitor daemon launch: {source}")]
    MonitorChild {
        #[source]
        source: io::Error,
    },
    #[error("failed to read health snapshot {path:?}: {source}")]
    ReadHealth {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error("failed to parse health snapshot {path:?}: {source}")]
    ParseHealth {
        path: PathBuf,
        #[source]
        source: serde_json::Error,
    },
    #[error("failed to read pid file {path:?}: {source}")]
    ReadPid {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error("failed to parse pid file {path:?}: {source}")]
    ParsePid {
        path: PathBuf,
        #[source]
        source: std::num::ParseIntError,
    },
    #[error(
        "daemon socket {endpoint} is reachable but pid file {path:?} is missing; inspect the runtime directory before retrying"
    )]
    MissingPidWithSocket { path: PathBuf, endpoint: String },
    #[error("failed to write lifecycle output: {0}")]
    Io(#[source] io::Error),
    #[error("failed to signal daemon pid {pid}: {source}")]
    SignalFailed {
        pid: u32,
        #[source]
        source: io::Error,
    },
    #[error("daemon shutdown did not complete within {timeout_ms} ms; check {pid_path:?}")]
    ShutdownTimeout { pid_path: PathBuf, timeout_ms: u64 },
    #[cfg(not(unix))]
    #[error("platform does not support daemon lifecycle signalling")]
    UnsupportedPlatform,
    #[error(transparent)]
    Paths(#[from] RuntimePathsError),
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::TcpListener;
    use tempfile::TempDir;
    use weaver_config::{Config, SocketEndpoint};

    fn temp_paths() -> (TempDir, RuntimePaths) {
        let dir = TempDir::new().expect("temp dir");
        let socket = dir.path().join("daemon.sock");
        let socket = socket.to_string_lossy().to_string();
        let config = Config {
            daemon_socket: SocketEndpoint::unix(socket),
            ..Config::default()
        };
        let paths = RuntimePaths::from_config(&config).expect("paths");
        (dir, paths)
    }

    #[test]
    fn read_pid_handles_missing_file() {
        let (_dir, paths) = temp_paths();
        assert_eq!(read_pid(paths.pid_path()).unwrap(), None);
    }

    #[test]
    fn read_pid_parses_integer() {
        let (_dir, paths) = temp_paths();
        fs::write(paths.pid_path(), b"42\n").expect("write pid");
        assert_eq!(read_pid(paths.pid_path()).unwrap(), Some(42));
    }

    #[test]
    fn socket_reachability_tracks_tcp_listener() {
        let listener = TcpListener::bind(("127.0.0.1", 0)).expect("bind listener");
        let addr = listener.local_addr().expect("local addr");
        let endpoint = SocketEndpoint::tcp(addr.ip().to_string(), addr.port());
        assert!(socket_is_reachable(&endpoint).expect("probe reachable"));
        drop(listener);
        assert!(!socket_is_reachable(&endpoint).expect("probe available"));
    }

    #[test]
    fn ensure_socket_available_rejects_bound_socket() {
        let listener = TcpListener::bind(("127.0.0.1", 0)).expect("bind listener");
        let addr = listener.local_addr().expect("local addr");
        let endpoint = SocketEndpoint::tcp(addr.ip().to_string(), addr.port());
        let error = ensure_socket_available(&endpoint).expect_err("socket should be reported busy");
        assert!(matches!(error, LifecycleError::SocketInUse { .. }));
        drop(listener);
        ensure_socket_available(&endpoint).expect("socket becomes available");
    }
}
