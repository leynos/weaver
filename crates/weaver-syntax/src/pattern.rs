//! Pattern compilation for structural code matching.
//!
//! This module implements an ast-grep-inspired pattern language that allows
//! matching code structures using metavariables. Patterns are compiled from
//! source strings and can be used to find matches in parsed code.
//!
//! # Pattern Syntax
//!
//! - `$VAR` - Matches any single AST node and captures it as `VAR`
//! - `$_` - Matches any single AST node without capturing (wildcard)
//! - `$$$VAR` - Matches zero or more AST nodes and captures them as `VAR`
//!
//! Metavariable names must start with an uppercase letter or underscore,
//! followed by uppercase letters, digits, or underscores.

use crate::error::SyntaxError;
use crate::language::SupportedLanguage;
use crate::parser::{ParseResult, Parser};

/// A compiled structural pattern for matching code.
///
/// Patterns are compiled from source strings that contain metavariables
/// (placeholders starting with `$`). The pattern is parsed using the
/// appropriate Tree-sitter grammar, and metavariables are identified for
/// capture during matching.
#[derive(Debug)]
pub struct Pattern {
    source: String,
    language: SupportedLanguage,
    metavariables: Vec<MetaVariable>,
    parsed: ParseResult,
}

/// A metavariable in a pattern.
///
/// Metavariables are placeholders that match AST nodes during pattern matching.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MetaVariable {
    /// The name of the metavariable (without the `$` prefix).
    pub name: String,
    /// The kind of metavariable (single node, multiple nodes, etc.).
    pub kind: MetaVarKind,
    /// Byte offset where this metavariable appears in the pattern source.
    pub offset: usize,
}

/// The kind of metavariable.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MetaVarKind {
    /// Matches a single AST node (`$VAR`).
    Single,
    /// Matches zero or more AST nodes (`$$$VAR`).
    Multiple,
}

impl Pattern {
    /// Compiles a pattern string for the given language.
    ///
    /// The pattern source should be valid code for the target language,
    /// with metavariables (`$VAR`, `$$$VAR`) in place of code elements
    /// to match.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The pattern contains invalid metavariable syntax
    /// - The pattern cannot be parsed by the language grammar
    ///
    /// # Examples
    ///
    /// ```
    /// use weaver_syntax::{Pattern, SupportedLanguage};
    ///
    /// // Match any function call with a single argument
    /// let pattern = Pattern::compile("$FUNC($ARG)", SupportedLanguage::Rust)?;
    /// # Ok::<(), weaver_syntax::SyntaxError>(())
    /// ```
    pub fn compile(source: &str, language: SupportedLanguage) -> Result<Self, SyntaxError> {
        // Extract metavariables from the source
        let metavariables = extract_metavariables(source)?;

        // Parse the pattern as code
        let mut parser = Parser::new(language)?;
        let parsed = parser.parse(source)?;

        Ok(Self {
            source: source.to_owned(),
            language,
            metavariables,
            parsed,
        })
    }

    /// Returns the original pattern source.
    #[must_use]
    pub fn source(&self) -> &str {
        &self.source
    }

    /// Returns the language this pattern is compiled for.
    #[must_use]
    pub const fn language(&self) -> SupportedLanguage {
        self.language
    }

    /// Returns the metavariables defined in this pattern.
    #[must_use]
    pub fn metavariables(&self) -> &[MetaVariable] {
        &self.metavariables
    }

    /// Returns the parsed syntax tree of the pattern.
    #[must_use]
    pub const fn parsed(&self) -> &ParseResult {
        &self.parsed
    }

    /// Returns whether this pattern has any metavariables.
    #[must_use]
    pub fn has_metavariables(&self) -> bool {
        !self.metavariables.is_empty()
    }
}

/// Returns whether `c` is a valid first character for a metavariable name.
///
/// Metavariable names must begin with an ASCII uppercase letter or `_`.
const fn is_valid_metavar_start_char(c: char) -> bool {
    c.is_ascii_uppercase() || c == '_'
}

/// Returns whether `c` is a valid continuation character for a metavariable name.
///
/// After the first character, metavariable names may contain ASCII uppercase
/// letters, ASCII digits, or `_`.
const fn is_valid_metavar_continuation_char(c: char) -> bool {
    c.is_ascii_uppercase() || c.is_ascii_digit() || c == '_'
}

