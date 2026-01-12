//! Behavioural tests for the daemon socket listener.

use std::cell::RefCell;
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};
use std::time::{Duration, Instant};

use rstest::fixture;
use rstest_bdd_macros::{given, scenario, then, when};

use weaver_config::SocketEndpoint;

use crate::transport::{CountingHandler, ListenerHandle, SocketListener};

struct ListenerWorld {
    endpoint: SocketEndpoint,
    listener: Option<ListenerHandle>,
    accepted: Arc<AtomicUsize>,
    address: Option<SocketAddr>,
    bind_error: Option<String>,
    reserved: Option<TcpListener>,
}

impl ListenerWorld {
    fn new() -> Self {
        Self {
            endpoint: SocketEndpoint::tcp("127.0.0.1", 0),
            listener: None,
            accepted: Arc::new(AtomicUsize::new(0)),
            address: None,
            bind_error: None,
            reserved: None,
        }
    }

    fn start_listener(&mut self) {
        let (count, handler) = CountingHandler::new();
        self.accepted = Arc::clone(&count);
        match SocketListener::bind(&self.endpoint) {
            Ok(listener) => {
                self.address = listener.local_addr();
                match listener.start(handler) {
                    Ok(handle) => {
                        self.listener = Some(handle);
                    }
                    Err(error) => {
                        self.bind_error = Some(error.to_string());
                    }
                }
            }
            Err(error) => {
                self.bind_error = Some(error.to_string());
            }
        }
    }

    fn reserve_port(&mut self) {
        let listener = TcpListener::bind(("127.0.0.1", 0)).expect("bind reserved port");
        let port = listener.local_addr().expect("local addr").port();
        self.endpoint = SocketEndpoint::tcp("127.0.0.1", port);
        self.reserved = Some(listener);
    }

    fn connect_clients(&self, count: usize) {
        let addr = self.address.expect("listener address should be set");
        for _ in 0..count {
            TcpStream::connect(addr).expect("connect client");
        }
    }

    fn wait_for_connections(&self, expected: usize) -> bool {
        let deadline = Instant::now() + Duration::from_secs(2);
        while Instant::now() < deadline {
            if self.accepted.load(Ordering::SeqCst) >= expected {
                return true;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        false
    }
}

impl Drop for ListenerWorld {
    fn drop(&mut self) {
        if let Some(handle) = self.listener.take() {
            handle.shutdown();
            let _ = handle.join();
        }
        self.reserved = None;
    }
}

#[fixture]
fn world() -> RefCell<ListenerWorld> {
    RefCell::new(ListenerWorld::new())
}

#[given("a TCP socket listener is running")]
fn given_tcp_listener(world: &RefCell<ListenerWorld>) {
    world.borrow_mut().start_listener();
    assert!(
        world.borrow().bind_error.is_none(),
        "listener start failed: {:?}",
        world.borrow().bind_error
    );
}

#[given("a TCP socket is already bound")]
fn given_tcp_in_use(world: &RefCell<ListenerWorld>) {
    world.borrow_mut().reserve_port();
}

#[when("a client connects")]
fn when_client_connects(world: &RefCell<ListenerWorld>) {
    world.borrow().connect_clients(1);
}

#[when("two clients connect")]
fn when_two_clients_connect(world: &RefCell<ListenerWorld>) {
    world.borrow().connect_clients(2);
}

#[when("the listener starts on the same socket")]
fn when_listener_starts_same_socket(world: &RefCell<ListenerWorld>) {
    world.borrow_mut().start_listener();
}

#[then("the listener records {count} connections")]
fn then_listener_records_plural(world: &RefCell<ListenerWorld>, count: usize) {
    assert_listener_records(world, count);
}

#[then("the listener records {count} connection")]
fn then_listener_records_singular(world: &RefCell<ListenerWorld>, count: usize) {
    assert_listener_records(world, count);
}

fn assert_listener_records(world: &RefCell<ListenerWorld>, count: usize) {
    assert!(
        world.borrow().wait_for_connections(count),
        "expected {count} connections, got {}",
        world.borrow().accepted.load(Ordering::SeqCst)
    );
}

#[then("starting the listener fails")]
fn then_listener_fails(world: &RefCell<ListenerWorld>) {
    assert!(
        world.borrow().bind_error.is_some(),
        "expected listener start to fail"
    );
}

#[scenario(path = "tests/features/daemon_socket.feature")]
fn daemon_socket_listener(#[from(world)] world: RefCell<ListenerWorld>) {
    drop(world);
}
