//! Semgrep-compatible YAML rule parsing for Sempai.
//!
//! This crate turns YAML rule files into schema-aligned Rust models while
//! preserving enough source information to report stable parser diagnostics.
//!
//! # Example
//!
//! ```
//! use sempai_yaml::parse_rule_file;
//!
//! let yaml = concat!(
//!     "rules:\n",
//!     "  - id: demo.rule\n",
//!     "    message: detect foo\n",
//!     "    languages: [rust]\n",
//!     "    severity: WARNING\n",
//!     "    pattern: foo($X)\n",
//! );
//!
//! let file = parse_rule_file(yaml, None).expect("valid rule file");
//! assert_eq!(file.rules()[0].id(), "demo.rule");
//! ```

mod model;
mod parser;
mod raw;
mod source_map;

pub use model::{
    ExtractQueryPrincipal, LegacyFormula, MatchFormula, Rule, RuleFile, RuleMode, RulePrincipal,
    RuleSeverity, SearchQueryPrincipal, TaintQueryPrincipal,
};
pub use parser::parse_rule_file;

#[cfg(test)]
mod tests;
