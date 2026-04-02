//! Engine and query plan types for the Sempai facade.
//!
//! The [`Engine`] compiles Semgrep-compatible queries from YAML rule files
//! or one-liner DSL expressions and executes them against source snapshots.
//! Compilation and execution are separate phases, allowing a compiled
//! [`QueryPlan`] to be reused across multiple source files.

use sempai_core::{DiagnosticReport, EngineConfig, Language, Match};
use sempai_yaml::parse_rule_file;

use crate::mode_validation::validate_supported_modes;

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
    /// Placeholder for the internal plan representation.  Will be replaced
    /// by `sempai_core::PlanNode` once the normalization layer is built.
    _plan: (),
}

impl QueryPlan {
    /// Creates a new query plan (crate-internal).
    // FIXME: remove `#[cfg(test)]` when `compile_yaml` / `compile_dsl` produce
    // real plans — https://github.com/leynos/weaver/issues/67
    #[cfg(test)]
    #[must_use]
    #[expect(
        clippy::missing_const_for_fn,
        reason = "heap types cannot be used in const contexts"
    )]
    pub(crate) fn new(rule_id: String, language: Language) -> Self {
        Self {
            rule_id,
            language,
            _plan: (),
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
/// let result = engine.compile_yaml("rules: []");
/// // Malformed YAML and schema errors now surface parser diagnostics.
/// assert!(result.is_err());
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
    /// Returns a diagnostic report if parsing or validation fails.
    ///
    /// Successful YAML parsing still stops at the post-parse placeholder until
    /// rule normalization is implemented.
    pub fn compile_yaml(&self, yaml: &str) -> Result<Vec<QueryPlan>, DiagnosticReport> {
        let file = parse_rule_file(yaml, None)?;
        validate_supported_modes(&file)?;
        Err(DiagnosticReport::not_implemented(
            "compile_yaml query-plan normalization",
        ))
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
