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

/// Failure modes when reading and validating the next request.
#[derive(Debug)]
enum ReadRequestError {
    /// The peer closed the connection before sending a request line.
    ClientDisconnected,
    /// The request could not be read, parsed, or validated.
    BadRequest(DispatchError),
}

/// Connection handler that parses and dispatches JSONL commands.
///
/// Each connection is handled synchronously: the handler reads a single JSONL
/// request line, parses it, routes it to domain handlers, and writes the
/// response stream before closing the connection.
#[derive(Debug)]
pub struct DispatchConnectionHandler {
    /// Router that maps validated commands to domain handlers.
    router: DomainRouter,
    /// Shared backend manager consulted while routing.
    backends: BackendManager,
    /// Endpoint label recorded in structured log events.
    endpoint: String,
    /// Runtime directory whose health snapshot accompanies each request.
    runtime_dir: PathBuf,
}

impl DispatchConnectionHandler {
    /// Creates a new dispatch handler with a backend manager and workspace root.
    ///
    /// # Errors
    ///
    /// Returns a `DispatchError` when `DomainRouter` cannot accept the supplied
    /// workspace root.
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

    /// Reads, routes, and answers a single request on the connection.
    fn dispatch(&self, mut stream: ConnectionStream) {
        let (request_bytes, request) = match self.receive_request(&mut stream) {
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

        self.route_request(&request, request_bytes.len(), &mut writer);
    }

    /// Reads one JSONL request line and validates the parsed command.
    fn receive_request(
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

    /// Routes a validated request and writes its buffered response.
    fn route_request<W: std::io::Write>(
        &self,
        request: &CommandRequest,
        request_size: usize,
        writer: &mut ResponseWriter<W>,
    ) {
        let mut response = Vec::new();
        let route_result = self.backends.with_backends(|backends| {
            let mut buffered_writer = ResponseWriter::new(&mut response);
            self.router.route(request, &mut buffered_writer, backends)
        });
        let context = Self::request_context(request, request_size);

        match route_result {
            Ok(Ok(result)) => {
                if self.write_buffered_response(&context, writer, &response) {
                    self.write_exit_status(&context, result.status, writer);
                }
            }
            Ok(Err(error)) => {
                emit_structured_event(
                    &self.with_metadata(&context, "dispatch_failed"),
                    "request dispatch failed",
                    true,
                );
                tracing::warn!(target: DISPATCH_TARGET, %error, "request dispatch failed");
                self.write_error_response(&context, writer, &error);
            }
            Err(error) => {
                emit_structured_event(
                    &self.with_metadata(&context, "dispatch_infra_error"),
                    "dispatch infrastructure error",
                    true,
                );
                tracing::warn!(target: DISPATCH_TARGET, %error, "dispatch infrastructure error");
                self.write_error_response(&context, writer, &error);
            }
        }
    }

    /// Writes buffered domain output and reports whether it succeeded.
    fn write_buffered_response<W: std::io::Write>(
        &self,
        context: &RouteContext<'_>,
        writer: &mut ResponseWriter<W>,
        response: &[u8],
    ) -> bool {
        if let Err(transport_error) = writer.write_buffered(response) {
            tracing::warn!(
                target: DISPATCH_TARGET,
                endpoint = %self.endpoint,
                domain = context.request.domain(),
                operation = context.request.operation(),
                request_size = context.request_size,
                transport_error = %transport_error,
                "failed to write routed response"
            );
            return false;
        }
        true
    }

    /// Borrows the request and its size for route logging.
    const fn request_context(request: &CommandRequest, request_size: usize) -> RouteContext<'_> {
        RouteContext {
            request,
            request_size,
        }
    }

    /// Builds a structured event carrying the routed request metadata.
    fn with_metadata(
        &self,
        context: &RouteContext<'_>,
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

    /// Writes the command exit status, logging any transport failure.
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

    /// Writes a dispatch error response, logging any transport failure.
    fn write_error_response<W: std::io::Write>(
        &self,
        context: &RouteContext<'_>,
        writer: &mut ResponseWriter<W>,
        response_error: &DispatchError,
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
                "failed to write error response"
            );
        }
    }
}

/// Borrowed request and size used while logging a routed response.
#[derive(Debug)]
struct RouteContext<'a> {
    /// Request being routed.
    request: &'a CommandRequest,
    /// Size of the received request line in bytes.
    request_size: usize,
}

impl ConnectionHandler for DispatchConnectionHandler {
    fn handle(&self, stream: ConnectionStream) { self.dispatch(stream); }
}
