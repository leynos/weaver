//! Fixtures and fallible helpers shared by the `observe graph-slice` tests.
//!
//! Every helper here propagates failure to the calling test so panics stay in
//! test bodies, where the reported line number is useful.

use camino::Utf8PathBuf;
use rstest::fixture;
use serde_json::Value;
use tempfile::TempDir;
use url::Url;
use weaver_cards::{DEFAULT_CACHE_CAPACITY, DetailLevel};
use weaver_config::{CapabilityMatrix, Config, SocketEndpoint};

use crate::{
    backends::FusionBackends,
    dispatch::{observe::graph_slice::handle, request::CommandRequest, response::ResponseWriter},
    semantic_provider::SemanticBackendProvider,
    tests::support::fs as test_fs,
};

#[fixture]
pub(crate) fn backends_fixture()
-> Result<(FusionBackends<SemanticBackendProvider>, TempDir), String> {
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

pub(crate) fn write_source(
    temp_dir: &TempDir,
    name: &str,
    content: &str,
) -> Result<Utf8PathBuf, String> {
    let path = temp_dir.path().join(name);
    let path = Utf8PathBuf::from_path_buf(path)
        .map_err(|path| format!("fixture path should be UTF-8: {path:?}"))?;
    test_fs::write(&path, content).map_err(|error| error.to_string())?;
    Ok(path)
}

pub(crate) fn make_request(arguments: &[&str]) -> Result<CommandRequest, String> {
    let payload = serde_json::json!({
        "command": {"domain": "observe", "operation": "graph-slice"},
        "arguments": arguments,
    });
    let json =
        serde_json::to_vec(&payload).map_err(|error| format!("serialise request: {error}"))?;
    CommandRequest::parse(&json).map_err(|error| format!("request: {error}"))
}

pub(crate) fn response_payload(output: Vec<u8>) -> Result<Value, String> {
    let response = String::from_utf8(output).map_err(|error| format!("utf8: {error}"))?;
    let mut stdout_data = None;
    for line in response.lines() {
        let envelope: Value =
            serde_json::from_str(line).map_err(|error| format!("envelope: {error}"))?;
        if envelope.get("stream").and_then(|value| value.as_str()) == Some("stdout") {
            stdout_data = Some(
                envelope
                    .get("data")
                    .and_then(|value| value.as_str())
                    .ok_or_else(|| "stdout data".to_string())?
                    .to_owned(),
            );
            break;
        }
    }
    let data = stdout_data.ok_or_else(|| "no stdout envelope".to_string())?;
    serde_json::from_str(&data).map_err(|error| format!("payload: {error}"))
}

pub(crate) fn detail_value(detail: DetailLevel) -> &'static str {
    match detail {
        DetailLevel::Minimal => "minimal",
        DetailLevel::Signature => "signature",
        DetailLevel::Structure => "structure",
        DetailLevel::Semantic => "semantic",
        DetailLevel::Full => "full",
        _ => "full",
    }
}

pub(crate) fn dispatch_payload(
    request: &CommandRequest,
    backends: &mut FusionBackends<SemanticBackendProvider>,
) -> Result<(i32, Value), String> {
    let mut output = Vec::new();
    let mut writer = ResponseWriter::new(&mut output);
    let result = handle(request, &mut writer, backends)
        .map_err(|error| format!("dispatch fails: {error}"))?;
    response_payload(output).map(|payload| (result.status, payload))
}

/// A source fixture plus the graph-slice arguments that follow `--uri`.
pub(crate) struct SourceRequest<'a> {
    pub(crate) temp_dir: &'a TempDir,
    pub(crate) filename: &'a str,
    pub(crate) content: &'a str,
    pub(crate) arguments: &'a [&'a str],
}

/// Writes the fixture and builds the request that targets it.
pub(crate) fn prepare_request(source: &SourceRequest<'_>) -> Result<CommandRequest, String> {
    let path = write_source(source.temp_dir, source.filename, source.content)?;
    let uri = Url::from_file_path(&path)
        .map_err(|()| "file uri".to_string())?
        .to_string();
    let mut arguments = vec!["--uri", uri.as_str()];
    arguments.extend_from_slice(source.arguments);
    make_request(&arguments)
}

/// Writes the fixture, dispatches it, and returns the status and payload.
pub(crate) fn dispatch_source(
    backends: &mut FusionBackends<SemanticBackendProvider>,
    source: &SourceRequest<'_>,
) -> Result<(i32, Value), String> {
    let request = prepare_request(source)?;
    dispatch_payload(&request, backends)
}
