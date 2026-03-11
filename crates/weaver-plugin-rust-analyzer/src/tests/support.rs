//! Shared test helpers for rust-analyzer plugin unit tests.

use std::collections::HashMap;
use std::path::PathBuf;

use mockall::mock;
use weaver_plugins::protocol::{FilePayload, PluginRequest};

use crate::{ByteOffset, RustAnalyzerAdapter, RustAnalyzerAdapterError};

mock! {
    pub(crate) Adapter {}
    impl RustAnalyzerAdapter for Adapter {
        fn rename(
            &self,
            file: &FilePayload,
            offset: ByteOffset,
            new_name: &str,
        ) -> Result<String, RustAnalyzerAdapterError>;
    }
}

/// Builds a `MockAdapter` that expects a single rename call returning `result`.
pub(crate) fn adapter_returning(result: Result<String, RustAnalyzerAdapterError>) -> MockAdapter {
    adapter_returning_with_path(result, None)
}

/// Builds a `MockAdapter` that can also assert the forwarded payload path.
pub(crate) fn adapter_returning_with_path(
    result: Result<String, RustAnalyzerAdapterError>,
    expected_payload_path: Option<&str>,
) -> MockAdapter {
    let expected_path_string = expected_payload_path.map(String::from);
    let mut adapter = MockAdapter::new();
    adapter
        .expect_rename()
        .once()
        .return_once(move |file, offset, new_name| {
            if let Some(path) = &expected_path_string {
                assert_eq!(file.path(), PathBuf::from(path).as_path());
            }
            assert_eq!(offset, ByteOffset::new(3));
            assert_eq!(new_name, "new_name");
            result
        });
    adapter
}

/// Builds a `MockAdapter` where rename is never expected.
pub(crate) fn adapter_unused() -> MockAdapter {
    MockAdapter::new()
}

/// Returns a valid `rename-symbol` argument map.
pub(crate) fn rename_arguments() -> HashMap<String, serde_json::Value> {
    let mut arguments = HashMap::new();
    arguments.insert(
        String::from("uri"),
        serde_json::Value::String(String::from("file:///src/main.rs")),
    );
    arguments.insert(
        String::from("position"),
        serde_json::Value::String(String::from("3")),
    );
    arguments.insert(
        String::from("new_name"),
        serde_json::Value::String(String::from("new_name")),
    );
    arguments
}

/// Builds a request with a single Rust file payload.
pub(crate) fn request_with_args(arguments: HashMap<String, serde_json::Value>) -> PluginRequest {
    PluginRequest::with_arguments(
        "rename-symbol",
        vec![FilePayload::new(
            PathBuf::from("src/main.rs"),
            "fn old_name() -> i32 {\n    1\n}\n",
        )],
        arguments,
    )
}

/// Builds a request using the provided file payload path.
pub(crate) fn request_with_path(path: &str) -> PluginRequest {
    PluginRequest::with_arguments(
        "rename-symbol",
        vec![FilePayload::new(
            PathBuf::from(path),
            "fn old_name() -> i32 {\n    1\n}\n",
        )],
        rename_arguments(),
    )
}
