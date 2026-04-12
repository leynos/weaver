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

use crate::{
    error::SyntaxError,
    language::SupportedLanguage,
    metavariables::{extract_metavar_name, placeholder_for_metavar},
    parser::{ParseResult, Parser},
};

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
    wrapped_in_function: bool,
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
        let raw = RawSource(source);
        let metavariables = extract_metavariables(raw)?;
        let normalised = normalise_metavariables(raw)?;

        let mut parser = Parser::new(language)?;
        let mut wrapped_in_function = false;
        let mut parsed = parser.parse(normalised.as_str())?;
        if parsed.has_errors() {
            let wrapped = wrap_pattern_for_parse(language, &normalised);
            parsed = parser.parse(&wrapped)?;
            wrapped_in_function = true;
        }

        if parsed.has_errors() {
            return Err(SyntaxError::pattern_compile(
                language,
                "pattern contains syntax errors",
            ));
        }

        Ok(Self {
            source: source.to_owned(),
            language,
            metavariables,
            parsed,
            wrapped_in_function,
        })
    }

    /// Returns the original pattern source.
    #[must_use]
    pub fn source(&self) -> &str { &self.source }

    /// Returns the language this pattern is compiled for.
    #[must_use]
    pub const fn language(&self) -> SupportedLanguage { self.language }

    pub(crate) const fn wrapped_in_function(&self) -> bool { self.wrapped_in_function }

    /// Returns the metavariables defined in this pattern.
    #[must_use]
    pub fn metavariables(&self) -> &[MetaVariable] { &self.metavariables }

    /// Returns the parsed syntax tree of the pattern.
    #[must_use]
    pub const fn parsed(&self) -> &ParseResult { &self.parsed }

    /// Returns whether this pattern has any metavariables.
    #[must_use]
    pub const fn has_metavariables(&self) -> bool { !self.metavariables.is_empty() }
}

/// Un-normalised pattern source, before metavariable substitution.
#[derive(Clone, Copy)]
struct RawSource<'a>(&'a str);

/// Pattern source after metavariable placeholders have been substituted.
#[derive(Debug)]
struct NormalisedSource(String);

impl NormalisedSource {
    fn as_str(&self) -> &str { &self.0 }
}

fn wrap_pattern_for_parse(language: SupportedLanguage, pattern: &NormalisedSource) -> String {
    let s = pattern.as_str();
    match language {
        SupportedLanguage::Rust => {
            format!(
                "fn __weaver_pattern_wrapper__() {{ {} }}",
                rust_pattern_wrapper_statement(pattern)
            )
        }
        SupportedLanguage::Python => python_pattern_wrapper(pattern),
        SupportedLanguage::TypeScript => {
            format!("function __weaver_pattern_wrapper__() {{ {s} }}")
        }
    }
}

fn rust_pattern_wrapper_statement(pattern: &NormalisedSource) -> String {
    let trimmed = pattern.as_str().trim_end();
    match trimmed.chars().last() {
        None | Some(';' | '}') => trimmed.to_owned(),
        Some(_) => format!("{trimmed};"),
    }
}

fn python_pattern_wrapper(pattern: &NormalisedSource) -> String {
    let s = pattern.as_str();
    let mut out = String::from("def __weaver_pattern_wrapper__():\n");
    if s.trim().is_empty() {
        out.push_str("    pass\n");
    } else {
        for line in s.lines() {
            out.push_str("    ");
            out.push_str(line);
            out.push('\n');
        }
    }
    out
}

fn normalise_metavariables(source: RawSource<'_>) -> Result<NormalisedSource, SyntaxError> {
    let mut out = String::with_capacity(source.0.len());

    visit_metavariables(source, |event| match event {
        MetavarEvent::Literal(ch) => out.push(ch),
        MetavarEvent::Metavar(metavar) => out.push_str(&placeholder_for_metavar(&metavar.name)),
    })?;

    Ok(NormalisedSource(out))
}

#[derive(Debug)]
struct MetavarReference {
    dollars: usize,
    name: String,
    offset: usize,
}

#[derive(Debug)]
enum MetavarEvent {
    Literal(char),
    Metavar(MetavarReference),
}

