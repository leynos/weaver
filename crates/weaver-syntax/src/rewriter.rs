//! Structural rewrite engine for code transformations.
//!
//! This module implements code rewriting based on pattern matching. It allows
//! replacing matched code structures with new code, with support for
//! metavariable substitution in the replacement.

use crate::error::SyntaxError;
use crate::language::SupportedLanguage;
use crate::matcher::MatchResult;
use crate::metavariables::extract_metavar_name;
use crate::parser::Parser;
use crate::pattern::Pattern;

/// A structural rewrite rule.
///
/// Combines a pattern to match with a replacement template. Metavariables
/// captured by the pattern can be referenced in the replacement.
#[derive(Debug)]
pub struct RewriteRule {
    pattern: Pattern,
    replacement: String,
}

impl RewriteRule {
    /// Creates a new rewrite rule.
    ///
    /// # Arguments
    ///
    /// * `pattern` - The pattern to match
    /// * `replacement` - The replacement template (may contain `$VAR` references)
    ///
    /// # Errors
    ///
    /// Returns an error if the replacement contains invalid metavariable
    /// references.
    pub fn new(pattern: Pattern, replacement: impl Into<String>) -> Result<Self, SyntaxError> {
        let replacement_str = replacement.into();

        // Validate that replacement metavariables exist in the pattern
        use std::collections::HashSet;

        let pattern_vars: HashSet<_> = pattern
            .metavariables()
            .iter()
            .map(|m| m.name.as_str())
            .collect();
        let replacement_vars = extract_replacement_vars(&replacement_str);

        for var in &replacement_vars {
            if var != "_" && !pattern_vars.contains(var.as_str()) {
                return Err(SyntaxError::invalid_replacement(format!(
                    "replacement references undefined metavariable: ${var}"
                )));
            }
        }

        Ok(Self {
            pattern,
            replacement: replacement_str,
        })
    }

    /// Returns the pattern for this rule.
    #[must_use]
    pub const fn pattern(&self) -> &Pattern {
        &self.pattern
    }

    /// Returns the replacement template.
    #[must_use]
    pub fn replacement(&self) -> &str {
        &self.replacement
    }
}

/// Engine for applying structural rewrites.
pub struct Rewriter {
    language: SupportedLanguage,
}

impl Rewriter {
    /// Creates a new rewriter for the given language.
    #[must_use]
    pub const fn new(language: SupportedLanguage) -> Self {
        Self { language }
    }

    /// Returns the language this rewriter is configured for.
    #[must_use]
    pub const fn language(&self) -> SupportedLanguage {
        self.language
    }

    /// Applies a rewrite rule to source code.
    ///
    /// Finds all matches of the rule's pattern and replaces them with the
    /// replacement template, substituting captured metavariables.
    ///
    /// # Errors
    ///
    /// Returns an error if parsing fails.
    pub fn apply(&self, rule: &RewriteRule, source: &str) -> Result<RewriteResult, SyntaxError> {
        let mut parser = Parser::new(self.language)?;
        let parsed = parser.parse(source)?;

        let matches = rule.pattern.find_all(&parsed);
        if matches.is_empty() {
            return Ok(RewriteResult {
                output: source.to_owned(),
                num_replacements: 0,
            });
        }

        let output = Self::apply_replacements(source, &matches, &rule.replacement)?;

        Ok(RewriteResult {
            output,
            num_replacements: matches.len(),
        })
    }

    /// Applies multiple rewrite rules in sequence.
    ///
    /// Each rule is applied to the result of the previous rule.
    ///
    /// # Errors
    ///
    /// Returns an error if parsing fails during any rule application.
    pub fn apply_all(
        &self,
        rules: &[RewriteRule],
        source: &str,
    ) -> Result<RewriteResult, SyntaxError> {
        let mut current = source.to_owned();
        let mut total_replacements: usize = 0;

        for rule in rules {
            let result = self.apply(rule, &current)?;
            total_replacements = total_replacements.saturating_add(result.num_replacements);
            current = result.output;
        }

        Ok(RewriteResult {
            output: current,
            num_replacements: total_replacements,
        })
    }

