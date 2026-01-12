//! Socket listener implementation for the daemon transport.
//!
//! The listener binds to a configured [`SocketEndpoint`] and runs a background
//! accept loop that hands each connection to a [`ConnectionHandler`]. It tracks
//! the background thread via [`ListenerHandle`], enforces a simple concurrency
//! limit for handler threads, and cleans up Unix socket files during shutdown
//! or early error paths.

use std::io;
#[cfg(test)]
use std::net::SocketAddr;
use std::net::{TcpListener, ToSocketAddrs};
use std::sync::{
    Arc,
    atomic::{AtomicBool, AtomicUsize, Ordering},
};
use std::thread;
use std::time::Duration;

use tracing::{info, warn};

use weaver_config::SocketEndpoint;

use super::{ConnectionHandler, ConnectionStream, LISTENER_TARGET, ListenerError};

#[cfg(unix)]
use std::fs;
#[cfg(unix)]
use std::os::unix::fs::FileTypeExt;
#[cfg(unix)]
use std::os::unix::net::{UnixListener, UnixStream};
#[cfg(unix)]
use std::path::Path;

const ACCEPT_BACKOFF: Duration = Duration::from_millis(25);
const ERROR_BACKOFF: Duration = Duration::from_millis(150);
const MAX_HANDLER_THREADS: usize = 128;
const READ_TIMEOUT: Duration = Duration::from_secs(5);

/// Listener that binds to a socket endpoint and spawns a background accept loop.
#[derive(Debug)]
pub(crate) struct SocketListener {
    endpoint: SocketEndpoint,
    listener: ListenerKind,
}

/// Bound socket variants backed by TCP or Unix transports.
#[derive(Debug)]
enum ListenerKind {
    Tcp(TcpListener),
    #[cfg(unix)]
    Unix(UnixListener),
}

impl SocketListener {
    /// Binds to the provided socket endpoint.
    ///
    /// Returns a listener ready to start an accept loop. Binding can fail due to
    /// address resolution errors, sockets already in use, or Unix socket
    /// filesystem conflicts.
    pub(crate) fn bind(endpoint: &SocketEndpoint) -> Result<Self, ListenerError> {
        match endpoint {
            SocketEndpoint::Tcp { host, port } => {
                let listener = bind_tcp(host, *port)?;
                Ok(Self {
                    endpoint: endpoint.clone(),
                    listener: ListenerKind::Tcp(listener),
                })
            }
            SocketEndpoint::Unix { path } => {
                #[cfg(unix)]
                {
                    let listener = bind_unix(path.as_std_path())?;
                    Ok(Self {
                        endpoint: endpoint.clone(),
                        listener: ListenerKind::Unix(listener),
                    })
                }

                #[cfg(not(unix))]
                {
                    Err(ListenerError::UnsupportedUnix {
                        endpoint: endpoint.to_string(),
                    })
                }
            }
        }
    }

    #[cfg(test)]
    /// Returns the bound address for TCP listeners in tests.
    ///
    /// TCP listeners return `Some(SocketAddr)`, while Unix listeners return
    /// `None` because they are filesystem-backed.
    pub(crate) fn local_addr(&self) -> Option<SocketAddr> {
        match &self.listener {
            ListenerKind::Tcp(listener) => listener.local_addr().ok(),
            #[cfg(unix)]
            ListenerKind::Unix(_) => None,
        }
    }

    /// Starts the accept loop in a background thread and returns its handle.
    ///
    /// The listener switches into non-blocking mode, spawns the accept loop, and
    /// hands accepted connections to the supplied [`ConnectionHandler`]. On Unix
    /// platforms, socket cleanup is attempted if non-blocking configuration
    /// fails. Returns `ListenerError::NonBlocking` when the listener cannot be
    /// configured for non-blocking accepts.
    pub(crate) fn start(
        mut self,
        handler: Arc<dyn ConnectionHandler>,
    ) -> Result<ListenerHandle, ListenerError> {
        let shutdown = Arc::new(AtomicBool::new(false));
        if let Err(error) = match &self.listener {
            ListenerKind::Tcp(listener) => listener.set_nonblocking(true),
            #[cfg(unix)]
            ListenerKind::Unix(listener) => listener.set_nonblocking(true),
        } {
            #[cfg(unix)]
            cleanup_unix_socket(&self.endpoint);
            return Err(ListenerError::NonBlocking { source: error });
        }
        let shutdown_flag = Arc::clone(&shutdown);
        let handle = thread::spawn(move || run_accept_loop(&mut self, shutdown_flag, handler));
        Ok(ListenerHandle {
            shutdown,
            handle: Some(handle),
        })
    }
}

