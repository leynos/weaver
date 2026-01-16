//! Tests for the socket listener.

use std::net::TcpStream;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};

use rstest::{fixture, rstest};

use weaver_config::SocketEndpoint;

use super::listener::SocketListener;
use super::{ConnectionHandler, CountingHandler, ListenerError};

#[derive(Clone)]
struct CountingFixture {
    count: Arc<AtomicUsize>,
    handler: Arc<CountingHandler>,
}

#[fixture]
fn counting_fixture() -> CountingFixture {
    let (count, handler) = CountingHandler::new();
    CountingFixture { count, handler }
}

#[fixture]
fn tcp_endpoint() -> SocketEndpoint {
    SocketEndpoint::tcp("127.0.0.1", 0)
}

fn wait_for_count(count: &AtomicUsize, expected: usize) -> bool {
    let deadline = Instant::now() + Duration::from_secs(2);
    while Instant::now() < deadline {
        if count.load(Ordering::SeqCst) >= expected {
            return true;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    false
}

#[rstest]
fn tcp_listener_accepts_connections(
    tcp_endpoint: SocketEndpoint,
    counting_fixture: CountingFixture,
) {
    let listener = SocketListener::bind(&tcp_endpoint).expect("bind tcp listener");
    let addr = listener
        .local_addr()
        .expect("listener should report local address");
    let CountingFixture { count, handler } = counting_fixture;
    let handler: Arc<dyn ConnectionHandler> = handler;
    let handle = listener.start(handler).expect("start listener");

    TcpStream::connect(addr).expect("connect first client");
    TcpStream::connect(addr).expect("connect second client");

    assert!(wait_for_count(&count, 2), "expected two connections");
    handle.shutdown();
    handle.join().expect("join listener");
}

#[cfg(unix)]
#[fixture]
fn unix_tempdir() -> tempfile::TempDir {
    tempfile::tempdir().expect("temp dir")
}

#[cfg(unix)]
#[rstest]
fn unix_listener_cleans_stale_socket_files(unix_tempdir: tempfile::TempDir) {
    let path = unix_tempdir.path().join("weaverd.sock");
    {
        let _stale = std::os::unix::net::UnixListener::bind(&path).expect("bind stale listener");
    }
    assert!(path.exists(), "stale socket should remain");

    let endpoint = SocketEndpoint::unix(path.to_str().expect("utf8 path").to_string());
    let listener = SocketListener::bind(&endpoint).expect("bind new listener");
    let (_, handler) = CountingHandler::new();
    let handle = listener.start(handler).expect("start listener");

    std::os::unix::net::UnixStream::connect(&path).expect("connect unix client");

    handle.shutdown();
    handle.join().expect("join listener");
    assert!(
        !path.exists(),
        "listener should remove unix socket on shutdown"
    );
}

#[cfg(unix)]
#[rstest]
fn unix_listener_rejects_in_use_socket(unix_tempdir: tempfile::TempDir) {
    let path = unix_tempdir.path().join("weaverd.sock");
    let _existing = std::os::unix::net::UnixListener::bind(&path).expect("bind existing listener");

    let endpoint = SocketEndpoint::unix(path.to_str().expect("utf8 path").to_string());
    let error = SocketListener::bind(&endpoint).expect_err("should fail bind");
    assert!(matches!(error, ListenerError::UnixInUse { .. }));
}
