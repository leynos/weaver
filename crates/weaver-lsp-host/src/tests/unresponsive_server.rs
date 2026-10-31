//! Regression tests for a language server that accepts requests but never answers.
//!
//! The daemon used to block on the server's stdout with no deadline, so one
//! wedged `rust-analyzer` stalled a whole `observe graph-slice` request until
//! the caller was killed from outside (a 180-second nextest kill in CI).
//! These tests use a server that reads stdin for ever and writes nothing.
#![cfg(unix)]

use std::{
    error::Error,
    path::PathBuf,
    time::{Duration, Instant},
};

use rstest::{fixture, rstest};

use crate::{
    Language,
    adapter::{AdapterError, LspServerConfig, ProcessLanguageServer},
    server::{LanguageServer, LanguageServerError},
};

/// Short enough to keep the suite quick, long enough that scheduling jitter
/// cannot turn a healthy exchange into a timeout.
const RESPONSE_TIMEOUT: Duration = Duration::from_millis(300);

/// Upper bound on how long a call may take. Far above [`RESPONSE_TIMEOUT`] to
/// tolerate a starved host, and far below any external kill limit.
const PROMPTNESS_BOUND: Duration = Duration::from_secs(20);

#[fixture]
fn silent_server() -> ProcessLanguageServer {
    let config = LspServerConfig {
        command: PathBuf::from("sh"),
        args: vec![String::from("-c"), String::from("cat > /dev/null")],
        working_dir: None,
        response_timeout: RESPONSE_TIMEOUT,
    };
    ProcessLanguageServer::with_config(Language::Rust, config)
}

fn adapter_error(error: &LanguageServerError) -> &AdapterError {
    error
        .source()
        .and_then(|source| source.downcast_ref::<AdapterError>())
        .expect("error should wrap an AdapterError")
}

#[rstest]
fn initialize_gives_up_on_a_silent_server(mut silent_server: ProcessLanguageServer) {
    let started = Instant::now();

    let error = silent_server
        .initialize()
        .expect_err("a server that never answers cannot initialise");

    assert!(
        started.elapsed() < PROMPTNESS_BOUND,
        "initialize should give up near the response timeout, took {:?}",
        started.elapsed()
    );
    assert!(
        adapter_error(&error).is_timeout(),
        "expected a timeout, got {error:?}"
    );
}

#[rstest]
fn an_unresponsive_server_is_not_restarted_straight_away(mut silent_server: ProcessLanguageServer) {
    silent_server
        .initialize()
        .expect_err("first attempt times out");

    let started = Instant::now();
    let error = silent_server
        .initialize()
        .expect_err("second attempt is refused during the cooldown");

    // The second attempt must not spawn a process and wait out another deadline,
    // otherwise a graph slice with N cards would stall for N deadlines.
    assert!(
        started.elapsed() < RESPONSE_TIMEOUT,
        "refusal should be immediate, took {:?}",
        started.elapsed()
    );
    assert!(
        matches!(adapter_error(&error), AdapterError::Unresponsive { .. }),
        "expected Unresponsive, got {error:?}"
    );
}
