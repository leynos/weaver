//! Run the sandbox feature scenarios in single-threaded child processes.

#[cfg(target_os = "linux")]
use std::ffi::OsStr;
#[cfg(target_os = "linux")]
use std::io::Write as _;
#[cfg(target_os = "linux")]
use std::process::Command;

use anyhow::Result;
#[cfg(target_os = "linux")]
use anyhow::{Context as _, bail, ensure};
#[cfg(target_os = "linux")]
use cap_std::{ambient_authority, fs::Dir};
#[cfg(target_os = "linux")]
use gherkin::{Feature, GherkinEnv, Step, StepType};
use libtest_mimic::Arguments;
#[cfg(target_os = "linux")]
use libtest_mimic::Trial;

#[cfg(target_os = "linux")]
mod support;
#[cfg(target_os = "linux")]
use support::TestWorld;

#[cfg(target_os = "linux")]
const CHILD_ARGUMENT: &str = "--weaver-sandbox-scenario-child";
#[cfg(target_os = "linux")]
const SCENARIO_NAMES: [&str; 5] = [
    "Allowed file is readable",
    "Disallowed file access is blocked",
    "Environment inheritance is restricted by default",
    "Environment variables are isolated by default",
    "Environment variables are fully inherited when enabled",
];

fn main() -> Result<()> {
    #[cfg(target_os = "linux")]
    if std::env::args_os().nth(1).as_deref() == Some(OsStr::new(CHILD_ARGUMENT)) {
        return run_child();
    }

    let trials = {
        #[cfg(target_os = "linux")]
        {
            linux_trials()?
        }
        #[cfg(not(target_os = "linux"))]
        {
            Vec::new()
        }
    };
    libtest_mimic::run(&Arguments::from_args(), trials).exit();
}

#[cfg(target_os = "linux")]
fn linux_trials() -> Result<Vec<Trial>> {
    let feature = load_feature()?;
    Ok(feature
        .scenarios
        .iter()
        .enumerate()
        .map(|(index, scenario)| {
            Trial::test(format!("sandbox::{}", scenario.name), move || {
                run_mimic_trial(index).map_err(|error| format!("{error:#}").into())
            })
        })
        .collect())
}

#[cfg(target_os = "linux")]
fn load_feature() -> Result<Feature> {
    let feature = Feature::parse(
        include_str!("features/sandbox.feature"),
        GherkinEnv::default(),
    )
    .context("parse sandbox feature")?;
    ensure!(
        feature.background.is_none(),
        "unexpected feature background"
    );
    ensure!(feature.rules.is_empty(), "unexpected feature rules");
    let names = feature
        .scenarios
        .iter()
        .map(|scenario| scenario.name.as_str())
        .collect::<Vec<_>>();
    ensure!(
        names == SCENARIO_NAMES,
        "unexpected sandbox scenarios: {names:?}"
    );
    ensure!(
        feature
            .scenarios
            .iter()
            .all(|scenario| scenario.examples.is_empty()),
        "scenario examples need an explicit trial mapping"
    );
    Ok(feature)
}

#[cfg(target_os = "linux")]
fn run_mimic_trial(index: usize) -> Result<()> {
    let before = environment_markers()?;
    let executable = std::env::current_exe().context("locate sandbox test executable")?;
    let mut command = Command::new(executable);
    command
        .arg(CHILD_ARGUMENT)
        .arg(index.to_string())
        .env_clear();
    if (2..=4).contains(&index) {
        command
            .env("KEEP_ME", "present")
            .env("DROP_ME", "remove-me");
    }
    let child = command
        .output()
        .context("run single-threaded sandbox scenario")?;
    ensure!(
        child.status.success(),
        "scenario child failed: {}; stderr: {}",
        child.status,
        String::from_utf8_lossy(&child.stderr)
    );
    let child_stdout = std::str::from_utf8(&child.stdout).context("decode thread evidence")?;
    ensure!(
        child_stdout.lines().any(|line| line == "Threads: 1"),
        "missing single-thread evidence: {child_stdout}"
    );
    std::io::stdout()
        .lock()
        .write_all(&child.stdout)
        .context("report worker thread count")?;
    let after = environment_markers()?;
    ensure!(
        before == after,
        "scenario changed mimic process environment"
    );
    Ok(())
}

#[cfg(target_os = "linux")]
fn run_child() -> Result<()> {
    let index = std::env::args_os()
        .nth(2)
        .context("missing scenario index")?
        .into_string()
        .map_err(|_| anyhow::anyhow!("scenario index is not Unicode"))?
        .parse::<usize>()
        .context("invalid scenario index")?;
    let feature = load_feature()?;
    let scenario = feature
        .scenarios
        .get(index)
        .context("unknown scenario index")?;
    let mut world = TestWorld::new()?;
    let mut launch_count = 0;
    for step in &scenario.steps {
        if step.ty == StepType::When {
            require_one_thread()?;
            launch_count += 1;
        }
        execute_step(&mut world, step)
            .with_context(|| format!("scenario {:?}, step {step}", scenario.name))?;
    }
    ensure!(launch_count == 1, "scenario must launch exactly once");
    Ok(())
}

