//! Synthetic request and backend builders for refactor tests.

use std::path::Path;

use weaver_cards::DEFAULT_CACHE_CAPACITY;
use weaver_config::{CapabilityMatrix, Config, SocketEndpoint};

use super::content::{FileKind, classify_file};
use crate::{
    backends::FusionBackends,
    dispatch::request::{CommandDescriptor, CommandRequest},
    semantic_provider::SemanticBackendProvider,
};

pub(crate) fn command_request(arguments: Vec<String>) -> CommandRequest {
    CommandRequest {
        command: CommandDescriptor {
            domain: String::from("act"),
            operation: String::from("refactor"),
        },
        arguments,
        patch: None,
    }
}

pub(crate) fn build_backends(socket_path: &Path) -> FusionBackends<SemanticBackendProvider> {
    let config = Config {
        daemon_socket: SocketEndpoint::unix(socket_path.to_string_lossy().as_ref()),
        ..Config::default()
    };
    let provider =
        SemanticBackendProvider::new(CapabilityMatrix::default(), DEFAULT_CACHE_CAPACITY);
    FusionBackends::new(config, provider)
}

/// Builds a rename command argument vector with explicit provider selection.
///
/// Includes deterministic `--position` anchors: `1:1` for Python and fallback
/// files, and `2:9` for Rust where the fixture symbol starts on the second
/// line.
pub(crate) fn standard_rename_args_for_provider(file: &str, provider: &str) -> Vec<String> {
    let position = match classify_file(Path::new(file)) {
        FileKind::Rust => "2:9",
        _ => "1:1",
    };
    vec![
        String::from("--provider"),
        String::from(provider),
        String::from("--refactoring"),
        String::from("rename"),
        String::from("--file"),
        String::from(file),
        String::from("--position"),
        String::from(position),
        String::from("new_name=woven"),
    ]
}

pub(crate) fn configure_request(request: &mut CommandRequest, args: Vec<String>) {
    *request = command_request(args);
}
