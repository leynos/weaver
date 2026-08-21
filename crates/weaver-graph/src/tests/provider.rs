//! Unit tests for the LSP call graph provider.

use std::sync::{Arc, Mutex};

use lsp_types::{
    CallHierarchyIncomingCall,
    CallHierarchyIncomingCallsParams,
    CallHierarchyItem,
    CallHierarchyOutgoingCall,
    CallHierarchyOutgoingCallsParams,
    CallHierarchyPrepareParams,
};

use crate::{
    CallGraph,
    GraphError,
    provider::{CallGraphProvider, CallHierarchyClient, LspCallGraphProvider, SourcePosition},
    tests::support::{Response, incoming_call, item, outgoing_call},
};

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

macro_rules! impl_call_handler {
    (
        $method:ident,
        $params:ty,
        $item:ty,
        $response_field:ident,
        $counter_field:ident,
        $label:literal
    ) => {
        fn $method(&mut self, _params: $params) -> Result<Option<Vec<$item>>, GraphError> {
            self.handle_call(
                &self.$response_field.clone(),
                |counts| counts.$counter_field += 1,
                $label,
            )
        }
    };
}

impl CallHierarchyClient for TestClient {
    fn prepare_call_hierarchy(
        &mut self,
        _params: CallHierarchyPrepareParams,
    ) -> Result<Option<Vec<CallHierarchyItem>>, GraphError> {
        self.prepare.as_result()
    }

    impl_call_handler!(
        incoming_calls,
        CallHierarchyIncomingCallsParams,
        CallHierarchyIncomingCall,
        incoming,
        incoming,
        "incoming"
    );
    impl_call_handler!(
        outgoing_calls,
        CallHierarchyOutgoingCallsParams,
        CallHierarchyOutgoingCall,
        outgoing,
        outgoing,
        "outgoing"
    );
}

fn build_graph(
    provider: &mut LspCallGraphProvider<TestClient>,
    depth: u32,
) -> Result<CallGraph, GraphError> {
    let position = SourcePosition::new("/src/main.rs", 1, 1);
    provider.build_graph(&position, depth)
}

fn test_build_graph_error(
    prepare_response: Response<CallHierarchyItem>,
    expected_error: impl Fn(&GraphError) -> bool,
) -> Result<(), String> {
    let counts = Arc::new(Mutex::new(CallCounts::default()));
    let client = TestClient::new(
        prepare_response,
        Response::Ok(None),
        Response::Ok(None),
        counts,
    );
    let mut provider = LspCallGraphProvider::new(client);
    let position = SourcePosition::new("/src/main.rs", 1, 1);

    match provider.build_graph(&position, 1) {
        Err(error) if expected_error(&error) => Ok(()),
        Err(error) => Err(format!("unexpected error: {error:?}")),
        Ok(graph) => Err(format!("expected graph error, got: {graph:?}")),
    }
}

#[test]
fn build_graph_depth_zero_skips_traversal() {
    let counts = Arc::new(Mutex::new(CallCounts::default()));
    let client = TestClient::new(
        Response::Ok(Some(vec![match item("main", 1, 1) {
            Ok(item) => item,
            Err(error) => panic!("test item should be valid: {error}"),
        }])),
        Response::Err,
        Response::Err,
        Arc::clone(&counts),
    );
    let mut provider = LspCallGraphProvider::new(client);

    let graph = match build_graph(&mut provider, 0) {
        Ok(graph) => graph,
        Err(error) => panic!("graph should build: {error}"),
    };

    assert_eq!(graph.node_count(), 1);
    assert_eq!(graph.edge_count(), 0);
    let call_counts = counts.lock().expect("call count mutex poisoned");
    assert_eq!(call_counts.incoming, 0);
    assert_eq!(call_counts.outgoing, 0);
}

#[test]
fn build_graph_collects_incoming_and_outgoing_edges() -> Result<(), String> {
    let counts = Arc::new(Mutex::new(CallCounts::default()));
    let client = TestClient::new(
        Response::Ok(Some(vec![item("main", 1, 1)?])),
        Response::Ok(Some(vec![incoming_call("caller", 3, 0)?])),
        Response::Ok(Some(vec![outgoing_call("helper", 5, 0)?])),
        Arc::clone(&counts),
    );
    let mut provider = LspCallGraphProvider::new(client);

    let graph = build_graph(&mut provider, 1).map_err(|error| error.to_string())?;
    let call_counts = counts
        .lock()
        .map_err(|_| String::from("call count mutex poisoned"))?;

    assert_graph_has_expected_edges(&graph, &call_counts)
}

fn assert_graph_has_expected_edges(
    graph: &CallGraph,
    call_counts: &CallCounts,
) -> Result<(), String> {
    if graph.node_count() != 3 || graph.edge_count() != 2 {
        return Err(format!(
            "expected graph with three nodes and two edges, got {} nodes and {} edges",
            graph.node_count(),
            graph.edge_count(),
        ));
    }

    let main = graph
        .find_by_name("main")
        .ok_or_else(|| String::from("main node missing"))?;
    let caller = graph
        .find_by_name("caller")
        .ok_or_else(|| String::from("caller node missing"))?;
    let helper = graph
        .find_by_name("helper")
        .ok_or_else(|| String::from("helper node missing"))?;

    if !graph
        .callers_of(main.id())
        .any(|node| node.id() == caller.id())
    {
        return Err(String::from("caller edge missing"));
    }
    if !graph
        .callees_of(main.id())
        .any(|node| node.id() == helper.id())
    {
        return Err(String::from("callee edge missing"));
    }
    if call_counts.incoming == 1 && call_counts.outgoing == 1 {
        Ok(())
    } else {
        Err(format!(
            "expected one incoming and one outgoing call, got {} incoming and {} outgoing",
            call_counts.incoming, call_counts.outgoing,
        ))
    }
}

#[test]
fn callers_graph_uses_incoming_only() {
    let counts = Arc::new(Mutex::new(CallCounts::default()));
    let client = TestClient::new(
        Response::Ok(Some(vec![match item("main", 1, 1) {
            Ok(item) => item,
            Err(error) => panic!("test item should be valid: {error}"),
        }])),
        Response::Ok(Some(vec![match incoming_call("caller", 3, 0) {
            Ok(call) => call,
            Err(error) => panic!("test incoming call should be valid: {error}"),
        }])),
        Response::Err,
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
    if let Err(error) = test_build_graph_error(Response::Ok(Some(Vec::new())), |err| {
        matches!(err, GraphError::SymbolNotFound { .. })
    }) {
        panic!("symbol-not-found scenario should fail as expected: {error}");
    }
}

#[test]
fn build_graph_propagates_prepare_error() {
    if let Err(error) = test_build_graph_error(Response::Err, |err| {
        matches!(err, GraphError::Validation(_))
    }) {
        panic!("prepare-error scenario should fail as expected: {error}");
    }
}
