//! Engine and query plan types for the Sempai facade.
//!
//! The [`Engine`] compiles Semgrep-compatible queries from YAML rule files
//! or one-liner DSL expressions and executes them against source snapshots.
//! Compilation and execution are separate phases, allowing a compiled
//! [`QueryPlan`] to be reused across multiple source files.

use sempai_core::formula::{Decorated, Formula};
use sempai_core::{DiagnosticCode, DiagnosticReport, EngineConfig, Language, Match};
use sempai_yaml::{Rule, RulePrincipal, parse_rule_file};

use crate::mode_validation::validate_supported_modes;
use crate::normalize::normalize_search_principal;
use crate::semantic_check::validate_formula;

/// Compiles query plans for a single rule's languages.
fn compile_rule_plans(
    rule: &Rule,
    formula: &Decorated<Formula>,
) -> Result<Vec<QueryPlan>, DiagnosticReport> {
    rule.languages()
        .iter()
        .map(|lang_str| {
            let language = lang_str.parse::<Language>().map_err(|e| {
                DiagnosticReport::validation_error(
                    DiagnosticCode::ESempaiSchemaInvalid,
                    format!("unsupported language '{lang_str}': {e}"),
                    rule.rule_span().cloned(),
                    vec![],
                )
            })?;
            Ok(QueryPlan::new(
                rule.id().to_owned(),
                language,
                formula.clone(),
            ))
        })
        .collect()
}

/// A compiled query plan for one rule and one language.
///
/// Query plans are produced by [`Engine::compile_yaml`] or
/// [`Engine::compile_dsl`] and can be executed against source snapshots
/// via [`Engine::execute`].
///
/// # Example
///
/// ```
/// use sempai::{Engine, EngineConfig, Language};
///
/// let engine = Engine::new(EngineConfig::default());
/// // compile_dsl currently returns an error (not yet implemented)
/// let result = engine.compile_dsl("rule-1", Language::Rust, "pattern(\"fn $F\")");
/// assert!(result.is_err());
/// ```
#[derive(Debug, Clone)]
pub struct QueryPlan {
    rule_id: String,
    language: Language,
    /// The normalized canonical formula.
    formula: Decorated<Formula>,
}

impl QueryPlan {
    /// Creates a new query plan (crate-internal).
    #[must_use]
    #[expect(
        clippy::missing_const_for_fn,
        reason = "heap types cannot be used in const contexts"
    )]
    pub(crate) fn new(rule_id: String, language: Language, formula: Decorated<Formula>) -> Self {
        Self {
            rule_id,
            language,
            formula,
        }
    }

    /// Returns the rule identifier.
    #[must_use]
    pub fn rule_id(&self) -> &str {
        &self.rule_id
    }

    /// Returns the target language.
    #[must_use]
    pub const fn language(&self) -> Language {
        self.language
    }

    /// Returns the normalized canonical formula.
    #[must_use]
    pub const fn formula(&self) -> &Decorated<Formula> {
        &self.formula
    }
}

/// Compiles and executes Semgrep-compatible queries on Tree-sitter syntax
/// trees.
///
/// The engine is the primary entrypoint for the Sempai query pipeline.  It
/// accepts YAML rule files or one-liner DSL expressions, compiles them into
/// [`QueryPlan`] objects, and executes those plans against source snapshots
/// to produce [`Match`] results.
///
/// # Example
///
/// ```
/// use sempai::{Engine, EngineConfig};
///
/// let engine = Engine::new(EngineConfig::default());
/// // Valid YAML with rules compiles successfully
/// let result = engine.compile_yaml("rules:\n  - id: test\n    message: test\n    languages: [rust]\n    severity: ERROR\n    pattern: foo\n");
/// assert!(result.is_ok());
///
/// // Malformed YAML returns a parser diagnostic
/// let bad_result = engine.compile_yaml("{ invalid yaml");
/// assert!(bad_result.is_err());
/// ```
#[derive(Debug)]
pub struct Engine {
    config: EngineConfig,
}

impl Engine {
    /// Creates a new engine with the given configuration.
    #[must_use]
    pub const fn new(config: EngineConfig) -> Self {
        Self { config }
    }

    /// Returns the engine configuration.
    #[must_use]
    pub const fn config(&self) -> &EngineConfig {
        &self.config
    }

    /// Compiles a YAML rule file into query plans.
    ///
    /// # Errors
    ///
    /// Returns a diagnostic report if parsing, normalization, or validation fails.
    pub fn compile_yaml(&self, yaml: &str) -> Result<Vec<QueryPlan>, DiagnosticReport> {
        let file = parse_rule_file(yaml, None)?;
        validate_supported_modes(&file)?;

        file.rules()
            .iter()
            .filter_map(|rule| {
                if let RulePrincipal::Search(principal) = rule.principal() {
                    Some((rule, principal))
                } else {
                    None
                }
            })
            .try_fold(Vec::new(), |mut plans, (rule, principal)| {
                let formula = normalize_search_principal(principal, rule.rule_span())?;
                validate_formula(&formula)?;
                let rule_plans = compile_rule_plans(rule, &formula)?;
                plans.extend(rule_plans);
                Ok(plans)
            })
    }

    /// Compiles a one-liner query DSL expression into a query plan.
    ///
    /// # Errors
    ///
    /// Returns a diagnostic report if parsing or validation fails.
    /// Currently returns a "not implemented" diagnostic for all inputs.
    pub fn compile_dsl(
        &self,
        _rule_id: &str,
        _language: Language,
        _dsl: &str,
    ) -> Result<QueryPlan, DiagnosticReport> {
        Err(DiagnosticReport::not_implemented("compile_dsl"))
    }

    /// Executes a compiled query plan against a source snapshot.
    ///
    /// # Errors
    ///
    /// Returns a diagnostic report if execution fails.
    /// Currently returns a "not implemented" diagnostic for all inputs.
    pub fn execute(
        &self,
        _plan: &QueryPlan,
        _uri: &str,
        _source: &str,
    ) -> Result<Vec<Match>, DiagnosticReport> {
        Err(DiagnosticReport::not_implemented("execute"))
    }
}
