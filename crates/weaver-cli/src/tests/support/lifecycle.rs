//! Lifecycle test utilities and fixtures.
//!
//! Provides mocks for lifecycle operations and fixtures for creating temporary
//! runtime directories with health snapshots for daemon monitoring tests.

use std::cell::RefCell;
use std::collections::VecDeque;
use std::fs;
use std::io::Write;
use std::path::Path;
use std::process::ExitCode;

use rstest::fixture;
use tempfile::TempDir;
use weaver_config::{Config, RuntimePaths, SocketEndpoint};

use crate::lifecycle::{
    LifecycleCommand, LifecycleContext, LifecycleError, LifecycleInvocation, LifecycleOutput,
};

/// Captures lifecycle invocations and replays queued results for behavioural tests.
#[derive(Default)]
pub(in crate::tests) struct TestLifecycle {
    calls: RefCell<Vec<LifecycleCall>>,
    responses: RefCell<VecDeque<Result<ExitCode, LifecycleError>>>,
}

/// A recorded lifecycle command invocation.
#[derive(Debug, Clone)]
pub(in crate::tests) struct LifecycleCall {
    pub command: LifecycleCommand,
}

impl TestLifecycle {
    /// Returns all recorded lifecycle calls.
    pub fn record(&self) -> Vec<LifecycleCall> {
        self.calls.borrow().clone()
    }

    /// Enqueues a result to be returned by the next `handle` call.
    pub fn enqueue(&self, result: Result<ExitCode, LifecycleError>) {
        self.responses.borrow_mut().push_back(result);
    }

    /// Handles a lifecycle invocation, recording it and returning a queued result.
    pub fn handle<W: Write, E: Write>(
        &self,
        invocation: LifecycleInvocation,
        _context: LifecycleContext<'_>,
        _output: &mut LifecycleOutput<W, E>,
    ) -> Result<ExitCode, LifecycleError> {
        let call = LifecycleCall {
            command: invocation.command,
        };
        self.calls.borrow_mut().push(call);
        self.responses
            .borrow_mut()
            .pop_front()
            .unwrap_or(Ok(ExitCode::SUCCESS))
    }
}

// ── Fixtures ───────────────────────────────────────────────────────────────────

/// Creates a temporary directory with runtime paths configured for a Unix socket.
///
/// Returns both the `TempDir` (which must be kept alive to preserve the directory)
/// and the derived `RuntimePaths`.
#[fixture]
pub(crate) fn temp_paths() -> (TempDir, RuntimePaths) {
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

// ── Health snapshot utilities ──────────────────────────────────────────────────

/// Writes a health snapshot JSON file to the specified path.
///
/// Used by lifecycle tests to simulate daemon health state. This helper
/// constructs the JSON using `serde_json::json!` to ensure proper escaping.
pub(crate) fn write_health_json(path: &Path, status: &str, pid: u32, timestamp: u64) {
    let snapshot = serde_json::json!({
        "status": status,
        "pid": pid,
        "timestamp": timestamp
    });
    let json = serde_json::to_string(&snapshot).expect("serialize health snapshot");
    fs::write(path, json).expect("write health snapshot");
}

/// Convenience wrapper that writes a health snapshot to the standard health path.
pub(crate) fn write_health_snapshot(paths: &RuntimePaths, status: &str, pid: u32, timestamp: u64) {
    write_health_json(paths.health_path(), status, pid, timestamp);
}
