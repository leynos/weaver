//! Disposable single-thread caller/target probe; only the external controller owns cleanup.
//!
//! This harness never runs through libtest. Its private helpers belong only to
//! this diagnostic and must not be reused as production sandbox abstractions.

#[cfg(target_os = "macos")]
#[path = "macos_caller_probe/probe.rs"]
mod probe;

fn main() {
    #[cfg(target_os = "macos")]
    let status = match probe::run() {
        Ok(()) => 0,
        Err(error) => {
            use std::io::Write as _;
            let _diagnostic = writeln!(std::io::stderr().lock(), "probe: {error:#}");
            20
        }
    };
    #[cfg(not(target_os = "macos"))]
    let status = 77;
    std::process::exit(status);
}
