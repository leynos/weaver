//! Unit tests for the LSP call graph provider.

use crate::provider::{
    CallGraphProvider, CallHierarchyClient, LspCallGraphProvider, SourcePosition,
};
use crate::{CallGraph, GraphError};
use lsp_types::{
    CallHierarchyIncomingCall, CallHierarchyIncomingCallsParams, CallHierarchyItem,
    CallHierarchyOutgoingCall, CallHierarchyOutgoingCallsParams, CallHierarchyPrepareParams,
    Position, Range, SymbolKind, Uri,
};
use std::str::FromStr;
use std::sync::{Arc, Mutex};

#[derive(Clone, Copy, Debug)]
enum ErrorKind {
    Validation,
}

impl ErrorKind {
    fn to_error(self) -> GraphError {
        match self {
            Self::Validation => GraphError::validation("test failure"),
        }
    }
}

#[derive(Clone, Debug)]
enum Response<T: Clone> {
    Ok(Option<Vec<T>>),
    Err(ErrorKind),
}

impl<T: Clone> Response<T> {
    fn as_result(&self) -> Result<Option<Vec<T>>, GraphError> {
        match self {
            Self::Ok(value) => Ok(value.clone()),
            Self::Err(kind) => Err(kind.to_error()),
        }
    }
}

#[derive(Debug, Default)]
struct CallCounts {
    incoming: usize,
    outgoing: usize,
}

#[derive(Debug, Clone)]
struct TestClient {
    prepare: Response<CallHierarchyItem>,
    incoming: Response<CallHierarchyIncomingCall>,
    outgoing: Response<CallHierarchyOutgoingCall>,
    counts: Arc<Mutex<CallCounts>>,
}

impl TestClient {
    fn new(
        prepare: Response<CallHierarchyItem>,
        incoming: Response<CallHierarchyIncomingCall>,
        outgoing: Response<CallHierarchyOutgoingCall>,
        counts: Arc<Mutex<CallCounts>>,
    ) -> Self {
        Self {
            prepare,
            incoming,
            outgoing,
            counts,
        }
    }

    fn handle_call<T: Clone>(
        &mut self,
        response: &Response<T>,
        counter_update: impl FnOnce(&mut CallCounts),
        call_type: &str,
    ) -> Result<Option<Vec<T>>, GraphError> {
        let mut counts = self.counts.lock().map_err(|_| {
            GraphError::validation(format!("{call_type} call count mutex poisoned"))
        })?;
        counter_update(&mut counts);
        response.as_result()
    }
}

impl CallHierarchyClient for TestClient {
    fn prepare_call_hierarchy(
        &mut self,
        _params: CallHierarchyPrepareParams,
    ) -> Result<Option<Vec<CallHierarchyItem>>, GraphError> {
        self.prepare.as_result()
    }

    fn incoming_calls(
        &mut self,
        _params: CallHierarchyIncomingCallsParams,
    ) -> Result<Option<Vec<CallHierarchyIncomingCall>>, GraphError> {
        self.handle_call(
            &self.incoming.clone(),
            |counts| counts.incoming += 1,
            "incoming",
        )
    }

    fn outgoing_calls(
        &mut self,
        _params: CallHierarchyOutgoingCallsParams,
    ) -> Result<Option<Vec<CallHierarchyOutgoingCall>>, GraphError> {
        self.handle_call(
            &self.outgoing.clone(),
            |counts| counts.outgoing += 1,
            "outgoing",
        )
    }
}

fn test_uri() -> Uri {
    Uri::from_str("file:///src/main.rs").expect("valid URI")
}

fn range(line: u32, column: u32) -> Range {
    Range {
        start: Position::new(line, column),
        end: Position::new(line, column + 1),
    }
}

fn item(name: &str, line: u32, column: u32) -> CallHierarchyItem {
    CallHierarchyItem {
        name: name.to_owned(),
        kind: SymbolKind::FUNCTION,
        tags: None,
        detail: None,
        uri: test_uri(),
        range: range(line, column),
        selection_range: range(line, column),
        data: None,
    }
}

fn incoming_call(name: &str, line: u32, column: u32) -> CallHierarchyIncomingCall {
    CallHierarchyIncomingCall {
        from: item(name, line, column),
        from_ranges: vec![range(line + 1, column + 2)],
    }
}

