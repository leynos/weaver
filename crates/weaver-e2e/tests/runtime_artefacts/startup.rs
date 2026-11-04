//! Scenario steps that verify daemon startup and legacy-file isolation.

use std::{
    path::Path,
    time::{SystemTime, UNIX_EPOCH},
};

use super::{
    artefacts::{
        DECOY_PID,
        DaemonGuard,
        LEGACY_HEALTH_FILE,
        LEGACY_LOCK_FILE,
        LEGACY_PID_FILE,
        ScenarioResult,
        read_file,
        read_pid,
        require_absent,
        require_present,
        require_text,
        write_file,
    },
    scenario::{Scenario, Side},
};

/// Verifies both endpoints share the runtime directory but not file names.
pub(super) fn verify_distinct_artefact_names(scenario: &Scenario) -> ScenarioResult<()> {
    let shared_dir = scenario.runtime_dir();
    for side in [Side::A, Side::B] {
        let paths = scenario.paths(side);
        if paths.runtime_dir() != shared_dir {
            return Err(format!(
                "endpoint {side:?} runtime directory should be {}, got {}",
                shared_dir.display(),
                paths.runtime_dir().display()
            ));
        }
        require_endpoint_scoped_name(paths.lock_file_name(), "lock")?;
        require_endpoint_scoped_name(paths.pid_file_name(), "pid")?;
        require_endpoint_scoped_name(paths.health_file_name(), "health")?;
    }

    for (first, second) in [
        (
            scenario.paths(Side::A).lock_file_name(),
            scenario.paths(Side::B).lock_file_name(),
        ),
        (
            scenario.paths(Side::A).pid_file_name(),
            scenario.paths(Side::B).pid_file_name(),
        ),
        (
            scenario.paths(Side::A).health_file_name(),
            scenario.paths(Side::B).health_file_name(),
        ),
    ] {
        if first == second {
            return Err(format!(
                "endpoints should not share the runtime file name {}",
                first.display()
            ));
        }
    }
    Ok(())
}

/// Checks that `name` follows the `weaverd-<64 hex>.<extension>` pattern.
fn require_endpoint_scoped_name(name: &Path, extension: &str) -> ScenarioResult<()> {
    let text = name
        .to_str()
        .ok_or_else(|| format!("runtime file name {} is not UTF-8", name.display()))?;
    let suffix = format!(".{extension}");
    let identifier = text
        .strip_prefix("weaverd-")
        .and_then(|rest| rest.strip_suffix(&suffix))
        .ok_or_else(|| format!("expected weaverd-<hash>.{extension}, got {text}"))?;
    if identifier.len() != 64 {
        return Err(format!(
            "endpoint identifier should be 64 hexadecimal digits, got {identifier:?}"
        ));
    }
    if !identifier
        .bytes()
        .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
    {
        return Err(format!(
            "endpoint identifier should be lowercase hexadecimal, got {identifier:?}"
        ));
    }
    Ok(())
}

/// Starts both daemons and verifies they keep independent runtime state.
pub(super) fn start_both_daemons(
    scenario: &Scenario,
    guard: &mut DaemonGuard,
) -> ScenarioResult<(u32, u32)> {
    let pid_a = scenario.start(Side::A)?;
    guard.register(pid_a);
    let pid_b = scenario.start(Side::B)?;
    guard.register(pid_b);
    verify_independent_state(scenario, pid_a, pid_b)?;
    Ok((pid_a, pid_b))
}

/// Verifies both daemons own their files and no legacy file was created.
fn verify_independent_state(scenario: &Scenario, pid_a: u32, pid_b: u32) -> ScenarioResult<()> {
    if pid_a == pid_b {
        return Err(format!("both daemons reported the same PID {pid_a}"));
    }
    for (side, pid) in [(Side::A, pid_a), (Side::B, pid_b)] {
        let paths = scenario.paths(side);
        require_present(scenario.dir(), paths.pid_file_name(), "endpoint PID file")?;
        require_present(
            scenario.dir(),
            paths.health_file_name(),
            "endpoint health file",
        )?;
        require_present(scenario.dir(), paths.lock_file_name(), "endpoint lock file")?;
        if read_pid(scenario.dir(), paths)? != pid {
            return Err(format!("endpoint {side:?} PID file should record {pid}"));
        }
        let health = read_file(scenario.dir(), paths.health_file_name())?;
        require_text(&health, "\"status\":\"ready\"", "health snapshot")?;
    }

    require_absent(
        scenario.dir(),
        Path::new(LEGACY_PID_FILE),
        "legacy PID file",
    )?;
    require_absent(
        scenario.dir(),
        Path::new(LEGACY_HEALTH_FILE),
        "legacy health file",
    )?;
    require_absent(
        scenario.dir(),
        Path::new(LEGACY_LOCK_FILE),
        "legacy lock file",
    )
}

/// Writes legacy decoy files that a correct implementation must ignore.
pub(super) fn install_legacy_decoys(scenario: &Scenario) -> ScenarioResult<()> {
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|error| format!("read system clock: {error}"))?
        .as_secs();
    write_file(
        scenario.dir(),
        Path::new(LEGACY_PID_FILE),
        &format!("{DECOY_PID}\n"),
    )?;
    let health =
        format!("{{\"status\":\"ready\",\"pid\":{DECOY_PID},\"timestamp\":{timestamp}}}\n");
    write_file(scenario.dir(), Path::new(LEGACY_HEALTH_FILE), &health)
}

/// Verifies `status` reports the endpoint-scoped PID, not the decoy.
pub(super) fn verify_status_ignores_legacy_decoys(
    scenario: &Scenario,
    pid_a: u32,
) -> ScenarioResult<()> {
    let (stdout, _stderr) = scenario.status(Side::A)?;
    let expected = format!(
        "daemon status: ready (pid {pid_a}) via {}",
        scenario.endpoint(Side::A)
    );
    require_text(&stdout, &expected, "daemon status stdout")?;
    if stdout.contains(&DECOY_PID.to_string()) {
        return Err(format!(
            "daemon status read the legacy decoy PID {DECOY_PID}: {stdout:?}"
        ));
    }
    Ok(())
}
