//! Request builders for `observe graph-slice` end-to-end snapshots.
//!
//! Kept separate from `test_support/mod.rs` so that suites which never issue a
//! `graph-slice` request do not compile (and then have to silence) these
//! helpers.

use crate::test_support::{TestDaemon, Transcript, run_cli};

/// Input parameters for a single `observe graph-slice` CLI invocation in tests.
#[derive(Debug, Clone, Copy)]
pub(crate) struct GraphSliceRequest<'a> {
    pub uri: &'a str,
    pub line: u32,
    pub column: u32,
    pub entry_detail: &'a str,
    pub node_detail: &'a str,
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
