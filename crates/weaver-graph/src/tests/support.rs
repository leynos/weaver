//! Shared test helpers for call hierarchy test cases.

use crate::GraphError;
use lsp_types::{
    CallHierarchyIncomingCall, CallHierarchyItem, CallHierarchyOutgoingCall, Position, Range,
    SymbolKind, Uri,
};
use std::str::FromStr;

/// Test-friendly wrapper for call hierarchy responses.
#[derive(Clone, Debug)]
pub(super) enum Response<T: Clone> {
    Ok(Option<Vec<T>>),
    Err,
}

impl<T: Clone> Response<T> {
    /// Converts the response into a `Result` for provider-facing tests.
    pub(super) fn as_result(&self) -> Result<Option<Vec<T>>, GraphError> {
        match self {
            Self::Ok(value) => Ok(value.clone()),
            Self::Err => Err(GraphError::validation("test failure")),
        }
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

/// Builds a test call hierarchy item for the provided name and position.
pub(super) fn item(name: &str, line: u32, column: u32) -> CallHierarchyItem {
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

/// Builds a test incoming call with a caller item and an offset call range.
pub(super) fn incoming_call(name: &str, line: u32, column: u32) -> CallHierarchyIncomingCall {
    CallHierarchyIncomingCall {
        from: item(name, line, column),
        from_ranges: vec![range(line + 1, column + 2)],
    }
}

/// Builds a test outgoing call with a callee item and an offset call range.
pub(super) fn outgoing_call(name: &str, line: u32, column: u32) -> CallHierarchyOutgoingCall {
    CallHierarchyOutgoingCall {
        to: item(name, line, column),
        from_ranges: vec![range(line + 1, column + 2)],
    }
}