impl Drop for SocketListener {
    fn drop(&mut self) {
        #[cfg(unix)]
        cleanup_unix_socket(&self.endpoint);
    }
}

/// Handle to the background listener thread.
///
/// Use this handle to signal shutdown and wait for the accept loop to stop.
pub(crate) struct ListenerHandle {
    shutdown: Arc<AtomicBool>,
    handle: Option<thread::JoinHandle<()>>,
}

impl ListenerHandle {
    /// Signals the accept loop to shut down.
    pub(crate) fn shutdown(&self) {
        self.shutdown.store(true, Ordering::SeqCst);
    }

    /// Waits for the accept loop to complete.
    ///
    /// Returns an error if the background thread panics.
    pub(crate) fn join(mut self) -> Result<(), ListenerError> {
        if let Some(handle) = self.handle.take() {
            match handle.join() {
                Ok(()) => Ok(()),
                Err(_) => Err(ListenerError::ThreadPanic),
            }
        } else {
            Ok(())
        }
    }
}

impl Drop for ListenerHandle {
    fn drop(&mut self) {
        self.shutdown.store(true, Ordering::SeqCst);
        if let Some(handle) = self.handle.take()
            && handle.join().is_err()
        {
            warn!(
                target: LISTENER_TARGET,
                "listener thread panicked during drop"
            );
        }
    }
}

struct HandlerLimiter {
    active: Arc<AtomicUsize>,
    max: usize,
}

impl HandlerLimiter {
    fn new(max: usize) -> Self {
        Self {
            active: Arc::new(AtomicUsize::new(0)),
            max,
        }
    }

    fn try_acquire(&self) -> Option<HandlerPermit> {
        let mut current = self.active.load(Ordering::SeqCst);
        loop {
            if current >= self.max {
                return None;
            }
            match self.active.compare_exchange(
                current,
                current + 1,
                Ordering::SeqCst,
                Ordering::SeqCst,
            ) {
                Ok(_) => return Some(HandlerPermit::new(Arc::clone(&self.active))),
                Err(next) => current = next,
            }
        }
    }
}

struct HandlerPermit {
    active: Arc<AtomicUsize>,
}

impl HandlerPermit {
    fn new(active: Arc<AtomicUsize>) -> Self {
        Self { active }
    }
}

impl Drop for HandlerPermit {
    fn drop(&mut self) {
        self.active.fetch_sub(1, Ordering::SeqCst);
    }
}

fn run_accept_loop(
    listener: &mut SocketListener,
    shutdown: Arc<AtomicBool>,
    handler: Arc<dyn ConnectionHandler>,
) {
    info!(
        target: LISTENER_TARGET,
        endpoint = %listener.endpoint,
        "socket listener active"
    );
    let mut last_error = None::<io::ErrorKind>;
    let limiter = HandlerLimiter::new(MAX_HANDLER_THREADS);
    while !shutdown.load(Ordering::SeqCst) {
        if let Some(delay) = handle_accept_cycle(listener, &handler, &limiter, &mut last_error) {
            thread::sleep(delay);
        }
    }

    #[cfg(unix)]
    cleanup_unix_socket(&listener.endpoint);
}

