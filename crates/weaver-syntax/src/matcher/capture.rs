//! Capture types for pattern matching.
//!
//! Captures bind metavariable names to the parts of the source code they match.

use std::collections::HashMap;
use std::ops::Range;

/// A single captured AST node.
#[derive(Debug, Clone)]
pub struct CapturedNode<'a> {
    node: tree_sitter::Node<'a>,
    text: &'a str,
}

impl<'a> CapturedNode<'a> {
    /// Returns the captured AST node.
    #[must_use]
    pub const fn node(&self) -> tree_sitter::Node<'a> {
        self.node
    }

    /// Returns the text of the captured node.
    #[must_use]
    pub const fn text(&self) -> &'a str {
        self.text
    }

    /// Returns the byte range of the captured node.
    #[must_use]
    pub fn byte_range(&self) -> Range<usize> {
        self.node.byte_range()
    }
}

/// A capture for a multiple-node metavariable (`$$$NAME`).
#[derive(Debug, Clone)]
pub struct CapturedNodes<'a> {
    nodes: Vec<CapturedNode<'a>>,
    text: &'a str,
    byte_range: Range<usize>,
}

impl<'a> CapturedNodes<'a> {
    /// Returns the captured nodes in order.
    #[must_use]
    pub fn nodes(&self) -> &[CapturedNode<'a>] {
        &self.nodes
    }

    /// Returns the full source text covered by the capture.
    #[must_use]
    pub const fn text(&self) -> &'a str {
        self.text
    }

    /// Returns the byte range covered by the capture.
    #[must_use]
    pub fn byte_range(&self) -> Range<usize> {
        self.byte_range.clone()
    }
}

/// Captured metavariable value.
#[derive(Debug, Clone)]
pub enum CapturedValue<'a> {
    /// A single-node capture (`$NAME`).
    Single(CapturedNode<'a>),
    /// A multi-node capture (`$$$NAME`).
    Multiple(CapturedNodes<'a>),
}

impl<'a> CapturedValue<'a> {
    /// Returns the captured text.
    #[must_use]
    pub const fn text(&self) -> &'a str {
        match self {
            Self::Single(node) => node.text(),
            Self::Multiple(nodes) => nodes.text(),
        }
    }

    /// Returns the byte range of the capture.
    #[must_use]
    pub fn byte_range(&self) -> Range<usize> {
        match self {
            Self::Single(node) => node.byte_range(),
            Self::Multiple(nodes) => nodes.byte_range(),
        }
    }

    /// Returns the capture as a single node, if applicable.
    #[must_use]
    pub const fn as_single(&self) -> Option<&CapturedNode<'a>> {
        match self {
            Self::Single(node) => Some(node),
            Self::Multiple(_) => None,
        }
    }

    /// Returns the capture as multiple nodes, if applicable.
    #[must_use]
    pub const fn as_multiple(&self) -> Option<&CapturedNodes<'a>> {
        match self {
            Self::Multiple(nodes) => Some(nodes),
            Self::Single(_) => None,
        }
    }
}

#[derive(Debug, Clone, Default)]
pub(super) struct Captures<'a> {
    inner: HashMap<String, CapturedValue<'a>>,
}

impl<'a> Captures<'a> {
    pub(super) fn into_inner(self) -> HashMap<String, CapturedValue<'a>> {
        self.inner
    }

    pub(super) fn capture_single(
        &mut self,
        name: &str,
        node: tree_sitter::Node<'a>,
        source: &'a str,
    ) -> bool {
        if name == "_" {
            return true;
        }

        let text = source.get(node.byte_range()).unwrap_or_default();
        let value = CapturedValue::Single(CapturedNode { node, text });

        self.insert_consistent(name, value)
    }

    pub(super) fn capture_multiple(
        &mut self,
        name: &str,
        nodes: &[tree_sitter::Node<'a>],
        source: &'a str,
    ) -> bool {
        if name == "_" {
            return true;
        }

        let captured_nodes: Vec<_> = nodes
            .iter()
            .map(|node| CapturedNode {
                node: *node,
                text: source.get(node.byte_range()).unwrap_or_default(),
            })
            .collect();

        let (byte_range, text) = if let (Some(first), Some(last)) =
            (nodes.first().copied(), nodes.last().copied())
        {
            let start = first.start_byte();
            let end = last.end_byte();
            let range = start..end;
            (range.clone(), source.get(range).unwrap_or_default())
        } else {
            (0..0, "")
        };

        let value = CapturedValue::Multiple(CapturedNodes {
            nodes: captured_nodes,
            text,
            byte_range,
        });

        self.insert_consistent(name, value)
    }

    fn insert_consistent(&mut self, name: &str, next: CapturedValue<'a>) -> bool {
        let Some(existing) = self.inner.get(name) else {
            self.inner.insert(name.to_owned(), next);
            return true;
        };

        match (existing, &next) {
            (CapturedValue::Single(a), CapturedValue::Single(b)) => {
                a.node.kind() == b.node.kind() && a.text == b.text
            }
            (CapturedValue::Multiple(a), CapturedValue::Multiple(b)) => {
                a.text == b.text
                    && a.nodes.len() == b.nodes.len()
                    && a
                        .nodes
                        .iter()
                        .zip(b.nodes.iter())
                        .all(|(left, right)| left.node.kind() == right.node.kind() && left.text == right.text)
            }
            _ => false,
        }
        .then(|| {
            self.inner.insert(name.to_owned(), next);
        })
        .is_some()
    }
}
