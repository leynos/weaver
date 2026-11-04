//! Command modelling for Weaver CLI requests.
//!
//! This module encapsulates the transformation from parsed CLI arguments into
//! the JSON payloads exchanged with the daemon so the main runtime remains
//! focused on IO orchestration.

use std::io::Write;

use serde::Serialize;

use crate::{
    AppError,
    Cli,
    CliCommand,
    DefinitionsAction,
    cli::DefinitionGetArgs,
    command_surface::{CommandSurfaceRecord, DEFINITIONS_GET, find_read_only_command},
};

/// A CLI request normalised into the daemon's domain and operation form.
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct CommandInvocation {
    /// Capability domain the daemon dispatches on, such as `"observe"`.
    pub(crate) domain: String,
    /// Operation name within `domain`, such as `"get-definition"`.
    pub(crate) operation: String,
    /// Arguments forwarded verbatim to the daemon operation.
    pub(crate) arguments: Vec<String>,
}

impl TryFrom<Cli> for CommandInvocation {
    type Error = AppError;

    fn try_from(cli: Cli) -> Result<Self, Self::Error> {
        if let Some(command) = cli.command {
            return Self::try_from_structured_command(command);
        }

        let domain = cli.domain.ok_or(AppError::MissingDomain)?.trim().to_owned();
        let operation = cli
            .operation
            .ok_or(AppError::MissingOperation)?
            .trim()
            .to_owned();
        if domain.is_empty() {
            return Err(AppError::MissingDomain);
        }
        if operation.is_empty() {
            return Err(AppError::MissingOperation);
        }
        Ok(Self {
            domain,
            operation,
            arguments: cli.arguments,
        })
    }
}

impl CommandInvocation {
    /// Converts a structured `clap` subcommand into a daemon invocation.
    ///
    /// # Errors
    ///
    /// Returns `AppError::MissingDomain` for subcommands without a daemon
    /// mapping, and `AppError::MissingCommandSurfaceRecord` when the
    /// command-surface table has no matching read-only record.
    fn try_from_structured_command(command: CliCommand) -> Result<Self, AppError> {
        match command {
            CliCommand::Definitions {
                action: DefinitionsAction::Get(args),
            } => {
                let adapter_record = &DEFINITIONS_GET;
                let record =
                    find_read_only_command(adapter_record.resource_path, adapter_record.verb)
                        .ok_or_else(|| AppError::MissingCommandSurfaceRecord {
                            resource: adapter_record.resource_path.join(" "),
                            verb: String::from(adapter_record.verb),
                        })?;
                Ok(definition_get_invocation(record, args))
            }
            CliCommand::Daemon { .. } => Err(AppError::MissingDomain),
        }
    }
}

/// Builds the daemon invocation for `weaver definitions get`.
fn definition_get_invocation(
    record: &CommandSurfaceRecord,
    args: DefinitionGetArgs,
) -> CommandInvocation {
    CommandInvocation {
        domain: record.daemon_domain.to_owned(),
        operation: record.daemon_operation.to_owned(),
        arguments: vec![
            String::from("--uri"),
            args.uri,
            String::from("--position"),
            args.position,
        ],
    }
}

/// The JSON payload sent to the daemon for one command execution.
#[derive(Debug, Serialize)]
pub(crate) struct CommandRequest {
    /// The domain and operation the daemon should execute.
    pub(crate) command: CommandDescriptor,
    /// Arguments forwarded verbatim to the daemon operation.
    pub(crate) arguments: Vec<String>,
    /// Unified diff to apply, present only for `act apply-patch` requests.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) patch: Option<String>,
}

/// The address of a daemon operation inside a capability domain.
#[derive(Debug, Serialize)]
pub(crate) struct CommandDescriptor {
    /// Capability domain, such as `"observe"`.
    pub(crate) domain: String,
    /// Operation name within `domain`, such as `"get-definition"`.
    pub(crate) operation: String,
}

impl From<CommandInvocation> for CommandRequest {
    fn from(invocation: CommandInvocation) -> Self {
        Self {
            command: CommandDescriptor {
                domain: invocation.domain,
                operation: invocation.operation,
            },
            arguments: invocation.arguments,
            patch: None,
        }
    }
}

impl CommandInvocation {
    /// Returns true when this invocation targets the `act apply-patch` operation.
    pub(crate) fn is_apply_patch(&self) -> bool {
        self.domain.eq_ignore_ascii_case("act")
            && self.operation.eq_ignore_ascii_case("apply-patch")
    }
}

impl CommandRequest {
    /// Builds an `act apply-patch` request from `invocation` and `patch`.
    pub(crate) fn with_patch(invocation: CommandInvocation, patch: String) -> Self {
        Self {
            patch: Some(patch),
            ..Self::from(invocation)
        }
    }

    /// Serialises the request as one JSON line and flushes the writer.
    ///
    /// # Errors
    ///
    /// Returns `AppError::SerialiseRequest` if the request cannot be encoded as
    /// JSON, and `AppError::SendRequest` if writing or flushing fails.
    pub(crate) fn write_jsonl<W>(&self, writer: &mut W) -> Result<(), AppError>
    where
        W: Write,
    {
        serde_json::to_writer(&mut *writer, self).map_err(AppError::SerialiseRequest)?;
        writer.write_all(b"\n").map_err(AppError::SendRequest)?;
        writer.flush().map_err(AppError::SendRequest)
    }
}
