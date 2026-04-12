//! Handlers for the `observe` domain.
//!
//! This module contains operation handlers for querying the codebase,
//! including definition lookup, reference finding, card retrieval,
//! graph-slice traversal, and structural search.

pub mod arguments;
pub mod enrich;
pub mod get_card;
pub mod get_definition;
pub mod graph_slice;
pub mod responses;

#[cfg(test)]
pub(crate) mod test_support;
