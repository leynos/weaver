//! Budget constraints for graph-slice traversal.
//!
//! A [`SliceBudget`] controls how much of the graph the traversal may
//! explore before truncating. Every budget field has an explicit default
//! so that callers who omit optional flags receive deterministic,
//! documented behaviour.

use serde::{Deserialize, Serialize};

/// Default maximum number of cards in a graph slice.
pub const DEFAULT_MAX_CARDS: u32 = 30;

/// Default maximum number of edges in a graph slice.
pub const DEFAULT_MAX_EDGES: u32 = 200;

/// Default maximum estimated token count for a graph slice.
pub const DEFAULT_MAX_ESTIMATED_TOKENS: u32 = 4000;

/// Budget constraints that bound a graph-slice traversal.
///
/// All fields carry explicit defaults. When a caller omits budget flags
/// the defaults are applied and echoed back in the response
/// `constraints` object, making the applied budget observable and
/// snapshot-stable.
///
/// # Example
///
/// ```
/// use weaver_cards::SliceBudget;
///
/// let budget = SliceBudget::default();
/// assert_eq!(budget.max_cards(), 30);
/// assert_eq!(budget.max_edges(), 200);
/// assert_eq!(budget.max_estimated_tokens(), 4000);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SliceBudget {
    /// Maximum number of symbol cards to include.
    #[serde(rename = "max_cards")]
    cards: u32,
    /// Maximum number of edges to include.
    #[serde(rename = "max_edges")]
    edges: u32,
    /// Maximum estimated token count across all included cards.
    #[serde(rename = "max_estimated_tokens")]
    estimated_tokens: u32,
}

impl Default for SliceBudget {
    fn default() -> Self {
        Self {
            cards: DEFAULT_MAX_CARDS,
            edges: DEFAULT_MAX_EDGES,
            estimated_tokens: DEFAULT_MAX_ESTIMATED_TOKENS,
        }
    }
}

impl SliceBudget {
    /// Creates a budget with explicit values for all fields.
    #[must_use]
    pub const fn new(cards: u32, edges: u32, estimated_tokens: u32) -> Self {
        Self {
            cards,
            edges,
            estimated_tokens,
        }
    }

    /// Returns the maximum number of cards.
    #[must_use]
    pub const fn max_cards(&self) -> u32 {
        self.cards
    }

    /// Returns the maximum number of edges.
    #[must_use]
    pub const fn max_edges(&self) -> u32 {
        self.edges
    }

    /// Returns the maximum estimated token count.
    #[must_use]
    pub const fn max_estimated_tokens(&self) -> u32 {
        self.estimated_tokens
    }

    /// Returns a new budget with the given maximum number of cards.
    #[must_use]
    pub const fn with_max_cards(self, max_cards: u32) -> Self {
        Self::new(max_cards, self.edges, self.estimated_tokens)
    }

    /// Returns a new budget with the given maximum number of edges.
    #[must_use]
    pub const fn with_max_edges(self, max_edges: u32) -> Self {
        Self::new(self.cards, max_edges, self.estimated_tokens)
    }

    /// Returns a new budget with the given maximum estimated token count.
    #[must_use]
    pub const fn with_max_estimated_tokens(self, max_tokens: u32) -> Self {
        Self::new(self.cards, self.edges, max_tokens)
    }
}
