//! Request builders for `observe graph-slice` end-to-end snapshots.
//!
//! Kept separate from `test_support/mod.rs` so that suites which never issue a
//! `graph-slice` request do not compile (and then have to silence) these
//! helpers.

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
pub(crate) fn run_graph_slice(daemon: &TestDaemon, request: GraphSliceRequest<'_>) -> Transcript {
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
