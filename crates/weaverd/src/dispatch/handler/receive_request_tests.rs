//! Tests for dispatch request receiving and validation.

use std::{io::Write, net::TcpStream, thread};

use super::{
    tests_helpers::{backend_manager, create_listener},
    *,
};
use crate::transport::ConnectionStream;

fn receive_request_handler() -> Result<(DispatchConnectionHandler, tempfile::TempDir), String> {
    let temp_dir = tempfile::TempDir::new().map_err(|error| format!("temp dir: {error}"))?;
    let workspace_root = temp_dir.path().join("workspace");
    let endpoint = temp_dir.path().join("weaverd-test/socket.sock");
    let handler = DispatchConnectionHandler::new(
        backend_manager()?,
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
    let request = request.to_vec();
    let sender = thread::spawn(move || -> Result<(), String> {
        let mut stream = TcpStream::connect(addr).map_err(|error| format!("connect: {error}"))?;
        stream
            .write_all(&request)
            .map_err(|error| format!("write request: {error}"))?;
        stream.flush().map_err(|error| format!("flush: {error}"))?;
        Ok(())
    });

    let (stream, _) = listener
        .accept()
        .map_err(|error| format!("accept: {error}"))?;
    let mut stream = ConnectionStream::Tcp(stream);
    let result = handler.receive_request(&mut stream);
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

    assert_eq!(result.1.domain(), "observe");
    assert_eq!(result.1.operation(), "get-card");
    assert!(!result.0.is_empty());
    Ok(())
}

#[test]
fn receive_request_rejects_malformed_json() -> Result<(), String> {
    let (handler, _temp_dir) = receive_request_handler()?;
    let result = receive_request_from_bytes(&handler, b"not-json\n")?;

    assert!(matches!(
        result,
        Err(ReadRequestError::BadRequest(
            DispatchError::MalformedJsonl { .. }
        ))
    ));
    Ok(())
}

#[test]
fn receive_request_rejects_invalid_request() -> Result<(), String> {
    let (handler, _temp_dir) = receive_request_handler()?;
    let result = receive_request_from_bytes(
        &handler,
        b"{\"command\":{\"domain\":\"observe\",\"operation\":\"\"}}\n",
    )?;

    assert!(matches!(
        result,
        Err(ReadRequestError::BadRequest(
            DispatchError::InvalidStructure { .. }
        ))
    ));
    Ok(())
}

#[test]
fn receive_request_rejects_oversized_request() -> Result<(), String> {
    let (handler, _temp_dir) = receive_request_handler()?;
    let request = vec![b'a'; weaver_daemon_types::JSONL_REQUEST_MAX_LINE_BYTES + 1];
    let result = receive_request_from_bytes(&handler, &request)?;

    assert!(matches!(
        result,
        Err(ReadRequestError::BadRequest(
            DispatchError::RequestTooLarge { .. }
        ))
    ));
    Ok(())
}
