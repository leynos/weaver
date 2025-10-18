//! Command modelling for Weaver CLI requests.
//!
//! This module encapsulates the transformation from parsed CLI arguments into
//! the JSON payloads exchanged with the daemon so the main runtime remains
//! focused on IO orchestration.

use std::io::Write;

use serde::Serialize;

use crate::{AppError, Cli};

#[derive(Debug)]
pub(crate) struct CommandInvocation {
    pub(crate) domain: String,
    pub(crate) operation: String,
    pub(crate) arguments: Vec<String>,
}

impl TryFrom<Cli> for CommandInvocation {
    type Error = AppError;

    fn try_from(cli: Cli) -> Result<Self, Self::Error> {
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

#[derive(Debug, Serialize)]
pub(crate) struct CommandRequest {
    pub(crate) command: CommandDescriptor,
    pub(crate) arguments: Vec<String>,
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
        }
    }
}

impl CommandRequest {
    pub(crate) fn write_jsonl<W>(&self, writer: &mut W) -> Result<(), AppError>
    where
        W: Write,
    {
        serde_json::to_writer(&mut *writer, self).map_err(AppError::SerialiseRequest)?;
        writer.write_all(b"\n").map_err(AppError::SendRequest)?;
        writer.flush().map_err(AppError::SendRequest)
    }
}
