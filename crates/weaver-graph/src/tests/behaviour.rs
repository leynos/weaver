//! Behaviour-driven tests for the weaver-graph call hierarchy provider.

use std::cell::RefCell;

use anyhow::{Context, Result, bail, ensure};
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
    fn simple_chain() -> Result<Self, GraphError> {
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

/// Unwraps `world.result` into a reference to the built `CallGraph`,
/// mapping a stored `GraphError` into an `anyhow::Error` with context.
fn built_graph(world: &TestWorld) -> Result<&CallGraph> {
    world
        .result
        .as_ref()
        .context("result missing")?
        .as_ref()
        .map_err(|error| anyhow::anyhow!("graph build failed: {error}"))
}

#[given("a call hierarchy client with a simple call chain")]
fn given_simple_chain(world: &RefCell<TestWorld>) -> Result<()> {
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
fn when_build_graph(world: &RefCell<TestWorld>, depth: u32) -> Result<()> {
    let mut world_state = world.borrow_mut();
    let provider = world_state
        .provider
        .as_mut()
        .context("provider should be configured")?;
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
) -> Result<()> {
    let world_state = world.borrow();
    let graph = built_graph(&world_state)?;
    ensure!(
        graph.node_count() == node_count,
        "expected {node_count} nodes"
    );
    ensure!(
        graph.edge_count() == edge_count,
        "expected {edge_count} edges"
    );
    Ok(())
}

#[then("the graph includes node {name}")]
fn then_graph_includes_node(world: &RefCell<TestWorld>, name: String) -> Result<()> {
    let world_state = world.borrow();
    let graph = built_graph(&world_state)?;
    let node_name = strip_quotes(&name);
    ensure!(
        graph.find_by_name(node_name).is_some(),
        "node {node_name} missing"
    );
    Ok(())
}

#[then("the graph includes an edge from {caller} to {callee}")]
fn then_graph_includes_edge(
    world: &RefCell<TestWorld>,
    caller: String,
    callee: String,
) -> Result<()> {
    let world_state = world.borrow();
    let graph = built_graph(&world_state)?;
    let caller_name = strip_quotes(&caller);
    let callee_name = strip_quotes(&callee);
    let caller_node = graph
        .find_by_name(caller_name)
        .context("caller node missing")?;
    let callee_node = graph
        .find_by_name(callee_name)
        .context("callee node missing")?;
    let has_edge = graph
        .edges()
        .any(|edge| edge.caller() == caller_node.id() && edge.callee() == callee_node.id());
    ensure!(has_edge, "edge {caller_name} -> {callee_name} missing");
    Ok(())
}

#[then("the graph build fails with {error_kind}")]
fn then_graph_build_fails(world: &RefCell<TestWorld>, error_kind: String) -> Result<()> {
    let world_state = world.borrow();
    let result = world_state.result.as_ref().context("result missing")?;
    let Err(err) = result else {
        bail!("expected graph build to fail");
    };
    let expected_kind = strip_quotes(&error_kind);
    match expected_kind {
        "symbol_not_found" => {
            ensure!(matches!(err, GraphError::SymbolNotFound { .. }));
        }
        "validation_error" => {
            ensure!(matches!(err, GraphError::Validation(_)));
        }
        other => bail!("unknown error kind: {other}"),
    }
    Ok(())
}

#[scenario(path = "tests/features/weaver_graph.feature")]
fn call_graph_behaviour(world: RefCell<TestWorld>) { std::mem::drop(world); }
