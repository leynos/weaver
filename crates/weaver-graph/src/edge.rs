//! Call graph edge representation.

use crate::node::{NodeId, Position};

/// Provenance of a call edge, indicating its source.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EdgeSource {
    /// Edge discovered via LSP call hierarchy.
    Lsp,
    /// Edge discovered via static analysis.
    StaticAnalysis,
    /// Edge discovered via dynamic profiling.
    DynamicProfiling,
}

impl std::fmt::Display for EdgeSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let label = match self {
            Self::Lsp => "lsp",
            Self::StaticAnalysis => "static",
            Self::DynamicProfiling => "dynamic",
        };
        f.write_str(label)
    }
}

/// An edge in the call graph representing a call relationship.
///
/// Edges are directed from caller to callee. Each edge carries provenance
/// information indicating how the relationship was discovered.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CallEdge {
    /// Node ID of the caller.
    caller: NodeId,
    /// Node ID of the callee.
    callee: NodeId,
    /// How this edge was discovered.
    source: EdgeSource,
    /// Position in the caller where the call occurs (if known).
    call_site: Option<Position>,
}

impl CallEdge {
    /// Creates a new call edge.
    #[must_use]
    pub const fn new(from_caller: NodeId, to_callee: NodeId, source: EdgeSource) -> Self {
        Self {
            caller: from_caller,
            callee: to_callee,
            source,
            call_site: None,
        }
    }

    /// Sets the call site location.
    #[must_use]
    pub const fn with_call_site(mut self, position: Position) -> Self {
        self.call_site = Some(position);
        self
    }

    /// Returns the caller node ID.
    #[must_use]
    pub const fn caller(&self) -> &NodeId {
        &self.caller
    }

    /// Returns the callee node ID.
    #[must_use]
    pub const fn callee(&self) -> &NodeId {
        &self.callee
    }

    /// Returns how this edge was discovered.
    #[must_use]
    pub const fn source(&self) -> EdgeSource {
        self.source
    }

    /// Returns the call site position if known.
    #[must_use]
    pub const fn call_site(&self) -> Option<Position> {
        self.call_site
    }

    /// Returns the call site line if known.
    #[must_use]
    pub const fn call_site_line(&self) -> Option<u32> {
        match self.call_site {
            Some(pos) => Some(pos.line),
            None => None,
        }
    }

    /// Returns the call site column if known.
    #[must_use]
    pub const fn call_site_column(&self) -> Option<u32> {
        match self.call_site {
            Some(pos) => Some(pos.column),
            None => None,
        }
    }
}
