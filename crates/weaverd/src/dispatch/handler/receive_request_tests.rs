//! Tests for dispatch request receiving and validation.

use std::{io::Write, net::TcpStream, thread};

use rstest::rstest;

use super::{
    tests_helpers::{backend_manager, create_listener, ensure},
    *,
};
use crate::transport::ConnectionStream;

#[derive(Debug, Clone, Copy)]
enum ExpectedDispatchError {
    MalformedJsonl,
    InvalidStructure,
    RequestTooLarge,
}

fn receive_request_handler() -> Result<(DispatchConnectionHandler, tempfile::TempDir), String> {
    let temp_dir = tempfile::TempDir::new().map_err(|error| format!("temp dir: {error}"))?;
    let workspace_root = temp_dir.path().join("workspace");
    let endpoint = temp_dir.path().join("weaverd-test/socket.sock");
    let handler = DispatchConnectionHandler::new(
        backend_manager()?.manager(),
        workspace_root,
        endpoint.to_string_lossy().into_owned(),
        temp_dir.path().to_path_buf(),
    )
    .map_err(|error| format!("create handler: {error}"))?;
    Ok((handler, temp_dir))
}

fn receive_request_from_bytes(
    handler: &DispatchConnectionHandler,
    request: &[u8],
) -> Result<Result<(Vec<u8>, CommandRequest), ReadRequestError>, String> {
    let (listener, addr) = create_listener()?;
    let request_bytes = request.to_vec();
    let sender = thread::spawn(move || -> Result<(), String> {
        let mut stream = TcpStream::connect(addr).map_err(|error| format!("connect: {error}"))?;
        stream
            .write_all(&request_bytes)
            .map_err(|error| format!("write request: {error}"))?;
        stream.flush().map_err(|error| format!("flush: {error}"))?;
        Ok(())
    });

    let (stream, _) = listener
        .accept()
        .map_err(|error| format!("accept: {error}"))?;
    let mut connection = ConnectionStream::Tcp(stream);
    let result = handler.receive_request(&mut connection);
    sender
        .join()
        .map_err(|error| format!("join sender: {error:?}"))??;
    Ok(result)
}

#[test]
fn receive_request_accepts_valid_request() -> Result<(), String> {
    let (handler, _temp_dir) = receive_request_handler()?;
    let result = receive_request_from_bytes(
        &handler,
        b"{\"command\":{\"domain\":\"observe\",\"operation\":\"get-card\"}}\n",
    )?
    .map_err(|error| format!("receive request: {error:?}"))?;

    ensure(
        result.1.domain() == "observe",
        format!("expected domain observe, got {}", result.1.domain()),
    )?;
    ensure(
        result.1.operation() == "get-card",
        format!("expected operation get-card, got {}", result.1.operation()),
    )?;
    ensure(!result.0.is_empty(), "expected non-empty response bytes")?;
    Ok(())
}

#[rstest]
#[case::malformed_json(b"not-json\n".to_vec(), ExpectedDispatchError::MalformedJsonl)]
#[case::invalid_request(
    b"{\"command\":{\"domain\":\"observe\",\"operation\":\"\"}}\n".to_vec(),
    ExpectedDispatchError::InvalidStructure
)]
#[case::oversized_request(
    vec![b'a'; weaver_daemon_types::JSONL_REQUEST_MAX_LINE_BYTES + 1],
    ExpectedDispatchError::RequestTooLarge
)]
fn receive_request_rejects_bad_requests(
    #[case] payload: Vec<u8>,
    #[case] expected: ExpectedDispatchError,
) -> Result<(), String> {
    let (handler, _temp_dir) = receive_request_handler()?;
    let result = receive_request_from_bytes(&handler, &payload)?;

    match expected {
        ExpectedDispatchError::MalformedJsonl => ensure(
            matches!(
                result,
                Err(ReadRequestError::BadRequest(
                    DispatchError::MalformedJsonl { .. }
                ))
            ),
            "expected a malformed JSONL rejection",
        )?,
        ExpectedDispatchError::InvalidStructure => ensure(
            matches!(
                result,
                Err(ReadRequestError::BadRequest(
                    DispatchError::InvalidStructure { .. }
                ))
            ),
            "expected an invalid-structure rejection",
        )?,
        ExpectedDispatchError::RequestTooLarge => ensure(
            matches!(
                result,
                Err(ReadRequestError::BadRequest(
                    DispatchError::RequestTooLarge { .. }
                ))
            ),
            "expected a request-too-large rejection",
        )?,
    }
    Ok(())
}
