//! Daemon lifecycle orchestration utilities.
//!
//! Provides low-level helpers for preparing runtime directories, spawning the
//! daemon, monitoring health snapshots, and coordinating shutdown sequences.

use std::env;
use std::ffi::OsString;
use std::fs;
use std::io::{self, Write};
use std::net::{SocketAddr, TcpStream, ToSocketAddrs};
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use socket2::{Domain, SockAddr, Socket, Type};
use weaver_config::{RuntimePaths, SocketEndpoint};

use super::LifecycleOutput;
use super::error::LifecycleError;
use super::types::{LifecycleContext, LifecycleInvocation};

#[cfg(unix)]
use libc::{SIGTERM, kill};

const STARTUP_TIMEOUT: Duration = Duration::from_secs(10);
const SHUTDOWN_TIMEOUT: Duration = Duration::from_secs(10);
const POLL_INTERVAL: Duration = Duration::from_millis(200);
const SOCKET_PROBE_TIMEOUT: Duration = Duration::from_secs(5);

pub(super) fn ensure_no_extra_arguments(
    invocation: &LifecycleInvocation,
) -> Result<(), LifecycleError> {
    if let Some(argument) = invocation.arguments.first() {
        return Err(LifecycleError::UnexpectedArgument {
            command: invocation.command,
            argument: argument.clone(),
        });
    }
    Ok(())
}

pub(super) fn prepare_runtime(
    context: LifecycleContext<'_>,
) -> Result<RuntimePaths, LifecycleError> {
    let config = context.config;
    config.daemon_socket().prepare_filesystem()?;
    RuntimePaths::from_config(config).map_err(LifecycleError::from)
}

pub(super) fn spawn_daemon(config_arguments: &[OsString]) -> Result<Child, LifecycleError> {
    let binary = daemon_binary();
    let mut command = Command::new(&binary);
    if config_arguments.len() > 1 {
        // Skip argv[0], which is the binary name, and forward the remaining CLI
        // arguments verbatim to the daemon.
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
    env::var_os("WEAVERD_BIN").unwrap_or_else(|| OsString::from("weaverd"))
}

pub(super) fn wait_for_ready(
    paths: &RuntimePaths,
    child: &mut Child,
    started_at: SystemTime,
) -> Result<HealthSnapshot, LifecycleError> {
    let deadline = Instant::now() + STARTUP_TIMEOUT;
    let expected_pid = child.id();
    while Instant::now() < deadline {
        if let Some(snapshot) = read_health(paths.health_path())? {
            if !snapshot_matches_process(&snapshot, expected_pid)
                || !snapshot_is_recent(&snapshot, started_at)
            {
                thread::sleep(POLL_INTERVAL);
                continue;
            }
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

pub(super) fn wait_for_shutdown(
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

pub(super) fn read_health(path: &Path) -> Result<Option<HealthSnapshot>, LifecycleError> {
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

pub(super) fn read_pid(path: &Path) -> Result<Option<u32>, LifecycleError> {
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

pub(super) fn ensure_socket_available(endpoint: &SocketEndpoint) -> Result<(), LifecycleError> {
    if socket_is_reachable(endpoint)? {
        return Err(LifecycleError::SocketInUse {
            endpoint: endpoint.to_string(),
        });
    }
    Ok(())
}

pub(super) fn socket_is_reachable(endpoint: &SocketEndpoint) -> Result<bool, LifecycleError> {
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
        .next()
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

pub(super) fn signal_daemon(pid: u32) -> Result<(), LifecycleError> {
    #[cfg(unix)]
    {
        // SAFETY: `kill(2)` is memory-safe even when the PID is invalid; the
        // kernel simply returns an error. We only translate the integer and use
        // the standard SIGTERM signal.
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

#[derive(Debug, serde::Deserialize, PartialEq, Eq)]
pub(crate) struct HealthSnapshot {
    pub status: String,
    pub pid: u32,
    pub timestamp: u64,
}

fn snapshot_matches_process(snapshot: &HealthSnapshot, expected_pid: u32) -> bool {
    snapshot.pid == expected_pid
}

fn snapshot_is_recent(snapshot: &HealthSnapshot, started_at: SystemTime) -> bool {
    match UNIX_EPOCH.checked_add(Duration::from_secs(snapshot.timestamp)) {
        Some(snapshot_time) => snapshot_time >= started_at,
        None => false,
    }
}

pub(super) fn write_startup_banner<W: Write, E: Write>(
    output: &mut LifecycleOutput<W, E>,
    context: LifecycleContext<'_>,
    snapshot: &HealthSnapshot,
    paths: &RuntimePaths,
) -> Result<(), LifecycleError> {
    output.stdout_line(format_args!(
        "daemon ready (pid {}) on {}",
        snapshot.pid,
        context.config.daemon_socket()
    ))?;
    output.stderr_line(format_args!(
        "runtime artefacts stored under {}",
        paths.runtime_dir().display()
    ))
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use std::net::TcpListener;
    use tempfile::TempDir;
    use weaver_config::SocketEndpoint;

    fn temp_paths() -> (TempDir, RuntimePaths) {
        let dir = TempDir::new().expect("temp dir");
        let socket = dir.path().join("daemon.sock");
        let socket = socket.to_string_lossy().to_string();
        let config = weaver_config::Config {
            daemon_socket: SocketEndpoint::unix(socket),
            ..weaver_config::Config::default()
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

    #[test]
    fn snapshot_validation_requires_matching_pid() {
        let snapshot = HealthSnapshot {
            status: String::from("ready"),
            pid: 42,
            timestamp: 0,
        };
        assert!(snapshot_matches_process(&snapshot, 42));
        assert!(!snapshot_matches_process(&snapshot, 1));
    }

    #[test]
    fn snapshot_validation_requires_recent_timestamp() {
        let snapshot = HealthSnapshot {
            status: String::from("ready"),
            pid: 1,
            timestamp: 10,
        };
        let start = UNIX_EPOCH + Duration::from_secs(20);
        assert!(!snapshot_is_recent(&snapshot, start));
        let start = UNIX_EPOCH + Duration::from_secs(5);
        assert!(snapshot_is_recent(&snapshot, start));
    }
}
