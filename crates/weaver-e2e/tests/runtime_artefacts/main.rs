//! End-to-end coverage for endpoint-scoped daemon runtime artefacts.
//!
//! The scenario launches the real `weaver` CLI and `weaverd` binary against two
//! non-default Unix endpoints that share one runtime directory. It verifies
//! that each endpoint owns distinct lock, PID, and health files, that the
//! lifecycle commands act only on the configured endpoint, that legacy
//! `weaverd.pid`/`weaverd.health` files are ignored, and that a reachable
//! socket without an endpoint-scoped PID file is never signalled.
#![cfg(unix)]

#[path = "../support/weaver_binary.rs"]
mod weaver_binary;
#[path = "../support/weaverd_binary.rs"]
mod weaverd_binary;

mod artefacts;
mod scenario;
mod shutdown;
mod startup;

use self::{
    artefacts::DaemonGuard,
    scenario::Scenario,
    shutdown::{
        stop_endpoint_a,
        stop_endpoint_b,
        verify_endpoint_a_cleanup,
        verify_endpoint_b_cleanup,
        verify_endpoint_b_survives,
        verify_unverified_process_is_not_signalled,
    },
    startup::{
        install_legacy_decoys,
        start_both_daemons,
        verify_distinct_artefact_names,
        verify_status_ignores_legacy_decoys,
    },
};

#[test]
fn lifecycle_commands_scope_runtime_artefacts_to_the_configured_endpoint() {
    let scenario = Scenario::prepare().expect("scenario should prepare");
    verify_distinct_artefact_names(&scenario).expect("endpoints should own distinct artefacts");

    let mut guard = DaemonGuard::default();
    let (pid_a, pid_b) =
        start_both_daemons(&scenario, &mut guard).expect("both daemons should start");

    install_legacy_decoys(&scenario).expect("legacy decoys should be written");
    verify_status_ignores_legacy_decoys(&scenario, pid_a)
        .expect("status should use endpoint-scoped files");

    stop_endpoint_a(&scenario, pid_a).expect("endpoint A should stop");
    guard.mark_stopped(pid_a);
    verify_endpoint_a_cleanup(&scenario).expect("endpoint A should retain only its lock");
    verify_endpoint_b_survives(&scenario, pid_b).expect("endpoint B should survive");

    verify_unverified_process_is_not_signalled(&scenario, pid_b)
        .expect("stop should refuse without a verified PID");
    stop_endpoint_b(&scenario, pid_b).expect("endpoint B should stop");
    guard.mark_stopped(pid_b);
    verify_endpoint_b_cleanup(&scenario).expect("endpoint B should retain only its lock");
}
