//! Verify a directly launched sandboxed command in a single-threaded child.

#[cfg(target_os = "linux")]
use std::{ffi::OsStr, io::Write as _, process::Command};

#[cfg(target_os = "linux")]
use anyhow::{Context as _, Result, ensure};
#[cfg(target_os = "linux")]
use cap_std::{ambient_authority, fs::Dir};
use libtest_mimic::Arguments;
#[cfg(target_os = "linux")]
use libtest_mimic::Trial;
#[cfg(target_os = "linux")]
use weaver_sandbox::{Sandbox, SandboxCommand, SandboxProfile, process::Stdio};

#[cfg(target_os = "linux")]
const CHILD_ARGUMENT: &str = "--weaver-sandbox-probe-child";
#[cfg(target_os = "linux")]
const MARKERS: [&str; 2] = ["KEEP_ME=present-λ", "DROP_ME=remove-me-東京"];

fn main() {
    #[cfg(target_os = "linux")]
    if std::env::args_os().any(|argument| argument == OsStr::new(CHILD_ARGUMENT)) {
        if let Err(error) = run_child_probe() {
            let _diagnostic = writeln!(std::io::stderr().lock(), "child probe failed: {error:#}");
            std::process::exit(1);
        }
        return;
    }

    let trials = {
        #[cfg(target_os = "linux")]
        {
            vec![Trial::test(
                "sandbox::full_environment_in_single_thread_child",
                || run_mimic_trial().map_err(|error| format!("{error:#}").into()),
            )]
        }
        #[cfg(not(target_os = "linux"))]
        {
            Vec::new()
        }
    };
    libtest_mimic::run(&Arguments::from_args(), trials).exit();
}

#[cfg(target_os = "linux")]
fn run_mimic_trial() -> Result<()> {
    require_markers_absent("before child")?;

    let executable = std::env::current_exe().context("locate current test executable")?;
    let child = Command::new(executable)
        .arg(CHILD_ARGUMENT)
        .env_clear()
        .env("KEEP_ME", "present-λ")
        .env("DROP_ME", "remove-me-東京")
        .output()
        .context("run single-threaded child")?;
    ensure!(
        child.status.success(),
        "child failed: {}; stderr: {}",
        child.status,
        String::from_utf8_lossy(&child.stderr)
    );
    let child_thread_line =
        std::str::from_utf8(&child.stdout).context("decode worker thread evidence")?;
    ensure!(
        child_thread_line.split_whitespace().eq(["Threads:", "1"]),
        "unexpected worker thread evidence: {child_thread_line}"
    );
    std::io::stdout()
        .lock()
        .write_all(&child.stdout)
        .context("report verified worker thread count")?;

    require_markers_absent("after child")
}

#[cfg(target_os = "linux")]
fn run_child_probe() -> Result<()> {
    let process_dir = Dir::open_ambient_dir("/proc/self", ambient_authority())
        .context("open own procfs directory")?;
    let status = process_dir
        .read_to_string("status")
        .context("read own status")?;
    let thread_line = status
        .lines()
        .find(|line| line.starts_with("Threads:"))
        .context("find thread count")?;
    ensure!(
        thread_line.split_whitespace().nth(1) == Some("1"),
        "child is not single-threaded: {thread_line}"
    );

    let profile = SandboxProfile::new()
        .allow_executable("/usr/bin/env")
        .allow_full_environment();
    let mut command = SandboxCommand::new("/usr/bin/env");
    command.stdout(Stdio::piped());
    let output = Sandbox::new(profile)
        .spawn(command)
        .context("spawn public sandbox command")?
        .wait_with_output()
        .context("wait for sandboxed env")?;
    ensure!(
        output.status.success(),
        "sandboxed env failed: {}",
        output.status
    );
    require_markers_present(&output.stdout, "sandboxed env")?;

    let restored = Command::new("/usr/bin/env")
        .output()
        .context("run unsandboxed env after spawn")?;
    ensure!(
        restored.status.success(),
        "restored env failed: {}",
        restored.status
    );
    require_markers_present(&restored.stdout, "restored env")?;
    writeln!(std::io::stdout().lock(), "{thread_line}").context("report worker thread evidence")?;
    Ok(())
}

#[cfg(target_os = "linux")]
fn require_markers_present(output: &[u8], label: &str) -> Result<()> {
    let text = String::from_utf8(output.to_vec()).context("decode env output")?;
    for marker in MARKERS {
        ensure!(
            text.lines().any(|line| line == marker),
            "{label} lacks {marker}"
        );
    }
    Ok(())
}

#[cfg(target_os = "linux")]
fn require_markers_absent(label: &str) -> Result<()> {
    let output = Command::new("/usr/bin/env")
        .output()
        .context("inspect mimic process environment")?;
    ensure!(
        output.status.success(),
        "mimic env failed: {}",
        output.status
    );
    let text = String::from_utf8(output.stdout).context("decode mimic env")?;
    for marker in MARKERS {
        ensure!(
            !text.lines().any(|line| line == marker),
            "mimic process contains {marker} {label}"
        );
    }
    Ok(())
}