fn outgoing_call(name: &str, line: u32, column: u32) -> CallHierarchyOutgoingCall {
    CallHierarchyOutgoingCall {
        to: item(name, line, column),
        from_ranges: vec![range(line + 1, column + 2)],
    }
}

fn build_graph(provider: &mut LspCallGraphProvider<TestClient>, depth: u32) -> CallGraph {
    let position = SourcePosition::new("/src/main.rs", 1, 1);
    provider
        .build_graph(&position, depth)
        .expect("graph should build")
}

fn test_build_graph_error(
    prepare_response: Response<CallHierarchyItem>,
    expected_error: impl Fn(&GraphError) -> bool,
) {
    let counts = Arc::new(Mutex::new(CallCounts::default()));
    let client = TestClient::new(
        prepare_response,
        Response::Ok(None),
        Response::Ok(None),
        counts,
    );
    let mut provider = LspCallGraphProvider::new(client);
    let position = SourcePosition::new("/src/main.rs", 1, 1);

    let err = provider
        .build_graph(&position, 1)
        .expect_err("expected graph error");

    assert!(expected_error(&err), "unexpected error: {err:?}");
}

#[test]
fn build_graph_depth_zero_skips_traversal() {
    let counts = Arc::new(Mutex::new(CallCounts::default()));
    let client = TestClient::new(
        Response::Ok(Some(vec![item("main", 1, 1)])),
        Response::Err(ErrorKind::Validation),
        Response::Err(ErrorKind::Validation),
        Arc::clone(&counts),
    );
    let mut provider = LspCallGraphProvider::new(client);

    let graph = build_graph(&mut provider, 0);

    assert_eq!(graph.node_count(), 1);
    assert_eq!(graph.edge_count(), 0);
    let call_counts = counts.lock().expect("call count mutex poisoned");
    assert_eq!(call_counts.incoming, 0);
    assert_eq!(call_counts.outgoing, 0);
}

#[test]
fn build_graph_collects_incoming_and_outgoing_edges() {
    let counts = Arc::new(Mutex::new(CallCounts::default()));
    let client = TestClient::new(
        Response::Ok(Some(vec![item("main", 1, 1)])),
        Response::Ok(Some(vec![incoming_call("caller", 3, 0)])),
        Response::Ok(Some(vec![outgoing_call("helper", 5, 0)])),
        Arc::clone(&counts),
    );
    let mut provider = LspCallGraphProvider::new(client);

    let graph = build_graph(&mut provider, 1);

    assert_eq!(graph.node_count(), 3);
    assert_eq!(graph.edge_count(), 2);

    let main = graph.find_by_name("main").expect("main node missing");
    let caller = graph.find_by_name("caller").expect("caller node missing");
    let helper = graph.find_by_name("helper").expect("helper node missing");

    assert!(
        graph
            .callers_of(main.id())
            .any(|node| node.id() == caller.id()),
        "caller edge missing"
    );
    assert!(
        graph
            .callees_of(main.id())
            .any(|node| node.id() == helper.id()),
        "callee edge missing"
    );
    let call_counts = counts.lock().expect("call count mutex poisoned");
    assert_eq!(call_counts.incoming, 1);
    assert_eq!(call_counts.outgoing, 1);
}

#[test]
fn callers_graph_uses_incoming_only() {
    let counts = Arc::new(Mutex::new(CallCounts::default()));
    let client = TestClient::new(
        Response::Ok(Some(vec![item("main", 1, 1)])),
        Response::Ok(Some(vec![incoming_call("caller", 3, 0)])),
        Response::Err(ErrorKind::Validation),
        Arc::clone(&counts),
    );
    let mut provider = LspCallGraphProvider::new(client);
    let position = SourcePosition::new("/src/main.rs", 1, 1);

    let graph = provider
        .callers_graph(&position, 1)
        .expect("callers graph should build");

    assert_eq!(graph.node_count(), 2);
    let call_counts = counts.lock().expect("call count mutex poisoned");
    assert_eq!(call_counts.outgoing, 0);
    assert_eq!(call_counts.incoming, 1);
}

#[test]
fn build_graph_returns_symbol_not_found_on_empty_prepare() {
    test_build_graph_error(Response::Ok(Some(Vec::new())), |err| {
        matches!(err, GraphError::SymbolNotFound { .. })
    });
}

#[test]
fn build_graph_propagates_prepare_error() {
    test_build_graph_error(Response::Err(ErrorKind::Validation), |err| {
        matches!(err, GraphError::Validation(_))
    });
}
