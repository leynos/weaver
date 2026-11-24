//! A minimal libtest-compatible runner: it answers `--list`, honours `--exact`
//! and substring filters, and runs every case in a fresh child copy of the
//! binary so that no case changes the environment of a shared process.

use std::{
    env,
    io::Write as _,
    process::{Command, ExitCode},
};

use anyhow::{Context as _, Result, ensure};

use crate::cases::{CASES, Case, SCENARIO_ENV, Skipped};

/// Names the variable whose presence makes this binary run a case itself.
const CHILD_MARKER: &str = "WEAVER_SANDBOX_PROCESS_CHILD";
/// Prefix of the line a child prints when its case was skipped.
const SKIP_MARKER: &str = "SKIPPED: ";

/// Writes one line to standard output; a closed pipe is not worth failing for.
fn say(line: std::fmt::Arguments<'_>) { writeln!(std::io::stdout().lock(), "{line}").ok(); }

/// Whether `filter` selects the case called `name`.
fn matches_filter(name: &str, filter: &str, exact: bool) -> bool {
    if exact {
        filter == name
    } else {
        name.contains(filter)
    }
}

/// Cases the libtest arguments select: `--exact` names or substring filters.
fn selected(args: &[String]) -> Vec<&'static Case> {
    let exact = args.iter().any(|arg| arg == "--exact");
    let filters = positional(args);
    let wanted = |name: &str| {
        filters.is_empty()
            || filters
                .iter()
                .any(|filter| matches_filter(name, filter, exact))
    };
    CASES.iter().filter(|(name, _)| wanted(name)).collect()
}

/// Positional arguments, skipping flags and the values of valued flags.
fn positional(args: &[String]) -> Vec<String> {
    const VALUED: [&str; 5] = [
        "--format",
        "--test-threads",
        "--skip",
        "--color",
        "--logfile",
    ];
    let mut out = Vec::new();
    let mut skip_value = false;
    for arg in args {
        if skip_value {
            skip_value = false;
        } else if VALUED.contains(&arg.as_str()) {
            skip_value = true;
        } else if !arg.starts_with('-') {
            out.push(arg.clone());
        }
    }
    out
}

/// Re-runs `name` in a child copy of this binary and reports its outcome.
fn run_in_child(name: &str) -> Result<()> {
    let mut command = Command::new(env::current_exe().context("locate the test binary")?);
    // `birdcage` re-enters the working directory inside the new root, which
    // only holds the whitelisted paths, so the child must start at `/`.
    command
        .args(["--exact", name, "--nocapture"])
        .env(CHILD_MARKER, "1")
        .current_dir("/");
    command.envs(SCENARIO_ENV);
    let output = command.output().context("spawn the child")?;
    ensure!(
        output.status.success(),
        "{name} failed ({}): {}{}",
        output.status,
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    for line in stdout.lines().filter(|line| line.starts_with(SKIP_MARKER)) {
        say(format_args!("{name}: {line}"));
    }
    Ok(())
}

/// Runs one case, in this process when it is the child.
fn run_case((name, body): &Case) -> Result<()> {
    if env::var_os(CHILD_MARKER).is_none() {
        return run_in_child(name);
    }
    match body() {
        Err(err) if err.downcast_ref::<Skipped>().is_some() => {
            say(format_args!("{SKIP_MARKER}{err}"));
            Ok(())
        }
        other => other,
    }
}

/// Prints the case names in the format `cargo nextest` lists tests.
fn list_cases(args: &[String]) {
    if args.iter().any(|arg| arg == "--ignored") {
        return;
    }
    for (name, _) in CASES {
        say(format_args!("{name}: test"));
    }
}

/// Entry point: lists or runs the selected cases.
pub fn main() -> ExitCode {
    let args: Vec<String> = env::args_os()
        .skip(1)
        .filter_map(|arg| arg.into_string().ok())
        .collect();
    if args.iter().any(|arg| arg == "--list") {
        list_cases(&args);
        return ExitCode::SUCCESS;
    }
    let mut failed = false;
    for case in selected(&args) {
        let outcome = run_case(case);
        failed |= outcome.is_err();
        match outcome {
            Ok(()) => say(format_args!("test {} ... ok", case.0)),
            Err(err) => say(format_args!("test {} ... FAILED\n{err:#}", case.0)),
        }
    }
    if failed {
        ExitCode::FAILURE
    } else {
        ExitCode::SUCCESS
    }
}
