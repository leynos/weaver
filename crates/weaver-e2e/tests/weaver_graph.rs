//! End-to-end tests for weaver-graph using the Pyrefly LSP backend.
//!
//! These tests exercise the weaver-graph LSP provider with a real language
//! server. Tests are skipped gracefully if Pyrefly is not available.

use std::path::Path;

use camino::Utf8PathBuf;
use lsp_types::{
    CallHierarchyIncomingCallsParams, CallHierarchyOutgoingCallsParams, CallHierarchyPrepareParams,
    Uri,
};
use tempfile::TempDir;
use url::Url;

use weaver_e2e::fixtures;
use weaver_e2e::lsp_client::{LspClient, LspClientError};
use weaver_e2e::pyrefly_available;
use weaver_graph::{
    CallGraphProvider, CallHierarchyClient, GraphError, LspCallGraphProvider, SourcePosition,
};

#[derive(Debug, thiserror::Error)]
enum TestError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("LSP client error: {0}")]
    LspClient(#[from] LspClientError),

    #[error("graph error: {0}")]
    Graph(#[from] GraphError),

    #[error("invalid file path: cannot convert to URI")]
    InvalidFilePath,

    #[error("invalid URI: {0}")]
    InvalidUri(String),

    #[error("invalid utf-8 path for file: {0}")]
    InvalidUtf8Path(String),

    #[error("missing expected node: {0}")]
    MissingNode(String),

    #[error("missing expected edge: {caller} -> {callee}")]
    MissingEdge { caller: String, callee: String },
}

fn file_uri(path: &Path) -> Result<Uri, TestError> {
    let url = Url::from_file_path(path).map_err(|()| TestError::InvalidFilePath)?;
    url.as_str()
        .parse()
        .map_err(|_| TestError::InvalidUri(url.to_string()))
}

fn to_utf8_path(path: &Path) -> Result<Utf8PathBuf, TestError> {
    Utf8PathBuf::try_from(path.to_path_buf())
        .map_err(|_| TestError::InvalidUtf8Path(path.display().to_string()))
}

macro_rules! require_pyrefly {
    () => {
        if !pyrefly_available() {
            eprintln!(
                "Skipping test: Pyrefly not available (install with `uv tool install pyrefly`)"
            );
            return Ok(());
        }
    };
}

struct LspClientAdapter {
    client: LspClient,
}

impl Drop for LspClientAdapter {
    fn drop(&mut self) {
        drop(self.client.shutdown());
    }
}

impl CallHierarchyClient for LspClientAdapter {
    fn prepare_call_hierarchy(
        &mut self,
        params: CallHierarchyPrepareParams,
    ) -> Result<Option<Vec<lsp_types::CallHierarchyItem>>, GraphError> {
        self.client
            .prepare_call_hierarchy(params)
            .map_err(|err| GraphError::validation(err.to_string()))
    }

    fn incoming_calls(
        &mut self,
        params: CallHierarchyIncomingCallsParams,
    ) -> Result<Option<Vec<lsp_types::CallHierarchyIncomingCall>>, GraphError> {
        self.client
            .incoming_calls(params)
            .map_err(|err| GraphError::validation(err.to_string()))
    }

    fn outgoing_calls(
        &mut self,
        params: CallHierarchyOutgoingCallsParams,
    ) -> Result<Option<Vec<lsp_types::CallHierarchyOutgoingCall>>, GraphError> {
        self.client
            .outgoing_calls(params)
            .map_err(|err| GraphError::validation(err.to_string()))
    }
}

struct GraphTestContext {
    provider: LspCallGraphProvider<LspClientAdapter>,
    file_path: Utf8PathBuf,
    _temp_dir: TempDir,
}

impl GraphTestContext {
    fn new(fixture: &str) -> Result<Self, TestError> {
        let temp_dir = TempDir::new()?;
        let file_path = temp_dir.path().join("test.py");
        std::fs::write(&file_path, fixture)?;

        let root_uri = file_uri(temp_dir.path())?;
        let file_uri_val = file_uri(&file_path)?;

        let mut client = LspClient::spawn("uvx", &["pyrefly", "lsp"])?;
        client.initialize(root_uri)?;
        client.did_open(file_uri_val, "python", fixture)?;

        let adapter = LspClientAdapter { client };
        let provider = LspCallGraphProvider::new(adapter);

        Ok(Self {
            provider,
            file_path: to_utf8_path(&file_path)?,
            _temp_dir: temp_dir,
        })
    }

    fn source_position(&self, line: u32, column: u32) -> SourcePosition {
        SourcePosition::new(self.file_path.clone(), line, column)
    }
}

fn assert_node(graph: &weaver_graph::CallGraph, name: &str) -> Result<(), TestError> {
    if graph.find_by_name(name).is_some() {
        Ok(())
    } else {
        Err(TestError::MissingNode(name.to_owned()))
    }
}

fn assert_edge(
    graph: &weaver_graph::CallGraph,
    source: &str,
    target: &str,
) -> Result<(), TestError> {
    let source_node = graph
        .find_by_name(source)
        .ok_or_else(|| TestError::MissingNode(source.to_owned()))?;
    let target_node = graph
        .find_by_name(target)
        .ok_or_else(|| TestError::MissingNode(target.to_owned()))?;

    let has_edge = graph
        .edges()
        .any(|edge| edge.caller() == source_node.id() && edge.callee() == target_node.id());

    if has_edge {
        Ok(())
    } else {
        Err(TestError::MissingEdge {
            caller: source.to_owned(),
            callee: target.to_owned(),
        })
    }
}

#[test]
fn call_graph_builds_from_pyrefly() -> Result<(), TestError> {
    require_pyrefly!();

    let mut context = GraphTestContext::new(fixtures::LINEAR_CHAIN)?;
    let position = context.source_position(0, 4);
    let graph = context.provider.build_graph(&position, 2)?;

    assert_node(&graph, "a")?;
    assert_node(&graph, "b")?;
    assert_node(&graph, "c")?;
    assert_edge(&graph, "a", "b")?;
    assert_edge(&graph, "b", "c")?;
    Ok(())
}
