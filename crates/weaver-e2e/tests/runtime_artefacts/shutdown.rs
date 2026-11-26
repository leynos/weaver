//! Scenario steps that verify endpoint-scoped shutdown and refusal to signal.

use std::path::Path;

use super::{
    artefacts::{
        LEGACY_HEALTH_FILE,
        LEGACY_PID_FILE,
        ScenarioResult,
        expect_success,
        process_is_alive,
        require_absent,
        require_present,
        require_text,
        write_file,
    },
    scenario::{Scenario, Side},
};

/// Stops endpoint A and verifies its reported PID and retained lock.
pub(super) fn stop_endpoint_a(scenario: &Scenario, pid_a: u32) -> ScenarioResult<()> {
    let output = scenario.run(Side::A, &["daemon", "stop"])?;
    let (stdout, stderr) = expect_success(&output, "daemon stop for endpoint A")?;
    require_text(
        &stdout,
        &format!("daemon pid {pid_a} stopped cleanly"),
        "daemon stop stdout",
    )?;
    let expected = format!(
        "retained persistent runtime lock at {}",
        scenario.paths(Side::A).lock_path().display()
    );
    require_text(&stderr, &expected, "daemon stop stderr")
}

/// Verifies endpoint A removed its PID and health files but kept its lock.
pub(super) fn verify_endpoint_a_cleanup(scenario: &Scenario) -> ScenarioResult<()> {
    let paths = scenario.paths(Side::A);
    require_absent(scenario.dir(), paths.pid_file_name(), "endpoint A PID file")?;
    require_absent(
        scenario.dir(),
        paths.health_file_name(),
        "endpoint A health file",
    )?;
    require_present(
        scenario.dir(),
        paths.lock_file_name(),
        "endpoint A lock file",
    )?;
    require_present(
        scenario.dir(),
        Path::new(LEGACY_PID_FILE),
        "legacy decoy PID file",
    )?;
    require_present(
        scenario.dir(),
        Path::new(LEGACY_HEALTH_FILE),
        "legacy decoy health file",
    )
}

/// Verifies endpoint B survives endpoint A shutting down.
pub(super) fn verify_endpoint_b_survives(scenario: &Scenario, pid_b: u32) -> ScenarioResult<()> {
    if !process_is_alive(pid_b)? {
        return Err(format!("endpoint B daemon {pid_b} should still be running"));
    }
    let (stdout, _stderr) = scenario.status(Side::B)?;
    let expected = format!(
        "daemon status: ready (pid {pid_b}) via {}",
        scenario.endpoint(Side::B)
    );
    require_text(&stdout, &expected, "daemon status stdout")
}

/// Verifies `stop` refuses a reachable socket with no endpoint-scoped PID.
pub(super) fn verify_unverified_process_is_not_signalled(
    scenario: &Scenario,
    pid_b: u32,
) -> ScenarioResult<()> {
    let pid_file = scenario.paths(Side::B).pid_file_name();
    scenario
        .dir()
        .remove_file(pid_file)
        .map_err(|source| format!("remove endpoint B PID file: {source}"))?;

    let output = scenario.run(Side::B, &["daemon", "stop"])?;
    if output.status.success() {
        return Err(String::from(
            "daemon stop should refuse to signal a process without an endpoint-scoped PID file",
        ));
    }
    let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
    require_text(
        &stderr,
        "is reachable but endpoint-scoped PID file",
        "refusal stderr",
    )?;
    require_text(
        &stderr,
        "refusing to signal an unverified process",
        "refusal stderr",
    )?;
    if !process_is_alive(pid_b)? {
        return Err(format!(
            "endpoint B daemon {pid_b} should survive the refused signal"
        ));
    }
    write_file(scenario.dir(), pid_file, &format!("{pid_b}\n"))
}

/// Stops endpoint B after its PID file has been restored.
pub(super) fn stop_endpoint_b(scenario: &Scenario, pid_b: u32) -> ScenarioResult<()> {
    let output = scenario.run(Side::B, &["daemon", "stop"])?;
    let (stdout, stderr) = expect_success(&output, "daemon stop for endpoint B")?;
    require_text(
        &stdout,
        &format!("daemon pid {pid_b} stopped cleanly"),
        "daemon stop stdout",
    )?;
    let expected = format!(
        "retained persistent runtime lock at {}",
        scenario.paths(Side::B).lock_path().display()
    );
    require_text(&stderr, &expected, "daemon stop stderr")
}

/// Verifies endpoint B removed its PID and health files but kept its lock.
pub(super) fn verify_endpoint_b_cleanup(scenario: &Scenario) -> ScenarioResult<()> {
    let paths = scenario.paths(Side::B);
    require_absent(scenario.dir(), paths.pid_file_name(), "endpoint B PID file")?;
    require_absent(
        scenario.dir(),
        paths.health_file_name(),
        "endpoint B health file",
    )?;
    require_present(
        scenario.dir(),
        paths.lock_file_name(),
        "endpoint B lock file",
    )?;
    require_present(
        scenario.dir(),
        Path::new(LEGACY_PID_FILE),
        "legacy decoy PID file",
    )
}
