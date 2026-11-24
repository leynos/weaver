//! Lifecycle coverage for the E2E LSP client child process.

use std::process::Command;

use super::LspClient;

/// Asserts that the operating system no longer reports `pid` as a live process.
fn process_is_reaped(pid: u32) -> bool {
    Command::new("sh")
        .args(["-c", &format!("kill -0 {pid} 2>/dev/null")])
        .status()
        .is_ok_and(|status| !status.success())
}

/// A shutdown request that loses its response still terminates and reaps the
/// child rather than leaving it alive for the test process to exit.
#[test]
fn failed_shutdown_reaps_the_lsp_child() {
    let mut client = LspClient::spawn(
        "sh",
        &["-c", "read _; exec 1>&-; while :; do sleep 1; done"],
    )
    .expect("test server should start");
    let child_pid = client.child.id();

    assert!(
        client.shutdown().is_err(),
        "server closes stdout before responding"
    );
    assert!(
        process_is_reaped(child_pid),
        "LSP child {child_pid} should have been reaped"
    );
}
