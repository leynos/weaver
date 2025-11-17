//! Lifecycle command types and output abstractions.
//!
//! Defines the payloads and IO wrappers shared across lifecycle commands so the
//! controller can remain agnostic of concrete writers.

use std::ffi::OsString;
use std::fmt;
use std::io::Write;

use weaver_config::Config;

use super::LifecycleError;
use crate::DaemonAction;

/// Supported lifecycle commands.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LifecycleCommand {
    Start,
    Stop,
    Status,
}

impl fmt::Display for LifecycleCommand {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Start => formatter.write_str("start"),
            Self::Stop => formatter.write_str("stop"),
            Self::Status => formatter.write_str("status"),
        }
    }
}

/// Invocation payload forwarded from the CLI runtime.
#[derive(Debug, Clone)]
pub struct LifecycleInvocation {
    pub command: LifecycleCommand,
    pub arguments: Vec<String>,
}

/// Shared configuration context available to lifecycle handlers.
#[derive(Debug, Clone, Copy)]
pub struct LifecycleContext<'a> {
    pub config: &'a Config,
    pub config_arguments: &'a [OsString],
}

/// Output handle abstracting over stdout/stderr writers.
pub struct LifecycleOutput<W: Write, E: Write> {
    pub stdout: W,
    pub stderr: E,
}

impl<W: Write, E: Write> LifecycleOutput<W, E> {
    pub fn new(stdout: W, stderr: E) -> Self {
        Self { stdout, stderr }
    }

    pub fn stdout_line(&mut self, args: fmt::Arguments<'_>) -> Result<(), LifecycleError> {
        self.stdout.write_fmt(args).map_err(LifecycleError::Io)?;
        self.stdout.write_all(b"\n").map_err(LifecycleError::Io)?;
        self.stdout.flush().map_err(LifecycleError::Io)
    }

    pub fn stderr_line(&mut self, args: fmt::Arguments<'_>) -> Result<(), LifecycleError> {
        self.stderr.write_fmt(args).map_err(LifecycleError::Io)?;
        self.stderr.write_all(b"\n").map_err(LifecycleError::Io)?;
        self.stderr.flush().map_err(LifecycleError::Io)
    }
}

impl From<DaemonAction> for LifecycleCommand {
    fn from(action: DaemonAction) -> Self {
        match action {
            DaemonAction::Start => Self::Start,
            DaemonAction::Stop => Self::Stop,
            DaemonAction::Status => Self::Status,
        }
    }
}