/// Helper to extract a metavariable name from a character stream.
fn extract_metavar_name(chars: &mut std::iter::Peekable<std::str::CharIndices<'_>>) -> String {
    let mut name = String::new();

    // First character must be uppercase or underscore
    let Some((_, first_char)) = chars.peek().copied() else {
        return name;
    };

    if !is_valid_metavar_start_char(first_char) {
        return name;
    }

    name.push(first_char);
    chars.next();

    // Subsequent characters: uppercase, digit, or underscore
    while let Some((_, c)) = chars.peek().copied() {
        if !is_valid_metavar_continuation_char(c) {
            break;
        }
        name.push(c);
        chars.next();
    }

    name
}

/// Extracts metavariables from a pattern source string.
///
/// Scans the source for `$VAR` and `$$$VAR` patterns and returns
/// information about each metavariable found.
fn extract_metavariables(source: &str) -> Result<Vec<MetaVariable>, SyntaxError> {
    let mut metavariables = Vec::new();
    let mut chars = source.char_indices().peekable();

    while let Some((offset, ch)) = chars.next() {
        if ch == '$' {
            // Check for multiple-match prefix ($$$)
            let mut dollar_count = 1;
            while chars.peek().is_some_and(|(_, c)| *c == '$') {
                chars.next();
                dollar_count += 1;
            }

            let kind = if dollar_count >= 3 {
                MetaVarKind::Multiple
            } else {
                MetaVarKind::Single
            };

            // Extract the metavariable name
            let name_start = chars.peek().map(|(i, _)| *i);
            let name = extract_metavar_name(&mut chars);

            if name.is_empty() {
                return Err(SyntaxError::invalid_metavariable(format!(
                    "metavariable at offset {offset} has no valid name"
                )));
            }

            metavariables.push(MetaVariable {
                name,
                kind,
                offset: name_start.unwrap_or(offset),
            });
        }
    }

    Ok(metavariables)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extract_single_metavariable() {
        let metavars = extract_metavariables("$VAR").expect("extract");
        assert_eq!(metavars.len(), 1);
        assert_eq!(metavars.first().map(|m| m.name.as_str()), Some("VAR"));
        assert_eq!(metavars.first().map(|m| m.kind), Some(MetaVarKind::Single));
    }

    #[test]
    fn extract_multiple_metavariable() {
        let metavars = extract_metavariables("$$$ARGS").expect("extract");
        assert_eq!(metavars.len(), 1);
        assert_eq!(metavars.first().map(|m| m.name.as_str()), Some("ARGS"));
        assert_eq!(
            metavars.first().map(|m| m.kind),
            Some(MetaVarKind::Multiple)
        );
    }

    #[test]
    fn extract_multiple_metavariables() {
        let metavars = extract_metavariables("$FUNC($ARG1, $ARG2)").expect("extract");
        assert_eq!(metavars.len(), 3);

        let names: Vec<_> = metavars.iter().map(|m| m.name.as_str()).collect();
        assert_eq!(names, vec!["FUNC", "ARG1", "ARG2"]);
    }

    #[test]
    fn extract_wildcard() {
        let metavars = extract_metavariables("$_").expect("extract");
        assert_eq!(metavars.len(), 1);
        assert_eq!(metavars.first().map(|m| m.name.as_str()), Some("_"));
    }

    #[test]
    fn compile_rust_pattern() {
        let pattern = Pattern::compile("fn $NAME() {}", SupportedLanguage::Rust).expect("compile");
        assert_eq!(pattern.language(), SupportedLanguage::Rust);
        assert!(pattern.has_metavariables());

        let metavars = pattern.metavariables();
        assert_eq!(metavars.len(), 1);
        assert_eq!(metavars.first().map(|m| m.name.as_str()), Some("NAME"));
    }

    #[test]
    fn compile_python_pattern() {
        let pattern =
            Pattern::compile("def $FUNC($$$ARGS):", SupportedLanguage::Python).expect("compile");
        assert_eq!(pattern.language(), SupportedLanguage::Python);

        let metavars = pattern.metavariables();
        assert_eq!(metavars.len(), 2);
    }

    #[test]
    fn pattern_without_metavariables() {
        let pattern = Pattern::compile("fn main() {}", SupportedLanguage::Rust).expect("compile");
        assert!(!pattern.has_metavariables());
    }
}
