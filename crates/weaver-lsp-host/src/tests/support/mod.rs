//! Shared fixtures and helpers for host tests.

mod recording_server;
mod world;

use std::str::FromStr;

use lsp_types::Uri;
use rstest::fixture;

pub use recording_server::{CallKind, DocumentSyncErrors, RecordingLanguageServer, ResponseSet};
pub use world::{TestServerConfig, TestWorld};

/// Common URI used by host tests.
#[fixture]
pub fn sample_uri() -> Uri {
    Uri::from_str("file:///workspace/main.rs").expect("invalid test URI")
}