fn visit_metavariables<F>(source: RawSource<'_>, mut handler: F) -> Result<(), SyntaxError>
where
    F: FnMut(MetavarEvent),
{
    let mut chars = source.0.char_indices().peekable();
    while let Some((offset, ch)) = chars.next() {
        if ch != '$' {
            handler(MetavarEvent::Literal(ch));
            continue;
        }

        let mut dollar_count = 1;
        while chars.peek().is_some_and(|(_, c)| *c == '$') {
            chars.next();
            dollar_count += 1;
        }

        if dollar_count == 2 || dollar_count > 3 {
            return Err(SyntaxError::invalid_metavariable(format!(
                "metavariable at offset {offset} has invalid '$' prefix length ({dollar_count})"
            )));
        }

        let name = extract_metavar_name(&mut chars);
        if name.is_empty() {
            return Err(SyntaxError::invalid_metavariable(format!(
                "metavariable at offset {offset} has no valid name"
            )));
        }

        handler(MetavarEvent::Metavar(MetavarReference {
            dollars: dollar_count,
            name,
            offset,
        }));
    }

    Ok(())
}

/// Extracts metavariables from a pattern source string.
///
/// Scans the source for `$VAR` and `$$$VAR` patterns and returns
/// information about each metavariable found.
fn extract_metavariables(source: RawSource<'_>) -> Result<Vec<MetaVariable>, SyntaxError> {
    let mut metavariables = Vec::new();
    visit_metavariables(source, |event| {
        let MetavarEvent::Metavar(metavar) = event else {
            return;
        };

        let kind = if metavar.dollars == 3 {
            MetaVarKind::Multiple
        } else {
            MetaVarKind::Single
        };

        metavariables.push(MetaVariable {
            name: metavar.name,
            kind,
            offset: metavar.offset,
        });
    })?;

    Ok(metavariables)
}

#[cfg(test)]
mod tests {
    //! Unit tests for pattern metavariable extraction and validation.

    use super::*;

    #[test]
    fn extract_single_metavariable() {
        let metavars = extract_metavariables(RawSource("$VAR")).expect("extract");
        assert_eq!(metavars.len(), 1);
        assert_eq!(metavars.first().map(|m| m.name.as_str()), Some("VAR"));
        assert_eq!(metavars.first().map(|m| m.kind), Some(MetaVarKind::Single));
    }

    #[test]
    fn extract_multiple_metavariable() {
        let metavars = extract_metavariables(RawSource("$$$ARGS")).expect("extract");
        assert_eq!(metavars.len(), 1);
        assert_eq!(metavars.first().map(|m| m.name.as_str()), Some("ARGS"));
        assert_eq!(
            metavars.first().map(|m| m.kind),
            Some(MetaVarKind::Multiple)
        );
    }

    #[test]
    fn extract_multiple_metavariables() {
        let metavars = extract_metavariables(RawSource("$FUNC($ARG1, $ARG2)")).expect("extract");
        assert_eq!(metavars.len(), 3);

        let names: Vec<_> = metavars.iter().map(|m| m.name.as_str()).collect();
        assert_eq!(names, vec!["FUNC", "ARG1", "ARG2"]);
    }

    #[test]
    fn extract_wildcard() {
        let metavars = extract_metavariables(RawSource("$_")).expect("extract");
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

    #[test]
    fn compile_rejects_patterns_with_syntax_errors() {
        let result = Pattern::compile("fn (", SupportedLanguage::Rust);
        assert!(result.is_err());
    }

    #[test]
    fn wrap_rust_pattern_adds_statement_semicolon() {
        let src = NormalisedSource("dbg!($EXPR)".to_owned());
        let wrapped = wrap_pattern_for_parse(SupportedLanguage::Rust, &src);
        assert_eq!(wrapped, "fn __weaver_pattern_wrapper__() { dbg!($EXPR); }");
    }

    #[test]
    fn wrap_python_empty_pattern_uses_pass() {
        let src = NormalisedSource(" \n".to_owned());
        let wrapped = wrap_pattern_for_parse(SupportedLanguage::Python, &src);
        assert_eq!(wrapped, "def __weaver_pattern_wrapper__():\n    pass\n");
    }
}
