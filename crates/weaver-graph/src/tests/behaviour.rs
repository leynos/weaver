//! Behaviour-driven tests for the weaver-graph call hierarchy provider.

use std::cell::RefCell;
use std::str::FromStr;

use rstest::fixture;
use rstest_bdd_macros::{given, scenario, then, when};

use crate::provider::{
    CallGraphProvider, CallHierarchyClient, LspCallGraphProvider, SourcePosition,
};
use crate::{CallGraph, GraphError};
use lsp_types::{
    CallHierarchyIncomingCall, CallHierarchyIncomingCallsParams, CallHierarchyItem,
    CallHierarchyOutgoingCall, CallHierarchyOutgoingCallsParams, CallHierarchyPrepareParams,
    Position, Range, SymbolKind, Uri,
};

#[derive(Default)]
struct TestWorld {
    provider: Option<LspCallGraphProvider<TestClient>>,
    result: Option<Result<CallGraph, GraphError>>,
}

#[fixture]
fn world() -> RefCell<TestWorld> {
    RefCell::new(TestWorld::default())
}

#[derive(Clone, Copy, Debug)]
enum ErrorKind {
    Validation,
}

impl ErrorKind {
    fn to_error(self) -> GraphError {
        match self {
            Self::Validation => GraphError::validation("call hierarchy failure"),
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

#[derive(Clone, Debug)]
struct TestClient {
    prepare: Response<CallHierarchyItem>,
    incoming: Response<CallHierarchyIncomingCall>,
    outgoing: Response<CallHierarchyOutgoingCall>,
}

impl TestClient {
    fn simple_chain() -> Self {
        Self {
            prepare: Response::Ok(Some(vec![item("main", 1, 1)])),
            incoming: Response::Ok(Some(vec![incoming_call("caller", 3, 0)])),
            outgoing: Response::Ok(Some(vec![outgoing_call("helper", 5, 0)])),
        }
    }

    fn no_symbol() -> Self {
        Self {
            prepare: Response::Ok(Some(Vec::new())),
            incoming: Response::Ok(None),
            outgoing: Response::Ok(None),
        }
    }

    fn failing() -> Self {
        Self {
            prepare: Response::Err(ErrorKind::Validation),
            incoming: Response::Ok(None),
            outgoing: Response::Ok(None),
        }
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
        self.incoming.as_result()
    }

    fn outgoing_calls(
        &mut self,
        _params: CallHierarchyOutgoingCallsParams,
    ) -> Result<Option<Vec<CallHierarchyOutgoingCall>>, GraphError> {
        self.outgoing.as_result()
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

fn strip_quotes(value: &str) -> &str {
    value.trim_matches('"')
}

#[given("a call hierarchy client with a simple call chain")]
fn given_simple_chain(world: &RefCell<TestWorld>) {
    world.borrow_mut().provider = Some(LspCallGraphProvider::new(TestClient::simple_chain()));
}

#[given("a call hierarchy client with no matching symbol")]
fn given_no_symbol(world: &RefCell<TestWorld>) {
    world.borrow_mut().provider = Some(LspCallGraphProvider::new(TestClient::no_symbol()));
}

#[given("a call hierarchy client that returns an error")]
fn given_erroring_client(world: &RefCell<TestWorld>) {
    world.borrow_mut().provider = Some(LspCallGraphProvider::new(TestClient::failing()));
}

#[when("I build a call graph from {symbol} with depth {depth}")]
fn when_build_graph(world: &RefCell<TestWorld>, symbol: String, depth: u32) {
    let _ = strip_quotes(&symbol);
    let mut world_state = world.borrow_mut();
    let provider = world_state
        .provider
        .as_mut()
        .expect("provider should be configured");
    let position = SourcePosition::new("/src/main.rs", 1, 1);
    let result = provider.build_graph(&position, depth);
    world_state.result = Some(result);
}

#[then("the graph has {node_count} nodes and {edge_count} edges")]
fn then_graph_counts(world: &RefCell<TestWorld>, node_count: usize, edge_count: usize) {
    let world_state = world.borrow();
    let graph = world_state
        .result
        .as_ref()
        .expect("result missing")
        .as_ref()
        .expect("graph build failed");
    assert_eq!(graph.node_count(), node_count);
    assert_eq!(graph.edge_count(), edge_count);
}

#[then("the graph includes node {name}")]
fn then_graph_includes_node(world: &RefCell<TestWorld>, name: String) {
    let world_state = world.borrow();
    let graph = world_state
        .result
        .as_ref()
        .expect("result missing")
        .as_ref()
        .expect("graph build failed");
    let node_name = strip_quotes(&name);
    assert!(
        graph.find_by_name(node_name).is_some(),
        "node {node_name} missing"
    );
}

#[then("the graph includes an edge from {caller} to {callee}")]
fn then_graph_includes_edge(world: &RefCell<TestWorld>, caller: String, callee: String) {
    let world_state = world.borrow();
    let graph = world_state
        .result
        .as_ref()
        .expect("result missing")
        .as_ref()
        .expect("graph build failed");
    let caller_name = strip_quotes(&caller);
    let callee_name = strip_quotes(&callee);
    let caller_node = graph
        .find_by_name(caller_name)
        .expect("caller node missing");
    let callee_node = graph
        .find_by_name(callee_name)
        .expect("callee node missing");
    let has_edge = graph
        .edges()
        .any(|edge| edge.caller() == caller_node.id() && edge.callee() == callee_node.id());
    assert!(has_edge, "edge {caller_name} -> {callee_name} missing");
}

#[then("the graph build fails with {error_kind}")]
fn then_graph_build_fails(world: &RefCell<TestWorld>, error_kind: String) {
    let world_state = world.borrow();
    let err = world_state
        .result
        .as_ref()
        .expect("result missing")
        .as_ref()
        .expect_err("expected graph build to fail");
    let expected_kind = strip_quotes(&error_kind);
    match expected_kind {
        "symbol_not_found" => {
            assert!(matches!(err, GraphError::SymbolNotFound { .. }));
        }
        "validation_error" => {
            assert!(matches!(err, GraphError::Validation(_)));
        }
        other => panic!("unknown error kind: {other}"),
    }
}

#[scenario(path = "tests/features/weaver_graph.feature")]
fn call_graph_behaviour(world: RefCell<TestWorld>) {
    let _ = world;
}
