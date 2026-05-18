//! Tests for dispatch request receiving and validation.

use std::{io::Write, net::TcpStream, thread};

use rstest::rstest;

use super::{
    tests_helpers::{backend_manager, create_listener},
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
        ExpectedDispatchError::MalformedJsonl => assert!(matches!(
            result,
            Err(ReadRequestError::BadRequest(
                DispatchError::MalformedJsonl { .. }
            ))
        )),
        ExpectedDispatchError::InvalidStructure => assert!(matches!(
            result,
            Err(ReadRequestError::BadRequest(
                DispatchError::InvalidStructure { .. }
            ))
        )),
        ExpectedDispatchError::RequestTooLarge => assert!(matches!(
            result,
            Err(ReadRequestError::BadRequest(
                DispatchError::RequestTooLarge { .. }
            ))
        )),
    }
    Ok(())
}
