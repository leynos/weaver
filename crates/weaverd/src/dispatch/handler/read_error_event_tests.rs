//! Tests for read-error structured dispatch event mapping.

use super::{
    structured_event::{read_error_event, serialize_structured_event},
    *,
};
use crate::backends::{BackendKind, BackendStartupError};

#[test]
fn read_error_event_maps_all_dispatch_errors() {
    let temp_dir = std::env::temp_dir();
    let endpoint = temp_dir.join("weaverd.sock").to_string_lossy().into_owned();
    let cases = vec![
        (DispatchError::malformed("bad json"), "request_rejected"),
        (
            DispatchError::invalid_structure("missing command"),
            "request_rejected",
        ),
        (DispatchError::unknown_domain("bogus"), "request_rejected"),
        (
            DispatchError::unknown_operation("observe", "bogus", &["get-card"]),
            "request_rejected",
        ),
        (
            DispatchError::request_too_large(
                weaver_daemon_types::JSONL_REQUEST_MAX_LINE_BYTES + 1,
                weaver_daemon_types::JSONL_REQUEST_MAX_LINE_BYTES,
            ),
            "request_too_large",
        ),
        (
            DispatchError::Io(std::io::Error::other("read failed")),
            "request_rejected",
        ),
        (
            DispatchError::SerializeResponse(
                serde_json::from_str::<serde_json::Value>("{").expect_err("invalid JSON"),
            ),
            "request_rejected",
        ),
        (
            DispatchError::invalid_arguments("missing args"),
            "request_rejected",
        ),
        (
            DispatchError::backend_startup(BackendStartupError::new(
                BackendKind::Semantic,
                "startup failed",
            )),
            "request_rejected",
        ),
        (
            DispatchError::lsp_host("rust", "request failed"),
            "request_rejected",
        ),
        (
            DispatchError::unsupported_language("txt"),
            "request_rejected",
        ),
        (DispatchError::internal("lock poisoned"), "request_rejected"),
    ];

    for (error, expected_event) in cases {
        let event = read_error_event(&error, &endpoint, &temp_dir);
        let value = serialize_structured_event(&event);
        assert_eq!(value["event"], serde_json::json!(expected_event));
    }
}
