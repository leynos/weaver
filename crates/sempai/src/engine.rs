//! Engine and query plan types for the Sempai facade.
//!
//! The [`Engine`] compiles Semgrep-compatible queries from YAML rule files
//! or one-liner DSL expressions and executes them against source snapshots.
//! Compilation and execution are separate phases, allowing a compiled
//! [`QueryPlan`] to be reused across multiple source files.

use std::sync::Arc;

use sempai_core::{
    DiagnosticCode,
    DiagnosticReport,
    EngineConfig,
    Language,
    Match,
    formula::{Decorated, Formula},
};
use sempai_yaml::{Rule, RulePrincipal, parse_rule_file};

use crate::{
    mode_validation::validate_supported_modes,
    normalize::normalize_search_principal,
    semantic_check::{validate_constraints, validate_formula},
};

/// A compiled query plan for one rule and target language.
#[derive(Debug)]
pub struct QueryPlan {
    rule_id: String,
    language: Language,
    /// The normalized canonical formula.
    formula: Arc<Decorated<Formula>>,
}

impl QueryPlan {
    pub(crate) const fn new(
        rule_id: String,
        language: Language,
        formula: Arc<Decorated<Formula>>,
    ) -> Self {
        Self {
            rule_id,
            language,
            formula,
        }
    }

    /// Returns the rule identifier.
    #[must_use]
    pub fn rule_id(&self) -> &str { &self.rule_id }

    /// Returns the target language.
    #[must_use]
    pub const fn language(&self) -> Language { self.language }

    /// Returns the normalized canonical formula.
    #[must_use]
    pub fn formula(&self) -> &Decorated<Formula> { self.formula.as_ref() }
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
/// let result = engine.compile_yaml(
///     "rules:\n  - id: test\n    message: test\n    languages: [rust]\n    severity: ERROR\n    \
///      pattern: foo\n",
/// );
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
    pub const fn new(config: EngineConfig) -> Self { Self { config } }

    /// Returns the engine configuration.
    #[must_use]
    pub const fn config(&self) -> &EngineConfig { &self.config }

    /// Compiles a YAML rule file into query plans.
    ///
    /// # Errors
    ///
    /// Returns a diagnostic report if parsing, normalization, or validation fails.
    #[tracing::instrument(level = "info", skip_all, fields(rules = tracing::field::Empty))]
    pub fn compile_yaml(&self, yaml: &str) -> Result<Vec<QueryPlan>, DiagnosticReport> {
        let file = parse_rule_file(yaml, None)?;
        let rule_count = file.rules().len();
        tracing::Span::current().record("rules", rule_count);
        tracing::debug!(rules = rule_count, "yaml parsed successfully");
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
                tracing::debug!(rule_id = rule.id(), "normalizing principal");
                let formula = normalize_search_principal(principal, rule.rule_span())?;
                tracing::debug!(rule_id = rule.id(), "principal normalized");

                tracing::debug!(rule_id = rule.id(), "validating normalized formula");
                validate_formula(&formula)?;
                validate_constraints(&formula)?;

                tracing::debug!(
                    rule_id = rule.id(),
                    languages = ?rule.languages(),
                    "compiling rule plans"
                );
                let rule_plans = compile_rule_plans(rule, formula)?;
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

/// Compiles query plans for a single rule's languages.
fn compile_rule_plans(
    rule: &Rule,
    formula: Decorated<Formula>,
) -> Result<Vec<QueryPlan>, DiagnosticReport> {
    let shared_formula = Arc::new(formula);
    rule.languages()
        .iter()
        .map(|lang_str| {
            let _span = tracing::debug_span!(
                "compile_rule_plan",
                rule_id = rule.id(),
                language = lang_str.as_str()
            )
            .entered();
            let language = lang_str.parse::<Language>().map_err(|e| {
                DiagnosticReport::validation_error(
                    DiagnosticCode::ESempaiSchemaInvalid,
                    format!("unsupported language '{lang_str}': {e}"),
                    rule.rule_span().cloned(),
                    vec![],
                )
            })?;
            tracing::debug!("query plan created");
            Ok(QueryPlan::new(
                rule.id().to_owned(),
                language,
                Arc::clone(&shared_formula),
            ))
        })
        .collect()
}
