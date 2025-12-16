//! Tree-sitter powered syntactic analysis for the Weaver toolchain.
//!
//! This crate provides structural code analysis capabilities including:
//!
//! - **Syntactic validation** via [`TreeSitterSyntacticLock`] for the Double-Lock
//!   safety harness
//! - **Pattern matching** via [`Pattern`] for structural code search (powers
//!   `observe grep`)
//! - **Code rewriting** via [`Rewriter`] for structural transformations (powers
//!   `act apply-rewrite`)
//!
//! # Supported Languages
//!
//! Currently supports:
//! - Rust (`.rs`)
//! - Python (`.py`, `.pyi`)
//! - TypeScript (`.ts`, `.tsx`, `.mts`, `.cts`)
//!
//! # Pattern Language
//!
//! The pattern language is inspired by [ast-grep](https://ast-grep.github.io/)
//! and supports metavariables for capturing code elements:
//!
//! - `$VAR` - Matches any single AST node and captures it
//! - `$_` - Matches any single AST node without capturing (wildcard)
//! - `$$$VAR` - Matches zero or more AST nodes
//!
//! # Example: Pattern Matching
//!
//! ```
//! use weaver_syntax::{Pattern, Parser, SupportedLanguage};
//!
//! // Parse some Rust code
//! let mut parser = Parser::new(SupportedLanguage::Rust)?;
//! let source = parser.parse("fn main() { println!(\"hello\"); }")?;
//!
//! // Create a pattern to find function definitions
//! let pattern = Pattern::compile("fn $NAME() { $$$BODY }", SupportedLanguage::Rust)?;
//!
//! // Find matches
//! let matches = pattern.find_all(&source);
//! for m in matches {
//!     if let Some(name) = m.capture("NAME") {
//!         // Found function with this name
//!         let _ = name.text();
//!     }
//! }
//! # Ok::<(), weaver_syntax::SyntaxError>(())
//! ```
//!
//! # Example: Syntactic Validation
//!
//! ```
//! use std::path::Path;
//! use weaver_syntax::TreeSitterSyntacticLock;
//!
//! let lock = TreeSitterSyntacticLock::new();
//!
//! // Validate valid code
//! let failures = lock.validate_file(
//!     Path::new("main.rs"),
//!     "fn main() {}"
//! )?;
//! assert!(failures.is_empty());
//!
//! // Validate invalid code
//! let failures = lock.validate_file(
//!     Path::new("broken.rs"),
//!     "fn broken() {"
//! )?;
//! assert!(!failures.is_empty());
//! # Ok::<(), weaver_syntax::SyntaxError>(())
//! ```
//!
//! # Example: Code Rewriting
//!
//! ```
//! use weaver_syntax::{Pattern, RewriteRule, Rewriter, SupportedLanguage};
//!
//! // Create a rewrite rule
//! let pattern = Pattern::compile("dbg!($EXPR)", SupportedLanguage::Rust)?;
//! let rule = RewriteRule::new(pattern, "println!(\"{:?}\", $EXPR)")?;
//!
//! // Apply the rewrite
//! let rewriter = Rewriter::new(SupportedLanguage::Rust);
//! let result = rewriter.apply(&rule, "fn main() { dbg!(x); }")?;
//!
//! assert!(result.has_changes());
//! # Ok::<(), weaver_syntax::SyntaxError>(())
//! ```

mod error;
mod language;
mod matcher;
mod metavariables;
mod parser;
mod pattern;
mod position;
mod rewriter;
mod syntactic_lock;

pub use error::SyntaxError;
pub use language::{LanguageParseError, SupportedLanguage};
pub use matcher::{CapturedNode, CapturedNodes, CapturedValue, MatchResult, Matcher};
pub use parser::{ParseResult, Parser, SyntaxErrorInfo};
pub use pattern::{MetaVarKind, MetaVariable, Pattern};
pub use rewriter::{RewriteResult, RewriteRule, Rewriter};
pub use syntactic_lock::{OwnedFile, TreeSitterSyntacticLock, ValidationFailure};

#[cfg(test)]
mod tests;
