//! Tree-sitter parsing wrapper with error recovery.
//!
//! This module provides a high-level interface for parsing source code using
//! Tree-sitter. It wraps the raw Tree-sitter parser and provides structured
//! access to parse results and syntax errors.

use std::ops::Range;

use crate::error::SyntaxError;
use crate::language::SupportedLanguage;
use crate::position::point_to_one_based;

/// Result of parsing source code.
///
/// Contains the parsed syntax tree along with metadata about any errors
/// encountered during parsing. Tree-sitter is error-tolerant, so a parse
/// result may contain both a valid tree and error nodes.
#[derive(Debug)]
pub struct ParseResult {
    tree: tree_sitter::Tree,
    source: String,
    language: SupportedLanguage,
}

impl ParseResult {
    /// Returns the parsed syntax tree.
    #[must_use]
    pub const fn tree(&self) -> &tree_sitter::Tree {
        &self.tree
    }

    /// Returns the source code that was parsed.
    #[must_use]
    pub fn source(&self) -> &str {
        &self.source
    }

    /// Returns the language of the parsed code.
    #[must_use]
    pub const fn language(&self) -> SupportedLanguage {
        self.language
    }

    /// Returns whether the parse result contains any syntax errors.
    ///
    /// Tree-sitter produces ERROR nodes for portions of the source that
    /// could not be parsed according to the grammar.
    #[must_use]
    pub fn has_errors(&self) -> bool {
        has_error_nodes(self.tree.root_node())
    }

    /// Collects all syntax errors found in the parse result.
    ///
    /// Each error includes position information and a description of the
    /// problem.
    #[must_use]
    pub fn errors(&self) -> Vec<SyntaxErrorInfo> {
        let mut errors = Vec::new();
        collect_error_nodes(self.tree.root_node(), &self.source, &mut errors);
        errors
    }

    /// Returns the root node of the syntax tree.
    #[must_use]
    pub fn root_node(&self) -> tree_sitter::Node<'_> {
        self.tree.root_node()
    }
}

/// Information about a syntax error found during parsing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SyntaxErrorInfo {
    /// Byte range of the error in the source.
    pub byte_range: Range<usize>,
    /// Line number (one-based) where the error starts.
    pub line: u32,
    /// Column number (one-based) where the error starts.
    pub column: u32,
    /// A snippet of the problematic source text.
    pub context: String,
    /// Human-readable description of the error.
    pub message: String,
}

impl SyntaxErrorInfo {
    /// Creates a new syntax error info from a Tree-sitter node.
    fn from_node(node: tree_sitter::Node<'_>, source: &str) -> Self {
        let start = node.start_position();
        let byte_range = node.byte_range();

        // Extract context: the text of the error node, truncated if too long
        let context = source
            .get(byte_range.clone())
            .map(|s| {
                if s.len() > 50 {
                    let truncated: String = s.chars().take(47).collect();
                    format!("{truncated}...")
                } else {
                    s.to_owned()
                }
            })
            .unwrap_or_default();

        let message = if node.is_missing() {
            format!("missing {}", node.kind())
        } else {
            "syntax error".to_owned()
        };

        let (line, column) = point_to_one_based(start);

        Self {
            byte_range,
            line,
            column,
            context,
            message,
        }
    }
}

/// Tree-sitter parser wrapper for a specific language.
///
/// Each parser instance is configured for a single language. Create multiple
/// parsers if you need to parse multiple languages.
pub struct Parser {
    inner: tree_sitter::Parser,
    language: SupportedLanguage,
}

impl Parser {
    /// Creates a new parser for the given language.
    ///
    /// # Errors
    ///
    /// Returns an error if the Tree-sitter parser cannot be initialised
    /// with the language grammar.
    pub fn new(language: SupportedLanguage) -> Result<Self, SyntaxError> {
        let mut inner = tree_sitter::Parser::new();
        inner
            .set_language(&language.tree_sitter_language())
            .map_err(|e| SyntaxError::parser_init(language, e.to_string()))?;

        Ok(Self { inner, language })
    }

    /// Returns the language this parser is configured for.
    #[must_use]
    pub const fn language(&self) -> SupportedLanguage {
        self.language
    }

    /// Parses source code and returns the result.
    ///
    /// Tree-sitter is error-tolerant, so this method will return a parse
    /// result even if the source contains syntax errors. Use
    /// [`ParseResult::has_errors`] to check for errors.
    ///
    /// # Errors
    ///
    /// Returns an error if the parser fails to produce a syntax tree. This
    /// is rare and typically indicates a parser configuration issue.
    pub fn parse(&mut self, source: &str) -> Result<ParseResult, SyntaxError> {
        let tree = self
            .inner
            .parse(source, None)
            .ok_or_else(|| SyntaxError::parse(self.language, "parsing failed"))?;

        Ok(ParseResult {
            tree,
            source: source.to_owned(),
            language: self.language,
        })
    }
}

/// Recursively checks if a node or any of its descendants is an ERROR node.
fn has_error_nodes(node: tree_sitter::Node<'_>) -> bool {
    if node.is_error() || node.is_missing() {
        return true;
    }

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if has_error_nodes(child) {
            return true;
        }
    }

    false
}

/// Recursively collects all ERROR nodes from a syntax tree.
fn collect_error_nodes(
    node: tree_sitter::Node<'_>,
    source: &str,
    errors: &mut Vec<SyntaxErrorInfo>,
) {
    if node.is_error() || node.is_missing() {
        errors.push(SyntaxErrorInfo::from_node(node, source));
    }

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        collect_error_nodes(child, source, errors);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;

    #[rstest]
    #[case(SupportedLanguage::Rust, "fn main() {}")]
    #[case(SupportedLanguage::Python, "def hello():\n    pass")]
    #[case(
        SupportedLanguage::TypeScript,
        "function hello(): string { return 'hi'; }"
    )]
    fn parser_parses_valid_source(#[case] language: SupportedLanguage, #[case] source: &str) {
        let mut parser = Parser::new(language).expect("parser init");
        let result = parser.parse(source).expect("parse");

        assert!(!result.has_errors());
        assert_eq!(result.language(), language);
    }

    #[rstest]
    #[case(SupportedLanguage::Rust, "fn broken() {")]
    #[case(SupportedLanguage::Python, "def broken(")]
    #[case(SupportedLanguage::TypeScript, "function broken( {")]
    fn parser_detects_syntax_errors(#[case] language: SupportedLanguage, #[case] source: &str) {
        let mut parser = Parser::new(language).expect("parser init");
        let result = parser.parse(source).expect("parse");

        assert!(result.has_errors());
        assert!(!result.errors().is_empty());
    }

    #[test]
    fn syntax_error_info_has_line_and_column() {
        let mut parser = Parser::new(SupportedLanguage::Rust).expect("parser init");
        let result = parser.parse("fn test() {\n    let x = \n}").expect("parse");

        let errors = result.errors();
        assert!(!errors.is_empty());

        let first_error = errors.first().expect("has error");
        assert!(first_error.line >= 1);
        assert!(first_error.column >= 1);
    }
}
