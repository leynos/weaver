//! Request builders for `observe get-card` end-to-end snapshots.
//!
//! Kept separate from `test_support/mod.rs` so that suites which never issue a
//! `get-card` request do not compile (and then have to silence) these helpers.

use serde::Serialize;

use crate::test_support::{TestDaemon, Transcript, run_cli};

/// Paired transcripts from a two-request caching sequence.
#[derive(Debug, Serialize)]
pub(crate) struct CacheTranscript {
    pub first: Transcript,
    pub second: Transcript,
    pub cache_hits: u64,
    pub cache_misses: u64,
}

/// Input parameters for a single `observe get-card` CLI invocation in tests.
#[derive(Debug, Clone, Copy)]
pub(crate) struct GetCardRequest<'a> {
    pub uri: &'a str,
    pub line: u32,
    pub column: u32,
    pub detail: &'a str,
}

/// Executes `weaver observe get-card` via the test daemon and returns a `Transcript`.
pub(crate) fn run_get_card(daemon: &TestDaemon, request: GetCardRequest<'_>) -> Transcript {
    let position = format!("{}:{}", request.line, request.column);
    let command = format!(
        "weaver --daemon-socket tcp://<daemon-endpoint> --output json observe get-card --uri \
         <uri> --position {position} --detail {}",
        request.detail
    );
    let cli_args = vec![
        String::from("--daemon-socket"),
        daemon.endpoint(),
        String::from("--output"),
        String::from("json"),
        String::from("observe"),
        String::from("get-card"),
        String::from("--uri"),
        String::from(request.uri),
        String::from("--position"),
        position,
        String::from("--detail"),
        String::from(request.detail),
    ];

    run_cli(command, &cli_args)
}
