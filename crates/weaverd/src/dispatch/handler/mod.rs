//! Connection handler that dispatches JSONL commands.
//!
//! This module provides the `DispatchConnectionHandler` which implements the
//! `ConnectionHandler` trait from the transport layer. It reads JSONL requests,
//! parses them into typed commands, routes them to domain handlers, and streams
//! responses back to the client.

use std::path::PathBuf;

use super::{
    backend_manager::BackendManager,
    errors::DispatchError,
    request::CommandRequest,
    response::ResponseWriter,
    router::{DISPATCH_TARGET, DomainRouter},
};
use crate::transport::{ConnectionHandler, ConnectionStream};

mod reader;
mod structured_event;

use self::{
    reader::{read_error_message, read_request_line},
    structured_event::{
        StructuredDispatchEvent,
        StructuredEventMetadata,
        emit_structured_event,
        read_error_event,
    },
};

#[cfg(test)]
mod tests;

#[derive(Debug)]
enum ReadRequestError {
    ClientDisconnected,
    BadRequest(DispatchError),
}

/// Connection handler that parses and dispatches JSONL commands.
///
/// Each connection is handled synchronously: the handler reads a single JSONL
/// request line, parses it, routes it to domain handlers, and writes the
/// response stream before closing the connection.
#[derive(Debug)]
pub struct DispatchConnectionHandler {
    router: DomainRouter,
    backends: BackendManager,
    endpoint: String,
    runtime_dir: PathBuf,
}

impl DispatchConnectionHandler {
    /// Creates a new dispatch handler with a backend manager and workspace root.
    pub fn new(
        backends: BackendManager,
        workspace_root: PathBuf,
        endpoint: impl Into<String>,
        runtime_dir: PathBuf,
    ) -> Result<Self, DispatchError> {
        Ok(Self {
            router: DomainRouter::new(workspace_root)?,
            backends,
            endpoint: endpoint.into(),
            runtime_dir,
        })
    }

    fn dispatch(&self, mut stream: ConnectionStream) {
        let (request_bytes, request) = match self.read_request(&mut stream) {
            Ok(request) => request,
            Err(ReadRequestError::ClientDisconnected) => return,
            Err(ReadRequestError::BadRequest(error)) => {
                let mut writer = ResponseWriter::new(&mut stream);
                if let Err(writer_error) = writer.write_error(&error) {
                    tracing::warn!(
                        target: DISPATCH_TARGET,
                        endpoint = %self.endpoint,
                        transport_error = %writer_error,
                        response_error = %error,
                        "failed to write request parse error response"
                    );
                }
                return;
            }
        };
        let mut writer = ResponseWriter::new(&mut stream);

        let event = StructuredDispatchEvent::new(
            "dispatching_request",
            &self.endpoint,
            self.runtime_dir.as_path(),
            StructuredEventMetadata::new(request.domain(), request.operation())
                .with_size(request_bytes.len()),
        );
        emit_structured_event(&event, "dispatching request", false);

        self.route_request(request, request_bytes.len(), &mut writer);
    }

    fn read_request(
        &self,
        stream: &mut ConnectionStream,
    ) -> Result<(Vec<u8>, CommandRequest), ReadRequestError> {
        let request_bytes = match read_request_line(stream) {
            Ok(Some(bytes)) => bytes,
            Ok(None) => {
                tracing::debug!(
                    target: DISPATCH_TARGET,
                    "client disconnected without request"
                );
                return Err(ReadRequestError::ClientDisconnected);
            }
            Err(error) => {
                let event = read_error_event(&error, &self.endpoint, self.runtime_dir.as_path());
                emit_structured_event(&event, read_error_message(&error), true);
                tracing::warn!(target: DISPATCH_TARGET, %error, "failed to read request");
                return Err(ReadRequestError::BadRequest(error));
            }
        };

        let request = match CommandRequest::parse(&request_bytes) {
            Ok(req) => req,
            Err(error) => {
                let event = StructuredDispatchEvent::new(
                    "request_rejected",
                    &self.endpoint,
                    self.runtime_dir.as_path(),
                    StructuredEventMetadata::none().with_size(request_bytes.len()),
                );
                emit_structured_event(&event, "request rejected: malformed JSON", true);
                tracing::warn!(target: DISPATCH_TARGET, %error, "malformed request");
                return Err(ReadRequestError::BadRequest(error));
            }
        };

        if let Err(error) = request.validate() {
            let event = StructuredDispatchEvent::new(
                "request_rejected",
                &self.endpoint,
                self.runtime_dir.as_path(),
                StructuredEventMetadata::new(request.domain(), request.operation())
                    .with_size(request_bytes.len()),
            );
            emit_structured_event(&event, "request rejected: invalid request", true);
            tracing::warn!(target: DISPATCH_TARGET, %error, "invalid request");
            return Err(ReadRequestError::BadRequest(error));
        }

        Ok((request_bytes, request))
    }

