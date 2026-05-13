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
    command_surface::{CommandSurfaceRecord, find_read_only_command},
};

#[derive(Debug, PartialEq, Eq)]
pub(crate) struct CommandInvocation {
    pub(crate) domain: String,
    pub(crate) operation: String,
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
    fn try_from_structured_command(command: CliCommand) -> Result<Self, AppError> {
        match command {
            CliCommand::Definitions {
                action: DefinitionsAction::Get(args),
            } => {
                let record = find_read_only_command(&["definitions"], "get").ok_or_else(|| {
                    AppError::MissingCommandSurfaceRecord {
                        resource: String::from("definitions"),
                        verb: String::from("get"),
                    }
                })?;
                Ok(definition_get_invocation(record, args))
            }
            CliCommand::Daemon { .. } => Err(AppError::MissingDomain),
        }
    }
}

fn definition_get_invocation(
    record: &CommandSurfaceRecord,
    args: DefinitionGetArgs,
) -> CommandInvocation {
    CommandInvocation {
        domain: record.daemon_domain.to_string(),
        operation: record.daemon_operation.to_string(),
        arguments: vec![
            String::from("--uri"),
            args.uri,
            String::from("--position"),
            args.position,
        ],
    }
}

#[derive(Debug, Serialize)]
pub(crate) struct CommandRequest {
    pub(crate) command: CommandDescriptor,
    pub(crate) arguments: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) patch: Option<String>,
}

#[derive(Debug, Serialize)]
pub(crate) struct CommandDescriptor {
    pub(crate) domain: String,
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
    pub(crate) fn with_patch(invocation: CommandInvocation, patch: String) -> Self {
        Self {
            command: CommandDescriptor {
                domain: invocation.domain,
                operation: invocation.operation,
            },
            arguments: invocation.arguments,
            patch: Some(patch),
        }
    }

    pub(crate) fn write_jsonl<W>(&self, writer: &mut W) -> Result<(), AppError>
    where
        W: Write,
    {
        serde_json::to_writer(&mut *writer, self).map_err(AppError::SerialiseRequest)?;
        writer.write_all(b"\n").map_err(AppError::SendRequest)?;
        writer.flush().map_err(AppError::SendRequest)
    }
}
