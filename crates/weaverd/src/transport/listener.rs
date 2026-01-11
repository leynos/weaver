//! Listener implementation for daemon transport sockets.

use std::io;
use std::net::{SocketAddr, TcpListener, ToSocketAddrs};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
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

/// Listener that binds to a socket endpoint.
#[derive(Debug)]
pub(crate) struct SocketListener {
    endpoint: SocketEndpoint,
    listener: ListenerKind,
}

#[derive(Debug)]
enum ListenerKind {
    Tcp(TcpListener),
    #[cfg(unix)]
    Unix(UnixListener),
}

impl SocketListener {
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
    pub(crate) fn local_addr(&self) -> Option<SocketAddr> {
        match &self.listener {
            ListenerKind::Tcp(listener) => listener.local_addr().ok(),
            #[cfg(unix)]
            ListenerKind::Unix(_) => None,
        }
    }

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

/// Handle to the background listener thread.
pub(crate) struct ListenerHandle {
    shutdown: Arc<AtomicBool>,
    handle: Option<thread::JoinHandle<()>>,
}

impl ListenerHandle {
    pub(crate) fn shutdown(&self) {
        self.shutdown.store(true, Ordering::SeqCst);
    }

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
    while !shutdown.load(Ordering::SeqCst) {
        match accept_connection(listener) {
            Ok(Some(stream)) => {
                last_error = None;
                let handler = Arc::clone(&handler);
                thread::spawn(move || handler.handle(stream));
            }
            Ok(None) => {
                thread::sleep(ACCEPT_BACKOFF);
            }
            Err(error) => {
                let kind = error.kind();
                if last_error != Some(kind) {
                    warn!(
                        target: LISTENER_TARGET,
                        error = %error,
                        "socket accept error"
                    );
                }
                last_error = Some(kind);
                thread::sleep(ERROR_BACKOFF);
            }
        }
    }

    #[cfg(unix)]
    cleanup_unix_socket(&listener.endpoint);
}

fn accept_connection(listener: &mut SocketListener) -> Result<Option<ConnectionStream>, io::Error> {
    match &listener.listener {
        ListenerKind::Tcp(tcp) => match tcp.accept() {
            Ok((stream, _)) => {
                stream.set_nonblocking(false)?;
                Ok(Some(ConnectionStream::Tcp(stream)))
            }
            Err(error) if error.kind() == io::ErrorKind::WouldBlock => Ok(None),
            Err(error) => Err(error),
        },
        #[cfg(unix)]
        ListenerKind::Unix(unix) => match unix.accept() {
            Ok((stream, _)) => {
                stream.set_nonblocking(false)?;
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
    let addr = addrs
        .find(|addr| matches!(addr, SocketAddr::V4(_) | SocketAddr::V6(_)))
        .ok_or_else(|| ListenerError::ResolveEmpty {
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

#[cfg(test)]
mod tests {
    use super::super::NoopConnectionHandler;
    use super::*;
    use std::net::TcpStream;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::time::Instant;

    struct CountingHandler {
        count: Arc<AtomicUsize>,
    }

    impl ConnectionHandler for CountingHandler {
        fn handle(&self, _stream: ConnectionStream) {
            self.count.fetch_add(1, Ordering::SeqCst);
        }
    }

    fn wait_for_count(count: &AtomicUsize, expected: usize) -> bool {
        let deadline = Instant::now() + Duration::from_secs(2);
        while Instant::now() < deadline {
            if count.load(Ordering::SeqCst) >= expected {
                return true;
            }
            thread::sleep(Duration::from_millis(10));
        }
        false
    }

    #[test]
    fn tcp_listener_accepts_connections() {
        let endpoint = SocketEndpoint::tcp("127.0.0.1", 0);
        let listener = SocketListener::bind(&endpoint).expect("bind tcp listener");
        let addr = listener
            .local_addr()
            .expect("listener should report local address");
        let count = Arc::new(AtomicUsize::new(0));
        let handler = Arc::new(CountingHandler {
            count: Arc::clone(&count),
        });
        let handle = listener.start(handler).expect("start listener");

        TcpStream::connect(addr).expect("connect first client");
        TcpStream::connect(addr).expect("connect second client");

        assert!(wait_for_count(&count, 2), "expected two connections");
        handle.shutdown();
        handle.join().expect("join listener");
    }

    #[cfg(unix)]
    #[test]
    fn unix_listener_cleans_stale_socket_files() {
        let dir = tempfile::tempdir().expect("temp dir");
        let path = dir.path().join("weaverd.sock");
        {
            let _stale = UnixListener::bind(&path).expect("bind stale listener");
        }
        assert!(path.exists(), "stale socket should remain");

        let endpoint = SocketEndpoint::unix(path.to_str().expect("utf8 path").to_string());
        let listener = SocketListener::bind(&endpoint).expect("bind new listener");
        let handler = Arc::new(NoopConnectionHandler);
        let handle = listener.start(handler).expect("start listener");

        UnixStream::connect(&path).expect("connect unix client");

        handle.shutdown();
        handle.join().expect("join listener");
        assert!(
            !path.exists(),
            "listener should remove unix socket on shutdown"
        );
    }

    #[cfg(unix)]
    #[test]
    fn unix_listener_rejects_in_use_socket() {
        let dir = tempfile::tempdir().expect("temp dir");
        let path = dir.path().join("weaverd.sock");
        let _existing = UnixListener::bind(&path).expect("bind existing listener");

        let endpoint = SocketEndpoint::unix(path.to_str().expect("utf8 path").to_string());
        let error = SocketListener::bind(&endpoint).expect_err("should fail bind");
        assert!(matches!(error, ListenerError::UnixInUse { .. }));
    }
}