    fn route_request<W: std::io::Write>(
        &self,
        request: CommandRequest,
        request_size: usize,
        writer: &mut ResponseWriter<W>,
    ) {
        let route_result = self
            .backends
            .with_backends(|backends| self.router.route(&request, writer, backends));
        let context = Self::request_context(&request, request_size);

        match route_result {
            Ok(Ok(result)) => self.write_exit_status(&context, result.status, writer),
            Ok(Err(error)) => {
                emit_structured_event(
                    &self.with_metadata(&context, "dispatch_failed"),
                    "request dispatch failed",
                    true,
                );
                tracing::warn!(target: DISPATCH_TARGET, %error, "request dispatch failed");
                self.write_error_response(
                    &context,
                    writer,
                    &error,
                    "failed to write dispatch error response",
                );
            }
            Err(error) => {
                emit_structured_event(
                    &self.with_metadata(&context, "dispatch_infra_error"),
                    "dispatch infrastructure error",
                    true,
                );
                tracing::warn!(target: DISPATCH_TARGET, %error, "dispatch infrastructure error");
                self.write_error_response(
                    &context,
                    writer,
                    &error,
                    "failed to write infrastructure error response",
                );
                self.write_exit_status(&context, error.exit_status(), writer);
            }
        }
    }

    fn request_context(request: &CommandRequest, request_size: usize) -> RouteContext<'_> {
        RouteContext {
            request,
            request_size,
        }
    }

    fn with_metadata<'a>(
        &self,
        context: &RouteContext<'a>,
        event_name: &'static str,
    ) -> StructuredDispatchEvent {
        StructuredDispatchEvent::new(
            event_name,
            &self.endpoint,
            self.runtime_dir.as_path(),
            StructuredEventMetadata::new(context.request.domain(), context.request.operation())
                .with_size(context.request_size),
        )
    }

    fn write_exit_status<W: std::io::Write>(
        &self,
        context: &RouteContext<'_>,
        status: i32,
        writer: &mut ResponseWriter<W>,
    ) {
        if let Err(transport_error) = writer.write_exit(status) {
            tracing::warn!(
                target: DISPATCH_TARGET,
                endpoint = %self.endpoint,
                domain = context.request.domain(),
                operation = context.request.operation(),
                request_size = context.request_size,
                transport_error = %transport_error,
                "failed to write exit"
            );
        }
    }

    #[expect(
        clippy::too_many_arguments,
        reason = "keeps the shared write path explicit at both semantic call sites"
    )]
    fn write_error_response<W: std::io::Write>(
        &self,
        context: &RouteContext<'_>,
        writer: &mut ResponseWriter<W>,
        response_error: &DispatchError,
        failure_message: &str,
    ) {
        if let Err(transport_error) = writer.write_error(response_error) {
            tracing::warn!(
                target: DISPATCH_TARGET,
                endpoint = %self.endpoint,
                domain = context.request.domain(),
                operation = context.request.operation(),
                request_size = context.request_size,
                response_error = %response_error,
                transport_error = %transport_error,
                "{}", failure_message
            );
        }
    }
}

#[derive(Debug)]
struct RouteContext<'a> {
    request: &'a CommandRequest,
    request_size: usize,
}

impl ConnectionHandler for DispatchConnectionHandler {
    fn handle(&self, stream: ConnectionStream) { self.dispatch(stream); }
}
