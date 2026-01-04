//! Unit tests for the LSP call graph provider.

use crate::provider::{
    CallGraphProvider, CallHierarchyClient, LspCallGraphProvider, SourcePosition,
};
use crate::tests::support::{Response, incoming_call, item, outgoing_call};
use crate::{CallGraph, GraphError};
use lsp_types::{
    CallHierarchyIncomingCall, CallHierarchyIncomingCallsParams, CallHierarchyItem,
    CallHierarchyOutgoingCall, CallHierarchyOutgoingCallsParams, CallHierarchyPrepareParams,
};
use std::sync::{Arc, Mutex};

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
        Response::Err,
        Response::Err,
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
    test_build_graph_error(Response::Ok(Some(Vec::new())), |err| {
        matches!(err, GraphError::SymbolNotFound { .. })
    });
}

#[test]
fn build_graph_propagates_prepare_error() {
    test_build_graph_error(Response::Err, |err| {
        matches!(err, GraphError::Validation(_))
    });
}
