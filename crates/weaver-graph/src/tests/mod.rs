//! Unit tests for the weaver-graph crate.

// Tests use caller/callee terminology which triggers similar_names lint
#![expect(
    clippy::similar_names,
    reason = "caller/callee are domain terms for tests"
)]
// Closures are needed because first() returns &&T, not &T
#![expect(
    clippy::redundant_closure_for_method_calls,
    reason = "closures needed for &&T to &T conversion in first().map()"
)]

mod graph_tests {
    use crate::edge::{CallEdge, EdgeSource};
    use crate::graph::CallGraph;
    use crate::node::{CallNode, Position, SymbolKind};

    #[test]
    fn empty_graph_has_no_nodes() {
        let graph = CallGraph::new();
        assert!(graph.is_empty());
        assert_eq!(graph.node_count(), 0);
        assert_eq!(graph.edge_count(), 0);
    }

    #[test]
    fn can_add_and_retrieve_node() {
        let mut graph = CallGraph::new();
        let node = CallNode::new(
            "main",
            SymbolKind::Function,
            "/src/main.rs",
            Position::new(10, 0),
        );
        let id = node.id().clone();

        graph.add_node(node);

        assert!(!graph.is_empty());
        assert_eq!(graph.node_count(), 1);
        assert!(graph.contains_node(&id));

        let retrieved = graph.node(&id);
        assert!(retrieved.is_some());
        assert_eq!(retrieved.map(|n| n.name()), Some("main"));
    }

    #[test]
    fn can_add_edges_and_query_callers() {
        let mut graph = CallGraph::new();

        let caller = CallNode::new(
            "caller",
            SymbolKind::Function,
            "/src/lib.rs",
            Position::new(5, 0),
        );
        let callee = CallNode::new(
            "callee",
            SymbolKind::Function,
            "/src/lib.rs",
            Position::new(20, 0),
        );

        let caller_id = caller.id().clone();
        let callee_id = callee.id().clone();

        graph.add_node(caller);
        graph.add_node(callee);
        graph.add_edge(CallEdge::new(
            caller_id.clone(),
            callee_id.clone(),
            EdgeSource::Lsp,
        ));

        let callers: Vec<_> = graph.callers_of(&callee_id).collect();
        assert_eq!(callers.len(), 1);
        assert_eq!(callers.first().map(|n| n.name()), Some("caller"));

        let callees: Vec<_> = graph.callees_of(&caller_id).collect();
        assert_eq!(callees.len(), 1);
        assert_eq!(callees.first().map(|n| n.name()), Some("callee"));
    }

    #[test]
    fn find_by_name_works() {
        let mut graph = CallGraph::new();

        let node = CallNode::new(
            "my_function",
            SymbolKind::Function,
            "/src/lib.rs",
            Position::new(10, 0),
        );
        graph.add_node(node);

        assert!(graph.find_by_name("my_function").is_some());
        assert!(graph.find_by_name("other_function").is_none());
    }

    #[test]
    fn qualified_name_includes_container() {
        let node = CallNode::new(
            "method",
            SymbolKind::Method,
            "/src/lib.rs",
            Position::new(10, 0),
        )
        .with_container("Foo");

        assert_eq!(node.qualified_name(), "Foo.method");
    }
}

mod node_tests {
    use crate::node::{CallNode, NodeId, Position, SymbolKind};
    use camino::Utf8PathBuf;

    #[test]
    fn node_id_format_is_correct() {
        let path = Utf8PathBuf::from("/src/main.rs");
        let id = NodeId::new(&path, 10, 5, "main");

        assert_eq!(id.as_str(), "/src/main.rs:10:5:main");
    }

    #[test]
    fn node_accessors_return_correct_values() {
        let node = CallNode::new(
            "test_fn",
            SymbolKind::Function,
            "/src/lib.rs",
            Position::new(42, 8),
        );

        assert_eq!(node.name(), "test_fn");
        assert_eq!(node.kind(), SymbolKind::Function);
        assert_eq!(node.path().as_str(), "/src/lib.rs");
        assert_eq!(node.line(), 42);
        assert_eq!(node.column(), 8);
        assert!(node.container().is_none());
    }
}

mod edge_tests {
    use crate::edge::{CallEdge, EdgeSource};
    use crate::node::{NodeId, Position};
    use camino::Utf8PathBuf;

    #[test]
    fn edge_accessors_return_correct_values() {
        let path = Utf8PathBuf::from("/src/lib.rs");
        let caller_id = NodeId::new(&path, 10, 0, "caller");
        let callee_id = NodeId::new(&path, 20, 0, "callee");

        let edge = CallEdge::new(caller_id.clone(), callee_id.clone(), EdgeSource::Lsp)
            .with_call_site(Position::new(15, 4));

        assert_eq!(edge.caller().as_str(), caller_id.as_str());
        assert_eq!(edge.callee().as_str(), callee_id.as_str());
        assert_eq!(edge.source(), EdgeSource::Lsp);
        assert_eq!(edge.call_site_line(), Some(15));
        assert_eq!(edge.call_site_column(), Some(4));
    }
}

mod provider_tests {
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
            let mut counts = self
                .counts
                .lock()
                .expect("incoming call count mutex poisoned");
            counts.incoming += 1;
            self.incoming.as_result()
        }

        fn outgoing_calls(
            &mut self,
            _params: CallHierarchyOutgoingCallsParams,
        ) -> Result<Option<Vec<CallHierarchyOutgoingCall>>, GraphError> {
            let mut counts = self
                .counts
                .lock()
                .expect("outgoing call count mutex poisoned");
            counts.outgoing += 1;
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

    fn build_graph(provider: &mut LspCallGraphProvider<TestClient>, depth: u32) -> CallGraph {
        let position = SourcePosition::new("/src/main.rs", 1, 1);
        provider
            .build_graph(&position, depth)
            .expect("graph should build")
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
        let counts = Arc::new(Mutex::new(CallCounts::default()));
        let client = TestClient::new(
            Response::Ok(Some(Vec::new())),
            Response::Ok(None),
            Response::Ok(None),
            counts,
        );
        let mut provider = LspCallGraphProvider::new(client);
        let position = SourcePosition::new("/src/main.rs", 1, 1);

        let err = provider
            .build_graph(&position, 1)
            .expect_err("expected symbol not found error");

        assert!(matches!(err, GraphError::SymbolNotFound { .. }));
    }

    #[test]
    fn build_graph_propagates_prepare_error() {
        let counts = Arc::new(Mutex::new(CallCounts::default()));
        let client = TestClient::new(
            Response::Err(ErrorKind::Validation),
            Response::Ok(None),
            Response::Ok(None),
            counts,
        );
        let mut provider = LspCallGraphProvider::new(client);
        let position = SourcePosition::new("/src/main.rs", 1, 1);

        let err = provider
            .build_graph(&position, 1)
            .expect_err("expected validation error");

        assert!(matches!(err, GraphError::Validation(_)));
    }
}

mod behaviour;
