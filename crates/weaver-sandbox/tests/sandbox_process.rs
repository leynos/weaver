//! Process-level behaviour of the sandbox: spawn preflight errors (relative,
//! missing, unlisted and multi-threaded) and the parent environment's
//! restoration. No case verifies that a program runs successfully inside the
//! sandbox, so file access and environment inheritance are not covered here.
//!
//! `Sandbox::spawn` refuses a process with more than one thread, and
//! `birdcage` rewrites the environment of the process that activates it, so
//! neither fits the libtest harness (a thread per test, one shared
//! environment). This target therefore has its own `main` (`harness = false`).
//! It answers the libtest listing and `--exact` protocol, so `cargo test` and
//! `cargo nextest` both run it, and it runs every case in a fresh child copy of
//! the binary. The parent builds each child's environment with
//! `Command::env`, so no case changes the environment of a process that other
//! cases share.

#[cfg(target_os = "linux")]
#[path = "sandbox_process/cases.rs"]
mod cases;
#[cfg(target_os = "linux")]
#[path = "sandbox_process/runner.rs"]
mod runner;

/// Runs the cases on Linux, the only platform `birdcage` supports.
#[cfg(target_os = "linux")]
fn main() -> std::process::ExitCode { runner::main() }

/// Sandboxing is Linux-only, so there is nothing to run elsewhere.
#[cfg(not(target_os = "linux"))]
fn main() {}
