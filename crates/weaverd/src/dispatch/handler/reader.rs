//! Bounded request-line readers used by the dispatch handler.

use std::io::{self, Read};

use weaver_daemon_types::JSONL_REQUEST_MAX_LINE_BYTES;

use crate::{dispatch::errors::DispatchError, transport::ConnectionStream};

/// Reads a bounded JSONL request line from the stream.
///
/// Returns `Ok(None)` if the client disconnects without sending data.
/// Returns `Ok(Some(bytes))` when a complete line (or EOF with partial data)
/// is received. Returns an error if reading fails or the request exceeds the
/// maximum size.
pub(super) fn read_request_line(
    stream: &mut ConnectionStream,
) -> Result<Option<Vec<u8>>, DispatchError> {
    let mut buffer = Vec::new();
    let mut chunk = [0_u8; 1024];

    loop {
        let bytes_read = read_with_retry(stream, &mut chunk)?;
        if bytes_read == 0 {
            return Ok(finish_request_line(buffer));
        }
        if append_request_chunk(&mut buffer, &chunk[..bytes_read])? {
            return Ok(Some(buffer));
        }
    }
}

pub(super) fn read_error_message(error: &DispatchError) -> &'static str {
    if matches!(error, DispatchError::RequestTooLarge { .. }) {
        "request rejected: size exceeded"
    } else {
        "failed to read request"
    }
}

/// Reads from the stream, retrying on interrupts.
fn read_with_retry(stream: &mut ConnectionStream, buf: &mut [u8]) -> io::Result<usize> {
    loop {
        match stream.read(buf) {
            Ok(n) => return Ok(n),
            Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
            Err(e) => return Err(e),
        }
    }
}

fn finish_request_line(buffer: Vec<u8>) -> Option<Vec<u8>> {
    (!buffer.is_empty()).then_some(buffer)
}

/// Enforces the maximum request size limit.
fn enforce_limit(size: usize) -> Result<(), DispatchError> {
    if size > JSONL_REQUEST_MAX_LINE_BYTES {
        return Err(DispatchError::request_too_large(
            size,
            JSONL_REQUEST_MAX_LINE_BYTES,
        ));
    }
    Ok(())
}

fn append_request_chunk(buffer: &mut Vec<u8>, chunk: &[u8]) -> Result<bool, DispatchError> {
    let Some(newline_pos) = chunk.iter().position(|byte| *byte == b'\n') else {
        buffer.extend_from_slice(chunk);
        enforce_limit(buffer.len())?;
        return Ok(false);
    };
    buffer.extend_from_slice(&chunk[..=newline_pos]);
    enforce_limit(buffer.len())?;
    Ok(true)
}
