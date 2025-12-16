//! Pattern matching engine for finding code structures.
//!
//! This module implements a structural matcher inspired by ast-grep. It walks a
//! parsed Tree-sitter syntax tree and yields matches alongside captured
//! metavariables.

mod capture;
mod context;
mod matching;

use std::collections::HashMap;
use std::ops::Range;

use crate::parser::ParseResult;
use crate::pattern::Pattern;
use crate::position::point_to_one_based;

pub use capture::{CapturedNode, CapturedNodes, CapturedValue};

/// Result of a successful pattern match.
#[derive(Debug)]
pub struct MatchResult<'a> {
    node: tree_sitter::Node<'a>,
    source: &'a str,
    captures: HashMap<String, CapturedValue<'a>>,
}

impl<'a> MatchResult<'a> {
    /// Returns the matched AST node.
    #[must_use]
    pub const fn node(&self) -> tree_sitter::Node<'a> {
        self.node
    }

    /// Returns the byte range of the match in the source.
    #[must_use]
    pub fn byte_range(&self) -> Range<usize> {
        self.node.byte_range()
    }

    /// Returns the text of the matched region.
    #[must_use]
    pub fn text(&self) -> &'a str {
        self.source.get(self.byte_range()).unwrap_or_default()
    }

    /// Returns the start position (line, column) of the match.
    ///
    /// Both line and column are one-based for display purposes.
    #[must_use]
    pub fn start_position(&self) -> (u32, u32) {
        point_to_one_based(self.node.start_position())
    }

    /// Returns the end position (line, column) of the match.
    ///
    /// Both line and column are one-based for display purposes.
    #[must_use]
    pub fn end_position(&self) -> (u32, u32) {
        point_to_one_based(self.node.end_position())
    }

    /// Gets a captured metavariable by name.
    #[must_use]
    pub fn capture(&self, name: &str) -> Option<&CapturedValue<'a>> {
        self.captures.get(name)
    }

    /// Returns all captured metavariables.
    #[must_use]
    pub const fn captures(&self) -> &HashMap<String, CapturedValue<'a>> {
        &self.captures
    }
}

/// Pattern matcher that finds occurrences in parsed code.
pub struct Matcher<'p> {
    pattern: &'p Pattern,
}

impl<'p> Matcher<'p> {
    /// Creates a new matcher for the given pattern.
    #[must_use]
    pub const fn new(pattern: &'p Pattern) -> Self {
        Self { pattern }
    }

    /// Finds all matches of the pattern in the parsed source.
    #[must_use]
    pub fn find_all<'a>(&self, parsed: &'a ParseResult) -> Vec<MatchResult<'a>> {
        matching::find_all(self.pattern, parsed)
    }

    /// Finds the first match of the pattern in the parsed source.
    #[must_use]
    pub fn find_first<'a>(&self, parsed: &'a ParseResult) -> Option<MatchResult<'a>> {
        matching::find_first(self.pattern, parsed)
    }
}

impl Pattern {
    /// Finds all matches of this pattern in the parsed source.
    #[must_use]
    pub fn find_all<'a>(&self, parsed: &'a ParseResult) -> Vec<MatchResult<'a>> {
        Matcher::new(self).find_all(parsed)
    }

    /// Finds the first match of this pattern in the parsed source.
    #[must_use]
    pub fn find_first<'a>(&self, parsed: &'a ParseResult) -> Option<MatchResult<'a>> {
        Matcher::new(self).find_first(parsed)
    }
}

#[cfg(test)]
mod tests;