fn handle_accept_cycle(
    listener: &mut SocketListener,
    handler: &Arc<dyn ConnectionHandler>,
    limiter: &HandlerLimiter,
    last_error: &mut Option<io::ErrorKind>,
) -> Option<Duration> {
    match accept_connection(listener) {
        Ok(Some(stream)) => {
            *last_error = None;
            if let Some(permit) = limiter.try_acquire() {
                let handler = Arc::clone(handler);
                thread::spawn(move || {
                    let _permit = permit;
                    handler.handle(stream);
                });
            } else {
                warn!(
                    target: LISTENER_TARGET,
                    max_threads = limiter.max,
                    "listener at capacity, dropping connection"
                );
            }
            None
        }
        Ok(None) => Some(ACCEPT_BACKOFF),
        Err(error) => {
            let kind = error.kind();
            if *last_error != Some(kind) {
                warn!(
                    target: LISTENER_TARGET,
                    error = %error,
                    "socket accept error"
                );
            }
            *last_error = Some(kind);
            Some(ERROR_BACKOFF)
        }
    }
}

fn accept_connection(listener: &mut SocketListener) -> Result<Option<ConnectionStream>, io::Error> {
    match &listener.listener {
        ListenerKind::Tcp(tcp) => match tcp.accept() {
            Ok((stream, _)) => {
                stream.set_nonblocking(false)?;
                stream.set_read_timeout(Some(READ_TIMEOUT))?;
                Ok(Some(ConnectionStream::Tcp(stream)))
            }
            Err(error) if error.kind() == io::ErrorKind::WouldBlock => Ok(None),
            Err(error) => Err(error),
        },
        #[cfg(unix)]
        ListenerKind::Unix(unix) => match unix.accept() {
            Ok((stream, _)) => {
                stream.set_nonblocking(false)?;
                stream.set_read_timeout(Some(READ_TIMEOUT))?;
                Ok(Some(ConnectionStream::Unix(stream)))
            }
            Err(error) if error.kind() == io::ErrorKind::WouldBlock => Ok(None),
            Err(error) => Err(error),
        },
    }
}

fn bind_tcp(host: &str, port: u16) -> Result<TcpListener, ListenerError> {
    let mut addrs = (host, port)
        .to_socket_addrs()
        .map_err(|source| ListenerError::Resolve {
            host: host.to_string(),
            port,
            source,
        })?;
    let addr = addrs.next().ok_or_else(|| ListenerError::ResolveEmpty {
        host: host.to_string(),
        port,
    })?;
    TcpListener::bind(addr).map_err(|source| ListenerError::BindTcp { addr, source })
}

#[cfg(unix)]
fn bind_unix(path: &Path) -> Result<UnixListener, ListenerError> {
    if path.exists() {
        let metadata =
            fs::symlink_metadata(path).map_err(|source| ListenerError::UnixMetadata {
                path: path.display().to_string(),
                source,
            })?;
        if !metadata.file_type().is_socket() {
            return Err(ListenerError::UnixNotSocket {
                path: path.display().to_string(),
            });
        }
        match UnixStream::connect(path) {
            Ok(_stream) => {
                return Err(ListenerError::UnixInUse {
                    path: path.display().to_string(),
                });
            }
            Err(error)
                if error.kind() == io::ErrorKind::ConnectionRefused
                    || error.kind() == io::ErrorKind::NotFound =>
            {
                fs::remove_file(path).map_err(|source| ListenerError::UnixCleanup {
                    path: path.display().to_string(),
                    source,
                })?;
            }
            Err(error) => {
                return Err(ListenerError::UnixConnect {
                    path: path.display().to_string(),
                    source: error,
                });
            }
        }
    }

    UnixListener::bind(path).map_err(|source| ListenerError::BindUnix {
        path: path.display().to_string(),
        source,
    })
}

#[cfg(unix)]
fn cleanup_unix_socket(endpoint: &SocketEndpoint) {
    let SocketEndpoint::Unix { path } = endpoint else {
        return;
    };
    if let Err(error) = fs::remove_file(path.as_std_path())
        && error.kind() != io::ErrorKind::NotFound
    {
        warn!(
            target: LISTENER_TARGET,
            error = %error,
            path = %path,
            "failed to remove unix socket file"
        );
    }
}
