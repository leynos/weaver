//! A minimal libtest-compatible runner: it answers `--list`, honours `--exact`
//! and substring filters, and runs every case in a fresh child copy of the
//! binary so that no case changes the environment of a shared process.

use std::{
    env,
    io::Write as _,
    process::{Command, ExitCode},
};

use anyhow::{Context as _, Result, ensure};

use crate::cases::{CASES, Case, SCENARIO_ENV};

/// Names the variable whose presence makes this binary run a case itself.
const CHILD_MARKER: &str = "WEAVER_SANDBOX_PROCESS_CHILD";

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

/// What the libtest arguments ask for: names or filters, `--skip`s, `--exact`
/// matching and `--ignored`.
#[derive(Default)]
struct Selection {
    /// `--exact`: filters and skips name whole cases instead of substrings.
    exact: bool,
    /// `--ignored`: only ignored cases, and none of these is ignored.
    ignored_only: bool,
    /// Positional names or substrings; empty selects every case.
    filters: Vec<String>,
    /// `--skip` names or substrings, which remove cases again.
    skips: Vec<String>,
}

impl Selection {
    /// Whether this selection includes the case called `name`.
    fn wants(&self, name: &str) -> bool {
        let named = self.filters.is_empty()
            || self
                .filters
                .iter()
                .any(|filter| matches_filter(name, filter, self.exact));
        let skipped = self
            .skips
            .iter()
            .any(|skip| matches_filter(name, skip, self.exact));
        named && !skipped && !self.ignored_only
    }

    /// The cases this selection includes, in listing order.
    fn cases(&self) -> Vec<&'static Case> {
        CASES.iter().filter(|(name, _)| self.wants(name)).collect()
    }
}

/// Flags whose value is the next argument and which select nothing.
const VALUED_FLAGS: [&str; 4] = ["--format", "--test-threads", "--color", "--logfile"];

/// Reads the selection out of the libtest arguments.
fn parse_selection(args: &[String]) -> Selection {
    let mut selection = Selection::default();
    let mut rest = args.iter();
    while let Some(arg) = rest.next() {
        match arg.as_str() {
            "--exact" => selection.exact = true,
            "--ignored" => selection.ignored_only = true,
            "--skip" => selection.skips.extend(rest.next().cloned()),
            flag if VALUED_FLAGS.contains(&flag) => {
                rest.next();
            }
            flag if flag.starts_with('-') => {
                selection
                    .skips
                    .extend(flag.strip_prefix("--skip=").map(str::to_owned));
            }
            name => selection.filters.push(name.to_owned()),
        }
    }
    selection
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
    Ok(())
}

/// Runs one case, in this process when it is the child.
fn run_case((name, body): &Case) -> Result<()> {
    if env::var_os(CHILD_MARKER).is_none() {
        return run_in_child(name);
    }
    body()
}

/// Prints the selected case names in the format `cargo nextest` lists tests.
fn list_cases(selection: &Selection) {
    for (name, _) in selection.cases() {
        say(format_args!("{name}: test"));
    }
}

/// Entry point: lists or runs the selected cases.
pub fn main() -> ExitCode {
    let args: Vec<String> = env::args_os()
        .skip(1)
        .filter_map(|arg| arg.into_string().ok())
        .collect();
    let selection = parse_selection(&args);
    if args.iter().any(|arg| arg == "--list") {
        list_cases(&selection);
        return ExitCode::SUCCESS;
    }
    let mut failed = false;
    for case in selection.cases() {
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
