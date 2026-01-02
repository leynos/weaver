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

mod behaviour;
mod provider;
