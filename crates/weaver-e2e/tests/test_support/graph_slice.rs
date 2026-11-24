//! Request builders for `observe graph-slice` end-to-end snapshots.
//!
//! Kept separate from `test_support/mod.rs` so that suites which never issue a
//! `graph-slice` request do not compile (and then have to silence) these
//! helpers. The payload assertions the snapshot suite shares live here too,
//! which keeps `graph_slice_snapshots.rs` inside the module line budget.

use crate::test_support::{TestDaemon, Transcript, run_cli};

/// Input parameters for a single `observe graph-slice` CLI invocation in tests.
#[derive(Debug, Clone, Copy)]
pub(crate) struct GraphSliceRequest<'a> {
    /// `file://` Uniform Resource Identifier of the source file to slice.
    pub uri: &'a str,
    /// 1-indexed source line of the entry position (`--position` line half).
    pub line: u32,
    /// 1-indexed source column of the entry position (`--position` column half).
    pub column: u32,
    /// `--entry-detail` value: `"structure"` or `"semantic"`.
    pub entry_detail: &'a str,
    /// `--node-detail` value: `"structure"` or `"semantic"`.
    pub node_detail: &'a str,
    /// Optional `--max-cards` budget; omitted when `None`.
    pub max_cards: Option<u32>,
}

/// Executes `weaver observe graph-slice` via the test daemon and returns a `Transcript`.
///
/// # Errors
/// Returns a description if the CLI binary cannot be located or the invocation
/// cannot be run to completion.
pub(crate) fn run_graph_slice(
    daemon: &TestDaemon,
    request: GraphSliceRequest<'_>,
) -> Result<Transcript, String> {
    let position = format!("{}:{}", request.line, request.column);
    let endpoint = daemon.endpoint();
    let mut cli_args = vec![
        String::from("--daemon-socket"),
        endpoint.clone(),
        String::from("--output"),
        String::from("json"),
        String::from("observe"),
        String::from("graph-slice"),
        String::from("--uri"),
        String::from(request.uri),
        String::from("--position"),
        position,
        String::from("--entry-detail"),
        String::from(request.entry_detail),
        String::from("--node-detail"),
        String::from(request.node_detail),
    ];
    if let Some(max_cards) = request.max_cards {
        cli_args.push(String::from("--max-cards"));
        cli_args.push(max_cards.to_string());
    }

    let command = display_command(&cli_args, &endpoint, request.uri);
    run_cli(command, &cli_args)
}

/// Renders `cli_args` as the sanitised `command` string recorded in snapshots.
///
/// The ephemeral daemon endpoint and the temporary fixture URI are the only
/// run-varying arguments, so each is swapped for a stable placeholder. Deriving
/// the display form from the real arguments keeps the two in step by
/// construction.
fn display_command(cli_args: &[String], endpoint: &str, uri: &str) -> String {
    let mut command = String::from("weaver");
    for argument in cli_args {
        command.push(' ');
        if argument == endpoint {
            command.push_str("tcp://<daemon-endpoint>");
        } else if argument == uri {
            command.push_str("<uri>");
        } else {
            command.push_str(argument);
        }
    }
    command
}

/// Parses the CLI's stdout as a JSON payload.
///
/// # Errors
/// Returns a description if stdout is empty or is not valid JSON.
fn parse_stdout(stdout: &str) -> Result<serde_json::Value, String> {
    if stdout.is_empty() {
        return Err(String::from("transcript stdout should not be empty"));
    }
    serde_json::from_str(stdout)
        .map_err(|error| format!("transcript stdout should be valid JSON: {error}"))
}

/// Asserts the payload declares the `graph_slice.v1` schema version.
fn assert_schema_version(value: &serde_json::Value, context: &str) {
    assert_eq!(
        value.pointer("/schema_version"),
        Some(&serde_json::json!("graph_slice.v1")),
        "{context} schema_version should be graph_slice.v1"
    );
}

/// Returns the process exit status the payload's own status and refusal
/// reason imply, mirroring the CLI's documented exit-code contract.
fn expected_exit_status(value: &serde_json::Value) -> i32 {
    match value.get("status").and_then(serde_json::Value::as_str) {
        Some("success") => 0,
        Some("refusal") => match value
            .pointer("/refusal/reason")
            .and_then(serde_json::Value::as_str)
        {
            Some("unsupported_language") => 10,
            Some("no_symbol_at_position") => 11,
            Some("position_out_of_range") => 12,
            Some("not_yet_implemented") => 13,
            Some("backend_unavailable") => 14,
            Some(_) | None => 15,
        },
        Some(_) | None => 15,
    }
}

/// Asserts the observed exit status agrees with the payload it accompanied.
fn assert_exit_status(actual: i32, value: &serde_json::Value, context: &str) {
    assert_eq!(
        actual,
        expected_exit_status(value),
        "{context} exit status should match payload"
    );
}

/// Parses the CLI payload and asserts the envelope invariants every
/// graph-slice response shares, returning the payload for further checks.
///
/// # Errors
/// Returns a description if the transcript's stdout is empty or is not valid
/// JSON. Violated envelope invariants remain assertion failures.
pub(crate) fn assert_graph_slice_envelope(
    transcript: &Transcript,
    context: &str,
) -> Result<serde_json::Value, String> {
    let value = parse_stdout(&transcript.stdout)?;
    assert_schema_version(&value, context);
    assert_exit_status(transcript.status, &value, context);
    Ok(value)
}

/// Reports whether a graph-slice payload describes a successful slice.
pub(crate) fn is_success(value: &serde_json::Value) -> bool {
    value.get("status") == Some(&serde_json::json!("success"))
}

/// Asserts a payload is a refusal carrying `expected_reason`.
pub(crate) fn assert_refusal(value: &serde_json::Value, expected_reason: &str, context: &str) {
    assert_eq!(
        value.get("status"),
        Some(&serde_json::json!("refusal")),
        "{context} should return success or refusal"
    );
    assert_eq!(
        value.pointer("/refusal/reason"),
        Some(&serde_json::json!(expected_reason)),
        "{context} refusal reason should match the snapshot contract"
    );
}

/// Asserts an untruncated successful slice carries a budget, cards, and an
/// empty edge set.
pub(crate) fn assert_populated_slice(value: &serde_json::Value, context: &str) {
    assert!(
        value
            .pointer("/constraints/budget/max_cards")
            .and_then(serde_json::Value::as_u64)
            .unwrap_or(0)
            > 0,
        "{context} budget.max_cards should be positive"
    );
    assert_eq!(
        value.get("edges"),
        Some(&serde_json::json!([])),
        "{context} edges should be empty"
    );
    assert!(
        value
            .get("spillover")
            .is_some_and(serde_json::Value::is_object),
        "{context} spillover should be present"
    );
    assert!(
        value
            .get("cards")
            .and_then(serde_json::Value::as_array)
            .is_some_and(|cards| !cards.is_empty()),
        "{context} cards should be non-empty"
    );
}

/// Asserts a successful slice honoured `--max-cards 1` and flagged spillover.
pub(crate) fn assert_truncated_to_single_card(value: &serde_json::Value, context: &str) {
    assert_eq!(
        value
            .get("cards")
            .and_then(serde_json::Value::as_array)
            .map_or(0, Vec::len),
        1,
        "{context} should contain exactly 1 card (max_cards=1)"
    );
    assert_eq!(
        value
            .pointer("/spillover/truncated")
            .and_then(serde_json::Value::as_bool),
        Some(true),
        "{context} spillover should be truncated"
    );
}
