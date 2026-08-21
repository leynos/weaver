//! Behaviour-driven tests for the weaver-graph call hierarchy provider.

use std::cell::RefCell;

use lsp_types::{
    CallHierarchyIncomingCall,
    CallHierarchyIncomingCallsParams,
    CallHierarchyItem,
    CallHierarchyOutgoingCall,
    CallHierarchyOutgoingCallsParams,
    CallHierarchyPrepareParams,
};
use rstest::fixture;
use rstest_bdd_macros::{given, scenario, then, when};
use weaver_test_macros::allow_fixture_expansion_lints;

use crate::{
    CallGraph,
    GraphError,
    provider::{CallGraphProvider, CallHierarchyClient, LspCallGraphProvider, SourcePosition},
    tests::support::{Response, incoming_call, item, outgoing_call},
};

#[derive(Default)]
struct TestWorld {
    provider: Option<LspCallGraphProvider<TestClient>>,
    result: Option<Result<CallGraph, GraphError>>,
}

type StepResult = Result<(), String>;

#[allow_fixture_expansion_lints]
#[fixture]
fn world() -> RefCell<TestWorld> { RefCell::new(TestWorld::default()) }

#[derive(Clone, Debug)]
struct TestClient {
    prepare: Response<CallHierarchyItem>,
    incoming: Response<CallHierarchyIncomingCall>,
    outgoing: Response<CallHierarchyOutgoingCall>,
}

impl TestClient {
    fn simple_chain() -> Result<Self, String> {
        Ok(Self {
            prepare: Response::Ok(Some(vec![item("main", 1, 1)?])),
            incoming: Response::Ok(Some(vec![incoming_call("caller", 3, 0)?])),
            outgoing: Response::Ok(Some(vec![outgoing_call("helper", 5, 0)?])),
        })
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
            prepare: Response::Err,
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

fn strip_quotes(value: &str) -> &str { value.trim_matches('"') }

#[given("a call hierarchy client with a simple call chain")]
fn given_simple_chain(world: &RefCell<TestWorld>) -> StepResult {
    world.borrow_mut().provider = Some(LspCallGraphProvider::new(TestClient::simple_chain()?));
    Ok(())
}

#[given("a call hierarchy client with no matching symbol")]
fn given_no_symbol(world: &RefCell<TestWorld>) {
    world.borrow_mut().provider = Some(LspCallGraphProvider::new(TestClient::no_symbol()));
}

#[given("a call hierarchy client that returns an error")]
fn given_erroring_client(world: &RefCell<TestWorld>) {
    world.borrow_mut().provider = Some(LspCallGraphProvider::new(TestClient::failing()));
}

#[when("a call graph is built with depth {depth}")]
fn when_build_graph(world: &RefCell<TestWorld>, depth: u32) -> StepResult {
    let mut world_state = world.borrow_mut();
    let provider = world_state
        .provider
        .as_mut()
        .ok_or_else(|| String::from("provider should be configured"))?;
    let position = SourcePosition::new("/src/main.rs", 1, 1);
    let result = provider.build_graph(&position, depth);
    world_state.result = Some(result);
    Ok(())
}

#[then("the graph has {node_count} nodes and {edge_count} edges")]
fn then_graph_counts(
    world: &RefCell<TestWorld>,
    node_count: usize,
    edge_count: usize,
) -> StepResult {
    let world_state = world.borrow();
    let graph = world_state
        .result
        .as_ref()
        .ok_or_else(|| String::from("result missing"))?
        .as_ref()
        .map_err(|error| format!("graph build failed: {error}"))?;
    if graph.node_count() == node_count && graph.edge_count() == edge_count {
        Ok(())
    } else {
        Err(format!(
            "expected {node_count} nodes and {edge_count} edges, got {} nodes and {} edges",
            graph.node_count(),
            graph.edge_count(),
        ))
    }
}

#[then("the graph includes node {name}")]
fn then_graph_includes_node(world: &RefCell<TestWorld>, name: String) -> StepResult {
    let world_state = world.borrow();
    let graph = world_state
        .result
        .as_ref()
        .ok_or_else(|| String::from("result missing"))?
        .as_ref()
        .map_err(|error| format!("graph build failed: {error}"))?;
    let node_name = strip_quotes(&name);
    if graph.find_by_name(node_name).is_some() {
        Ok(())
    } else {
        Err(format!("node {node_name} missing"))
    }
}

#[then("the graph includes an edge from {caller} to {callee}")]
fn then_graph_includes_edge(
    world: &RefCell<TestWorld>,
    caller: String,
    callee: String,
) -> StepResult {
    let world_state = world.borrow();
    let graph = world_state
        .result
        .as_ref()
        .ok_or_else(|| String::from("result missing"))?
        .as_ref()
        .map_err(|error| format!("graph build failed: {error}"))?;
    let caller_name = strip_quotes(&caller);
    let callee_name = strip_quotes(&callee);
    let caller_node = graph
        .find_by_name(caller_name)
        .ok_or_else(|| format!("caller node {caller_name} missing"))?;
    let callee_node = graph
        .find_by_name(callee_name)
        .ok_or_else(|| format!("callee node {callee_name} missing"))?;
    let has_edge = graph
        .edges()
        .any(|edge| edge.caller() == caller_node.id() && edge.callee() == callee_node.id());
    if has_edge {
        Ok(())
    } else {
        Err(format!("edge {caller_name} -> {callee_name} missing"))
    }
}

#[then("the graph build fails with {error_kind}")]
fn then_graph_build_fails(world: &RefCell<TestWorld>, error_kind: String) -> StepResult {
    let world_state = world.borrow();
    let err = world_state
        .result
        .as_ref()
        .ok_or_else(|| String::from("result missing"))?
        .as_ref()
        .err()
        .ok_or_else(|| String::from("expected graph build to fail"))?;
    let expected_kind = strip_quotes(&error_kind);
    match expected_kind {
        "symbol_not_found" if matches!(err, GraphError::SymbolNotFound { .. }) => Ok(()),
        "validation_error" if matches!(err, GraphError::Validation(_)) => Ok(()),
        "symbol_not_found" | "validation_error" => Err(format!(
            "expected {expected_kind} graph error, got: {err:?}"
        )),
        other => Err(format!("unknown error kind: {other}")),
    }
}

#[scenario(path = "tests/features/weaver_graph.feature")]
fn call_graph_behaviour(world: RefCell<TestWorld>) { std::mem::drop(world); }
