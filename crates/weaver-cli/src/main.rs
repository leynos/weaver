//! CLI entrypoint for the Weaver semantic code tool.
//!
//! The binary now delegates to [`weaver_cli::run`], which loads configuration,
//! processes command-line arguments, negotiates capability output, and streams
//! JSONL requests to the configured daemon transport.

use std::io::{self, IsTerminal, StderrLock, StdinLock, StdoutLock};
use std::process::ExitCode;

fn main() -> ExitCode {
    let stdout_is_terminal = io::stdout().is_terminal();
    let mut stdin: StdinLock<'_> = io::stdin().lock();
    let mut stdout: StdoutLock<'_> = io::stdout().lock();
    let mut stderr: StderrLock<'_> = io::stderr().lock();
    let mut io =
        weaver_cli::IoStreams::new(&mut stdin, &mut stdout, &mut stderr, stdout_is_terminal);
    weaver_cli::run(std::env::args_os(), &mut io)
}
