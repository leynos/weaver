//! CLI entrypoint for the Weaver semantic code tool.
//!
//! The binary now delegates to [`weaver_cli::run`], which loads configuration,
//! processes command-line arguments, negotiates capability output, and streams
//! JSONL requests to the configured daemon transport.

use std::io::{self, StderrLock, StdoutLock};
use std::process::ExitCode;

fn main() -> ExitCode {
    let mut stdout: StdoutLock<'_> = io::stdout().lock();
    let mut stderr: StderrLock<'_> = io::stderr().lock();
    weaver_cli::run(std::env::args_os(), &mut stdout, &mut stderr)
}
