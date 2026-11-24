//! Capture types for pattern matching.
//!
//! Captures bind metavariable names to the parts of the source code they match.

use std::{collections::HashMap, ops::Range};

/// A single captured AST node.
#[derive(Debug, Clone)]
pub struct CapturedNode<'a> {
    /// The AST node matched by the pattern.
    node: tree_sitter::Node<'a>,
    /// The source text spanned by `node`, sliced once at capture time.
    text: &'a str,
}

impl<'a> CapturedNode<'a> {
    /// Returns the captured AST node.
    #[must_use]
    pub const fn node(&self) -> tree_sitter::Node<'a> { self.node }

    /// Returns the text of the captured node.
    #[must_use]
    pub const fn text(&self) -> &'a str { self.text }

    /// Returns the byte range of the captured node.
    #[must_use]
    pub fn byte_range(&self) -> Range<usize> { self.node.byte_range() }
}

/// A capture for a multiple-node metavariable (`$$$NAME`).
#[derive(Debug, Clone)]
pub struct CapturedNodes<'a> {
    /// The individual nodes matched by the `$$$NAME` capture, in source order.
    nodes: Vec<CapturedNode<'a>>,
    /// The source text spanning from the first node to the last, inclusive of
    /// any separators between them.
    text: &'a str,
    /// The byte range covering `text`; degenerates to an empty range at the
    /// anchor point when the capture matched zero nodes.
    byte_range: Range<usize>,
}

impl<'a> CapturedNodes<'a> {
    /// Returns the captured nodes in order.
    #[must_use]
    pub fn nodes(&self) -> &[CapturedNode<'a>] { &self.nodes }

    /// Returns the full source text covered by the capture.
    #[must_use]
    pub const fn text(&self) -> &'a str { self.text }

    /// Returns the byte range covered by the capture.
    #[must_use]
    pub fn byte_range(&self) -> Range<usize> { self.byte_range.clone() }
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

/// The set of metavariable bindings accumulated while matching a pattern
/// against a single AST subtree.
#[derive(Debug, Clone)]
pub(super) struct Captures<'a> {
    /// The full source text that `inner`'s captured ranges are sliced from.
    source: &'a str,
    /// Metavariable name to bound value, keyed by the name written in the pattern.
    inner: HashMap<String, CapturedValue<'a>>,
}

/// Slices `source` at a tree-sitter byte range, returning `""` (and tripping
/// a debug assertion) if the range falls outside `source`'s bounds — this
/// should not happen for ranges tree-sitter itself produced, but guards
/// against a corrupted or mismatched source string.
fn slice_source_range(source: &str, range: Range<usize>) -> &str {
    let start = range.start;
    let end = range.end;

    let Some(slice) = source.get(start..end) else {
        debug_assert!(
            false,
            "tree-sitter node byte range {start}..{end} is not valid for source length {}",
            source.len()
        );
        return "";
    };

    slice
}

impl<'a> Captures<'a> {
    /// Creates an empty capture set over `source`.
    pub(super) fn new(source: &'a str) -> Self {
        Self {
            source,
            inner: HashMap::new(),
        }
    }

    /// Consumes the capture set, yielding the raw name-to-value map for
    /// attachment to a completed match.
    pub(super) fn into_inner(self) -> HashMap<String, CapturedValue<'a>> { self.inner }

    /// Records a single-node capture for `name`, unless `name` is the
    /// wildcard `_`, which is never bound. Returns `false` if `name` was
    /// already bound to a value inconsistent with `node`.
    pub(super) fn capture_single(&mut self, name: &str, node: tree_sitter::Node<'a>) -> bool {
        if name == "_" {
            return true;
        }

        let text = slice_source_range(self.source, node.byte_range());
        let value = CapturedValue::Single(CapturedNode { node, text });

        self.insert_consistent(name, value)
    }

    /// Records a multi-node capture for `name`, unless `name` is the
    /// wildcard `_`. `empty_anchor_byte` anchors the capture's byte range
    /// when `nodes` is empty, since there is then no node to derive a range
    /// from. Returns `false` if `name` was already bound to an inconsistent
    /// value.
    pub(super) fn capture_multiple(
        &mut self,
        name: &str,
        nodes: &[tree_sitter::Node<'a>],
        empty_anchor_byte: usize,
    ) -> bool {
        if name == "_" {
            return true;
        }

        let captured_nodes: Vec<_> = nodes
            .iter()
            .map(|node| CapturedNode {
                node: *node,
                text: slice_source_range(self.source, node.byte_range()),
            })
            .collect();

        let (byte_range, text) =
            if let (Some(first), Some(last)) = (nodes.first().copied(), nodes.last().copied()) {
                let start = first.start_byte();
                let end = last.end_byte();
                let byte_range = start..end;
                let text = slice_source_range(self.source, start..end);
                (byte_range, text)
            } else {
                let byte_range = empty_anchor_byte..empty_anchor_byte;
                let text = slice_source_range(self.source, byte_range.clone());
                (byte_range, text)
            };

        let value = CapturedValue::Multiple(CapturedNodes {
            nodes: captured_nodes,
            text,
            byte_range,
        });

        self.insert_consistent(name, value)
    }

    /// Binds `name` to `next` if unbound, or if already bound to an
    /// equivalent value (same node kind and text) — repeated captures of the
    /// same metavariable within one pattern must agree. Returns whether the
    /// binding succeeded.
    fn insert_consistent(&mut self, name: &str, next: CapturedValue<'a>) -> bool {
        let Some(existing) = self.inner.get(name) else {
            self.inner.insert(name.to_owned(), next);
            return true;
        };

        let is_consistent = match (existing, &next) {
            (CapturedValue::Single(a), CapturedValue::Single(b)) => {
                a.node.kind() == b.node.kind() && a.text == b.text
            }
            (CapturedValue::Multiple(a), CapturedValue::Multiple(b)) => {
                a.text == b.text
                    && a.nodes.len() == b.nodes.len()
                    && a.nodes.iter().zip(b.nodes.iter()).all(|(left, right)| {
                        left.node.kind() == right.node.kind() && left.text == right.text
                    })
            }
            _ => false,
        };

        if is_consistent {
            self.inner.insert(name.to_owned(), next);
        }

        is_consistent
    }
}
