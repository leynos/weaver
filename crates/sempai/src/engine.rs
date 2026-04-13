//! Engine and query plan types for the Sempai facade.
//!
//! The [`Engine`] compiles Semgrep-compatible queries from YAML rule files
//! or one-liner DSL expressions and executes them against source snapshots.
//! Compilation and execution are separate phases, allowing a compiled
//! [`QueryPlan`] to be reused across multiple source files.

use sempai_core::formula::Formula;
use sempai_core::{DiagnosticReport, EngineConfig, Language, Match};
use sempai_yaml::{RuleMode, RulePrincipal, parse_rule_file};

use crate::mode_validation::validate_supported_modes;
use crate::normalise::{normalise_search_principal, validate_formula_constraints};

/// A compiled query plan for one rule and one language.
///
/// Query plans are produced by [`Engine::compile_yaml`] or
/// [`Engine::compile_dsl`] and can be executed against source snapshots
/// via [`Engine::execute`].
///
/// # Example
///
/// ```
/// use sempai::{Engine, EngineConfig};
///
/// let yaml = concat!(
///     "rules:\n",
///     "  - id: demo.rule\n",
///     "    message: detect foo\n",
///     "    languages: [rust]\n",
///     "    severity: ERROR\n",
///     "    pattern: foo($X)\n",
/// );
/// let engine = Engine::new(EngineConfig::default());
/// let plans = engine.compile_yaml(yaml).expect("valid rule");
/// assert_eq!(plans.len(), 1);
/// assert!(plans[0].formula().is_some());
/// ```
#[derive(Debug, Clone)]
pub struct QueryPlan {
    rule_id: String,
    language: Language,
    formula: Option<Formula>,
}

impl QueryPlan {
    /// Creates a new query plan.
    #[must_use]
    #[expect(
        clippy::missing_const_for_fn,
        reason = "heap types cannot be used in const contexts"
    )]
    pub(crate) fn new(rule_id: String, language: Language, formula: Option<Formula>) -> Self {
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

    /// Returns the normalised formula, if this rule has formula semantics.
    ///
    /// `ProjectDependsOn` rules return `None` because they have no
    /// formula representation.
    #[must_use]
    pub const fn formula(&self) -> Option<&Formula> {
        self.formula.as_ref()
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
/// // Malformed YAML surfaces parser diagnostics.
/// assert!(engine.compile_yaml("not valid yaml: [").is_err());
/// // A valid empty rule file produces an empty plan list.
/// let plans = engine.compile_yaml("rules: []").expect("valid");
/// assert!(plans.is_empty());
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
    /// Each search-mode rule produces one [`QueryPlan`] per declared
    /// language.  The plan carries a normalised [`Formula`] (or `None`
    /// for `ProjectDependsOn` rules).
    ///
    /// # Errors
    ///
    /// Returns a diagnostic report if parsing, mode validation,
    /// normalization, or semantic constraint validation fails.
    pub fn compile_yaml(&self, yaml: &str) -> Result<Vec<QueryPlan>, DiagnosticReport> {
        let file = parse_rule_file(yaml, None)?;
        validate_supported_modes(&file)?;

        let mut plans = Vec::new();
        for rule in file.rules() {
            if *rule.mode() != RuleMode::Search {
                continue;
            }
            let RulePrincipal::Search(principal) = rule.principal() else {
                continue;
            };

            let formula = normalise_search_principal(principal)?;
            if let Some(ref f) = formula {
                validate_formula_constraints(f)?;
            }

            compile_rule_languages(rule.id(), rule.languages(), formula.as_ref(), &mut plans)?;
        }
        Ok(plans)
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

/// Parses declared languages and appends query plans for a single rule.
fn compile_rule_languages(
    rule_id: &str,
    languages: &[String],
    formula: Option<&Formula>,
    plans: &mut Vec<QueryPlan>,
) -> Result<(), DiagnosticReport> {
    for lang_str in languages {
        let language: Language = lang_str
            .parse()
            .map_err(|_| DiagnosticReport::not_implemented(&format!("language `{lang_str}`")))?;
        plans.push(QueryPlan::new(
            String::from(rule_id),
            language,
            formula.cloned(),
        ));
    }
    Ok(())
}