    /// Applies replacements to source code based on matches.
    fn apply_replacements(
        source: &str,
        matches: &[MatchResult<'_>],
        replacement_template: &str,
    ) -> Result<String, SyntaxError> {
        // Sort matches by byte offset (descending) to replace from end to start
        // This preserves earlier offsets when replacing
        let mut sorted_matches: Vec<_> = matches.iter().collect();
        sorted_matches.sort_by(|a, b| b.byte_range().start.cmp(&a.byte_range().start));

        let mut result = source.to_owned();

        for m in sorted_matches {
            let replacement = substitute_metavariables(replacement_template, m);
            let range = m.byte_range();

            // Replace in the result string
            if range.start > result.len() || range.end > result.len() {
                continue;
            }
            if !result.is_char_boundary(range.start) || !result.is_char_boundary(range.end) {
                return Err(SyntaxError::internal_error(
                    "rewrite match range is not on a UTF-8 boundary",
                ));
            }

            result.replace_range(range, &replacement);
        }

        Ok(result)
    }
}

/// Result of a rewrite operation.
#[derive(Debug, Clone)]
pub struct RewriteResult {
    /// The transformed source code.
    output: String,
    /// Number of replacements made.
    num_replacements: usize,
}

impl RewriteResult {
    /// Returns the transformed source code.
    #[must_use]
    pub fn output(&self) -> &str {
        &self.output
    }

    /// Returns the number of replacements made.
    #[must_use]
    pub const fn num_replacements(&self) -> usize {
        self.num_replacements
    }

    /// Returns whether any replacements were made.
    #[must_use]
    pub const fn has_changes(&self) -> bool {
        self.num_replacements > 0
    }
}

/// Extracts metavariable references from a replacement template.
fn extract_replacement_vars(replacement: &str) -> Vec<String> {
    let mut vars = Vec::new();
    let mut chars = replacement.char_indices().peekable();

    while let Some((_, ch)) = chars.next() {
        if ch == '$' {
            let mut dollars = 1;
            while chars.peek().is_some_and(|(_, c)| *c == '$') {
                dollars += 1;
                chars.next();
            }

            let name = extract_metavar_name(&mut chars);
            if name.is_empty() || dollars == 2 {
                continue;
            }

            if dollars == 1 || dollars == 3 {
                vars.push(name);
            }
        }
    }

    vars
}

/// Substitutes metavariables in a replacement template with captured values.
fn substitute_metavariables(template: &str, match_result: &MatchResult<'_>) -> String {
    let mut out = String::with_capacity(template.len());
    let mut chars = template.char_indices().peekable();

    while let Some((_, ch)) = chars.next() {
        if ch != '$' {
            out.push(ch);
            continue;
        }

        let mut dollars = 1;
        while chars.peek().is_some_and(|(_, c)| *c == '$') {
            dollars += 1;
            chars.next();
        }

        let name = extract_metavar_name(&mut chars);
        if name.is_empty() || dollars == 2 {
            out.push_str(&"$".repeat(dollars));
            out.push_str(&name);
            continue;
        }

        if name == "_" {
            continue;
        }

        if dollars == 1 || dollars == 3 {
            if let Some(capture) = match_result.capture(&name) {
                out.push_str(capture.text());
            } else if dollars == 1 {
                out.push_str(&"$".repeat(dollars));
                out.push_str(&name);
            }
            continue;
        }

        out.push_str(&"$".repeat(dollars));
        out.push_str(&name);
    }

    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rewrite_simple_replacement() {
        let pattern =
            Pattern::compile("let $VAR = $VAL", SupportedLanguage::Rust).expect("pattern");
        let rule = RewriteRule::new(pattern, "const $VAR: _ = $VAL").expect("rule");

        let rewriter = Rewriter::new(SupportedLanguage::Rust);
        let result = rewriter
            .apply(&rule, "fn main() { let x = 1; }")
            .expect("rewrite");

        assert!(result.has_changes());
        assert!(result.output().contains("const"));
    }

    #[test]
    fn rewrite_no_match_returns_unchanged() {
        let pattern =
            Pattern::compile("struct $NAME {}", SupportedLanguage::Rust).expect("pattern");
        let rule = RewriteRule::new(pattern, "enum $NAME {}").expect("rule");

        let rewriter = Rewriter::new(SupportedLanguage::Rust);
        let source = "fn main() {}";
        let result = rewriter.apply(&rule, source).expect("rewrite");

        assert!(!result.has_changes());
        assert_eq!(result.output(), source);
    }

    #[test]
    fn rewrite_rule_validates_metavariables() {
        let pattern = Pattern::compile("fn $NAME() {}", SupportedLanguage::Rust).expect("pattern");
        let result = RewriteRule::new(pattern, "fn $UNDEFINED() {}");

        assert!(result.is_err());
    }

    #[test]
    fn extract_replacement_vars_finds_all() {
        let vars = extract_replacement_vars("$A + $B = $RESULT");
        assert_eq!(vars, vec!["A", "B", "RESULT"]);
    }

    #[test]
    fn extract_replacement_vars_handles_multiple_prefix() {
        let vars = extract_replacement_vars("f($$$ARGS)");
        assert_eq!(vars, vec!["ARGS"]);
    }
}