#[cfg(target_os = "linux")]
fn require_one_thread() -> Result<()> {
    let process_dir = Dir::open_ambient_dir("/proc/self", ambient_authority())
        .context("open own procfs directory")?;
    let status = process_dir
        .read_to_string("status")
        .context("read own status")?;
    let thread_line = status
        .lines()
        .find(|line| line.starts_with("Threads:"))
        .context("find process thread count")?;
    ensure!(
        thread_line.split_whitespace().eq(["Threads:", "1"]),
        "scenario child is not single-threaded: {thread_line}"
    );
    writeln!(std::io::stdout().lock(), "Threads: 1").context("report process thread count")
}

#[cfg(target_os = "linux")]
fn execute_step(world: &mut TestWorld, step: &Step) -> Result<()> {
    match step.ty {
        StepType::Given => execute_given(world, &step.value),
        StepType::When if step.value == "the sandbox launches the command" => world.launch(),
        StepType::Then => execute_then(world, &step.value),
        _ => bail!("unrecognized sandbox step: {step}"),
    }
}

#[cfg(target_os = "linux")]
fn execute_given(world: &mut TestWorld, text: &str) -> Result<()> {
    match text {
        "a sandbox world with fixture files" => Ok(()),
        "the command cats the allowed file" => world.configure_cat(&world.allowed_file.clone()),
        "the command cats the forbidden file" => world.configure_cat(&world.forbidden_file.clone()),
        "the sandbox allows the command and fixture file" => {
            let program = std::path::PathBuf::from(
                world
                    .command
                    .as_ref()
                    .context("command not configured")?
                    .get_program(),
            );
            world.profile = world
                .profile
                .clone()
                .allow_executable(program)
                .allow_read_path(&world.allowed_file);
            Ok(())
        }
        "environment variables KEEP_ME and DROP_ME are set" => {
            let markers = environment_markers()?;
            ensure!(
                markers.contains(&"KEEP_ME=present".to_owned())
                    && markers.contains(&"DROP_ME=remove-me".to_owned()),
                "scenario child did not receive its environment markers: {markers:?}"
            );
            Ok(())
        }
        "the sandbox allows only KEEP_ME to be inherited" => {
            world.configure_env_reader()?;
            world.profile = world.profile.clone().allow_environment_variable("KEEP_ME");
            Ok(())
        }
        "the sandbox uses the default environment isolation" => world.configure_env_reader(),
        "the sandbox inherits the full environment" => {
            world.configure_env_reader()?;
            world.profile = world.profile.clone().allow_full_environment();
            Ok(())
        }
        _ => bail!("unrecognized Given step: {text}"),
    }
}

#[cfg(target_os = "linux")]
fn execute_then(world: &mut TestWorld, text: &str) -> Result<()> {
    match text {
        "the sandboxed process succeeds" => {
            let output = world.output.as_ref().context("process output missing")?;
            ensure!(
                output.status.success(),
                "sandboxed process failed: {:?}; stderr: {}",
                output.status,
                String::from_utf8_lossy(&output.stderr)
            );
            ensure!(
                world.launch_error.is_none(),
                "sandbox launch failed: {:?}",
                world.launch_error
            );
            Ok(())
        }
        "the sandboxed process fails" => {
            ensure!(
                world.launch_error.is_none(),
                "sandbox launch failed: {:?}",
                world.launch_error
            );
            let output = world.output.as_ref().context("process output missing")?;
            ensure!(
                !output.status.success(),
                "sandboxed process unexpectedly succeeded"
            );
            let stderr = String::from_utf8_lossy(&output.stderr);
            ensure!(
                stderr.contains("cat:")
                    && stderr.contains(world.forbidden_file.to_string_lossy().as_ref()),
                "failure did not come from forbidden file access: {stderr}"
            );
            Ok(())
        }
        "environment markers are cleaned up" => {
            // Birdcage restores this child's input environment; the mimic
            // callback separately proves those inputs never reach its process.
            let markers = environment_markers()?;
            ensure!(
                markers.contains(&"KEEP_ME=present".to_owned())
                    && markers.contains(&"DROP_ME=remove-me".to_owned()),
                "sandbox did not restore the scenario child's environment"
            );
            Ok(())
        }
        text if text.starts_with("stdout contains ") => {
            let wanted = text
                .trim_start_matches("stdout contains ")
                .trim_matches('"');
            let stdout = captured_stdout(world)?;
            ensure!(
                stdout.contains(wanted),
                "stdout lacks {wanted:?}: {stdout:?}"
            );
            Ok(())
        }
        text if text.starts_with("stdout does not contain ") => {
            let unwanted = text
                .trim_start_matches("stdout does not contain ")
                .trim_matches('"');
            let stdout = captured_stdout(world)?;
            ensure!(
                !stdout.contains(unwanted),
                "stdout contains {unwanted:?}: {stdout:?}"
            );
            Ok(())
        }
        _ => bail!("unrecognized Then step: {text}"),
    }
}

#[cfg(target_os = "linux")]
fn captured_stdout(world: &TestWorld) -> Result<String> {
    let output = world.output.as_ref().context("process output missing")?;
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

#[cfg(target_os = "linux")]
fn environment_markers() -> Result<Vec<String>> {
    let output = Command::new("/usr/bin/env")
        .output()
        .context("inspect process environment")?;
    ensure!(output.status.success(), "environment inspection failed");
    let text = String::from_utf8(output.stdout).context("decode environment output")?;
    Ok(text
        .lines()
        .filter(|line| line.starts_with("KEEP_ME=") || line.starts_with("DROP_ME="))
        .map(str::to_owned)
        .collect())
}
