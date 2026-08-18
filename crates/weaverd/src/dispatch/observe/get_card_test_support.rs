//! Fixtures and fallible helpers shared by the `observe get-card` tests.
//!
//! Every helper propagates failure to the calling test so panics stay in test
//! bodies, where the reported line number identifies the failing case.

use std::path::PathBuf;

use rstest::fixture;
use serde_json::Value;
use tempfile::TempDir;
use weaver_cards::{DEFAULT_CACHE_CAPACITY, DetailLevel};
use weaver_config::{CapabilityMatrix, Config, SocketEndpoint};
use weaver_test_macros::allow_fixture_expansion_lints;

use crate::{
    backends::FusionBackends,
    dispatch::{
        observe::get_card::handle,
        request::CommandRequest,
        response::ResponseWriter,
        router::DispatchResult,
    },
    semantic_provider::SemanticBackendProvider,
    tests::support::fs as test_fs,
};

#[allow_fixture_expansion_lints]
#[fixture]
pub(crate) fn temp_dir() -> Result<TempDir, String> {
    TempDir::new().map_err(|error| format!("temp dir: {error}"))
}

#[fixture]
pub(crate) fn backends() -> Result<(FusionBackends<SemanticBackendProvider>, TempDir), String> {
    let dir = TempDir::new().map_err(|error| format!("create temp dir: {error}"))?;
    let socket_path = dir
        .path()
        .join("socket.sock")
        .to_string_lossy()
        .into_owned();
    let config = Config {
        daemon_socket: SocketEndpoint::unix(socket_path),
        ..Config::default()
    };
    let provider =
        SemanticBackendProvider::new(CapabilityMatrix::default(), DEFAULT_CACHE_CAPACITY);
    Ok((FusionBackends::new(config, provider), dir))
}

#[derive(Clone, Copy)]
pub(crate) struct SourceFile<'a> {
    pub(crate) name: &'a str,
    pub(crate) content: &'a str,
}

pub(crate) fn write_source(temp_dir: &TempDir, file: SourceFile<'_>) -> Result<PathBuf, String> {
    let path = temp_dir.path().join(file.name);
    test_fs::write(&path, file.content).map_err(|error| format!("write source: {error}"))?;
    Ok(path)
}

/// Renders the wire name for a detail level, rejecting unmodelled variants.
fn detail_value(detail: DetailLevel) -> Result<&'static str, String> {
    match detail {
        DetailLevel::Minimal => Ok("minimal"),
        DetailLevel::Signature => Ok("signature"),
        DetailLevel::Structure => Ok("structure"),
        DetailLevel::Semantic => Ok("semantic"),
        DetailLevel::Full => Ok("full"),
        other => Err(format!("unexpected DetailLevel variant: {other:?}")),
    }
}

pub(crate) fn make_request(
    uri: &str,
    line: u32,
    column: u32,
    detail: DetailLevel,
) -> Result<CommandRequest, String> {
    let detail_str = detail_value(detail)?;
    let json = format!(
        concat!(
            "{{\"command\":{{\"domain\":\"observe\",\"operation\":\"get-card\"}},",
            "\"arguments\":[\"--uri\",\"{uri}\",\"--position\",\"{line}:{column}\",",
            "\"--detail\",\"{detail}\"]}}"
        ),
        uri = uri,
        line = line,
        column = column,
        detail = detail_str,
    );
    CommandRequest::parse(json.as_bytes()).map_err(|error| format!("request: {error}"))
}

pub(crate) fn response_payload(output: Vec<u8>) -> Result<Value, String> {
    let response = String::from_utf8(output).map_err(|error| format!("utf8: {error}"))?;
    let stream_line = response
        .lines()
        .next()
        .ok_or_else(|| "stream line".to_string())?;
    let envelope: Value =
        serde_json::from_str(stream_line).map_err(|error| format!("envelope: {error}"))?;
    let data = envelope["data"]
        .as_str()
        .ok_or_else(|| "stdout data".to_string())?;
    serde_json::from_str(data).map_err(|error| format!("payload: {error}"))
}

pub(crate) fn dispatch_payload(
    request: &CommandRequest,
    backends: &mut FusionBackends<SemanticBackendProvider>,
) -> Result<(DispatchResult, Value), String> {
    let mut output = Vec::new();
    let mut writer = ResponseWriter::new(&mut output);
    let result = handle(request, &mut writer, backends)
        .map_err(|error| format!("handler should succeed: {error}"))?;
    response_payload(output).map(|payload| (result, payload))
}

/// Position and detail level a get-card dispatch probes at.
pub(crate) struct CardProbe {
    pub(crate) position: (u32, u32),
    pub(crate) detail: DetailLevel,
}

/// Writes `file`, then dispatches a get-card request against it.
pub(crate) fn dispatch_source(
    backends: &mut FusionBackends<SemanticBackendProvider>,
    temp_dir: &TempDir,
    file: SourceFile<'_>,
    probe: CardProbe,
) -> Result<(DispatchResult, Value), String> {
    let path = write_source(temp_dir, file)?;
    let uri = url::Url::from_file_path(&path)
        .map_err(|()| "file uri".to_string())?
        .to_string();
    let (line, column) = probe.position;
    let request = make_request(&uri, line, column, probe.detail)?;
    dispatch_payload(&request, backends)
}
