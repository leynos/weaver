//! Regression tests for a language server that accepts requests but never answers.
//!
//! The daemon used to block on the server's stdout with no deadline, so one
//! wedged `rust-analyzer` stalled a whole `observe graph-slice` request until
//! the caller was killed from outside (a 180-second nextest kill in CI).
//! These tests use servers that read stdin for ever and either write nothing or
//! keep writing notifications without ever answering.
#![cfg(unix)]

use std::{
    error::Error,
    path::PathBuf,
    sync::Arc,
    time::{Duration, Instant},
};

use crate::{
    Language,
    adapter::{AdapterError, Clock, LspServerConfig, ManualClock, ProcessLanguageServer},
    server::{LanguageServer, LanguageServerError},
};

/// Short enough to keep the suite quick, long enough that scheduling jitter
/// cannot turn a healthy exchange into a timeout.
const RESPONSE_TIMEOUT: Duration = Duration::from_millis(300);

/// Upper bound on how long a call may take: about sixteen times the timeout, so
/// a deadline that fails to hold shows up as a failure, while a starved host
/// still has room to start a shell.
const PROMPTNESS_BOUND: Duration = Duration::from_secs(5);

/// Reads stdin for ever and writes nothing.
const SILENT_SERVER: &str = "cat > /dev/null";

/// Writes a valid notification every 50 ms and never answers a request, so a
/// deadline that restarted on each message would never fire.
const CHATTY_SERVER: &str = r#"body='{"jsonrpc":"2.0","method":"window/logMessage"}'
while :; do
  printf 'Content-Length: %s\r\n\r\n%s' "${#body}" "$body"
  sleep 0.05
done"#;

fn server_running(script: &str, response_timeout: Duration) -> ProcessLanguageServer {
    let config = LspServerConfig {
        command: PathBuf::from("sh"),
        args: vec![String::from("-c"), String::from(script)],
        working_dir: None,
        response_timeout,
    };
    ProcessLanguageServer::with_config(Language::Rust, config)
}

fn adapter_error(error: &LanguageServerError) -> &AdapterError {
    error
        .source()
        .and_then(|source| source.downcast_ref::<AdapterError>())
        .expect("error should wrap an AdapterError")
}

fn assert_unresponsive(error: &LanguageServerError) {
    assert!(
        matches!(adapter_error(error), AdapterError::Unresponsive { .. }),
        "expected Unresponsive, got {error:?}"
    );
}

#[test]
fn initialize_gives_up_on_a_silent_server() {
    let mut server = server_running(SILENT_SERVER, RESPONSE_TIMEOUT);
    let started = Instant::now();

    let error = server
        .initialize()
        .expect_err("a server that never answers cannot initialise");

    assert!(
        started.elapsed() >= RESPONSE_TIMEOUT && started.elapsed() < PROMPTNESS_BOUND,
        "initialize should give up near the response timeout, took {:?}",
        started.elapsed()
    );
    assert!(
        adapter_error(&error).is_timeout(),
        "expected a timeout, got {error:?}"
    );
}

#[test]
fn a_server_that_keeps_chattering_still_hits_one_overall_deadline() {
    let mut server = server_running(CHATTY_SERVER, RESPONSE_TIMEOUT);
    let started = Instant::now();

    let error = server
        .initialize()
        .expect_err("notifications are not an answer");

    assert!(
        started.elapsed() >= RESPONSE_TIMEOUT && started.elapsed() < PROMPTNESS_BOUND,
        "each notification must not restart the wait, took {:?}",
        started.elapsed()
    );
    assert!(
        adapter_error(&error).is_timeout(),
        "expected a timeout, got {error:?}"
    );
    assert_unresponsive(
        &server
            .initialize()
            .expect_err("the cooldown follows the timeout"),
    );
}

#[test]
fn the_restart_cooldown_refuses_then_expires_on_the_injected_clock() {
    let clock = Arc::new(ManualClock::new());
    let mut server = server_running(SILENT_SERVER, Duration::from_millis(100))
        .with_clock(Arc::clone(&clock) as Arc<dyn Clock>);
    server
        .initialize()
        .expect_err("the first attempt times out");

    // Inside the 60-second cooldown the adapter refuses without spawning.
    assert_unresponsive(&server.initialize().expect_err("refused at once"));
    clock.advance(Duration::from_secs(59));
    assert_unresponsive(&server.initialize().expect_err("still refused at 59s"));

    // Once the full period has passed it tries again, and the silent server
    // times out afresh rather than being refused.
    clock.advance(Duration::from_secs(1));
    let error = server
        .initialize()
        .expect_err("the retry meets the same silent server");
    assert!(
        adapter_error(&error).is_timeout(),
        "expected a fresh timeout after the cooldown, got {error:?}"
    );
}

#[test]
fn a_timeout_too_large_for_the_clock_is_rejected_not_a_panic() {
    let mut server = server_running(SILENT_SERVER, Duration::MAX);

    let error = server
        .initialize()
        .expect_err("an unrepresentable deadline cannot be waited for");

    assert!(
        matches!(
            adapter_error(&error),
            AdapterError::InvalidResponseTimeout { .. }
        ),
        "expected InvalidResponseTimeout, got {error:?}"
    );
}
