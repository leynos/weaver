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
        Ok(_) => Err(String::from("expected graph error")),
        Err(error) if expected_error(&error) => Ok(()),
        Err(error) => Err(format!("unexpected error: {error:?}")),
    }
}

#[test]
fn build_graph_depth_zero_skips_traversal() -> Result<(), String> {
    let counts = Arc::new(Mutex::new(CallCounts::default()));
    let client = TestClient::new(
        Response::Ok(Some(vec![
            item("main", 1, 1).map_err(|error| format!("main item should build: {error}"))?,
        ])),
        Response::Err,
        Response::Err,
        Arc::clone(&counts),
    );
    let mut provider = LspCallGraphProvider::new(client);

    let graph = build_graph(&mut provider, 0)
        .map_err(|error| format!("depth-zero graph should build: {error}"))?;

    if graph.node_count() != 1 {
        return Err(format!(
            "expected one graph node, got {}",
            graph.node_count()
        ));
    }
    if graph.edge_count() != 0 {
        return Err(format!(
            "expected no graph edges, got {}",
            graph.edge_count()
        ));
    }
    let call_counts = counts.lock().expect("call count mutex poisoned");
    if call_counts.incoming != 0 {
        return Err(format!(
            "expected no incoming calls, got {}",
            call_counts.incoming
        ));
    }
    if call_counts.outgoing != 0 {
        return Err(format!(
            "expected no outgoing calls, got {}",
            call_counts.outgoing
        ));
    }
    Ok(())
}

#[test]
fn build_graph_collects_incoming_and_outgoing_edges() -> Result<(), String> {
    let counts = Arc::new(Mutex::new(CallCounts::default()));
    let client = TestClient::new(
        Response::Ok(Some(vec![
            item("main", 1, 1).map_err(|error| format!("main item should build: {error}"))?,
        ])),
        Response::Ok(Some(vec![
            incoming_call("caller", 3, 0)
                .map_err(|error| format!("caller item should build: {error}"))?,
        ])),
        Response::Ok(Some(vec![
            outgoing_call("helper", 5, 0)
                .map_err(|error| format!("helper item should build: {error}"))?,
        ])),
        Arc::clone(&counts),
    );
    let mut provider = LspCallGraphProvider::new(client);

    let graph =
        build_graph(&mut provider, 1).map_err(|error| format!("graph should build: {error}"))?;

    if graph.node_count() != 3 {
        return Err(format!(
            "expected three graph nodes, got {}",
            graph.node_count()
        ));
    }
    if graph.edge_count() != 2 {
        return Err(format!(
            "expected two graph edges, got {}",
            graph.edge_count()
        ));
    }

    let main = graph.find_by_name("main").expect("main node missing");
    let caller = graph.find_by_name("caller").expect("caller node missing");
    let helper = graph.find_by_name("helper").expect("helper node missing");

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
    let call_counts = counts.lock().expect("call count mutex poisoned");
    if call_counts.incoming != 1 {
        return Err(format!(
            "expected one incoming call, got {}",
            call_counts.incoming
        ));
    }
    if call_counts.outgoing != 1 {
        return Err(format!(
            "expected one outgoing call, got {}",
            call_counts.outgoing
        ));
    }
    Ok(())
}

#[test]
fn callers_graph_uses_incoming_only() -> Result<(), String> {
    let counts = Arc::new(Mutex::new(CallCounts::default()));
    let client = TestClient::new(
        Response::Ok(Some(vec![
            item("main", 1, 1).map_err(|error| format!("main item should build: {error}"))?,
        ])),
        Response::Ok(Some(vec![
            incoming_call("caller", 3, 0)
                .map_err(|error| format!("caller item should build: {error}"))?,
        ])),
        Response::Err,
        Arc::clone(&counts),
    );
    let mut provider = LspCallGraphProvider::new(client);
    let position = SourcePosition::new("/src/main.rs", 1, 1);

    let graph = provider
        .callers_graph(&position, 1)
        .map_err(|error| format!("callers graph should build: {error}"))?;

    if graph.node_count() != 2 {
        return Err(format!(
            "expected two graph nodes, got {}",
            graph.node_count()
        ));
    }
    let call_counts = counts.lock().expect("call count mutex poisoned");
    if call_counts.outgoing != 0 {
        return Err(format!(
            "expected no outgoing calls, got {}",
            call_counts.outgoing
        ));
    }
    if call_counts.incoming != 1 {
        return Err(format!(
            "expected one incoming call, got {}",
            call_counts.incoming
        ));
    }
    Ok(())
}

#[test]
fn build_graph_returns_symbol_not_found_on_empty_prepare() -> Result<(), String> {
    test_build_graph_error(Response::Ok(Some(Vec::new())), |err| {
        matches!(err, GraphError::SymbolNotFound { .. })
    })?;
    Ok(())
}

#[test]
fn build_graph_propagates_prepare_error() -> Result<(), String> {
    test_build_graph_error(Response::Err, |err| {
        matches!(err, GraphError::Validation(_))
    })?;
    Ok(())
}
