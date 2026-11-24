//! Deadline coverage for the rust-analyzer JSON-RPC reader.

use std::{sync::mpsc, time::Duration};

use super::read_response_for_id_with_deadline;
use crate::RustAnalyzerAdapterError;

/// An inbound frame that never completes times out instead of blocking the
/// adapter's request loop indefinitely.
#[test]
fn incomplete_message_times_out() {
    let (_sender, reader) = mpsc::channel();
    let mut writer = Vec::new();

    let result = read_response_for_id_with_deadline(&reader, &mut writer, 17, Duration::ZERO);

    assert!(matches!(
        result,
        Err(RustAnalyzerAdapterError::ResponseTimeout { .. })
    ));
}
