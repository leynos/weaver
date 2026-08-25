//! Raw serde-deserializable types mirroring the YAML schema.
//!
//! These types capture the on-disk shape of a Semgrep rule document exactly as
//! written, before semantic validation. Serde validates this raw shape,
//! rejecting unknown fields on strict mappings, while conversion into the
//! typed `model` layer applies additional semantic constraints and reports
//! violations as [`sempai_core::DiagnosticReport`] values. The conversion code
//! lives in [`convert`].
use serde::Deserialize;
use serde_json::Value;
use serde_saphyr::Spanned;
/// Whole rule document: the top level of a Semgrep YAML file is a mapping
/// whose only recognized key is `rules`. Serde rejects unknown top-level
/// fields; semantic constraints are applied during conversion.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct RawRuleFile {
    /// Rules in document order; the index is used to report a rule's position
    /// when no span is available.
    pub(crate) rules: Vec<RawRule>,
}
/// A single entry of `rules`, captured before semantic validation. Every field
/// is optional so omissions surface as span-carrying diagnostics rather than
/// opaque serde errors, and unknown fields are tolerated for forward
/// compatibility with future Semgrep rule extensions.
#[derive(Debug, Deserialize)]
pub(crate) struct RawRule {
    /// Rule identifier as written, used to key findings and suppressions.
    pub(crate) id: Option<Spanned<String>>,
    /// Finding text shown to users, kept spanned so defects in it can be
    /// blamed on the right line.
    pub(crate) message: Option<Spanned<String>>,
    /// Languages the rule applies to, checked later against the supported set.
    pub(crate) languages: Option<Spanned<Vec<String>>>,
    /// Severity token exactly as written; decoded by [`parse_severity`].
    pub(crate) severity: Option<Spanned<String>>,
    /// Analysis mode token; absent means the default search mode.
    pub(crate) mode: Option<Spanned<String>>,
    /// Oldest Semgrep release the rule is declared to work with.
    #[serde(rename = "min-version")]
    pub(crate) min_version: Option<Spanned<String>>,
    /// Newest Semgrep release the rule is declared to work with.
    #[serde(rename = "max-version")]
    pub(crate) max_version: Option<Spanned<String>>,
    /// Legacy single-pattern form; mutually exclusive with the other
    /// top-level formula keys.
    pub(crate) pattern: Option<Spanned<String>>,
    /// Legacy regular-expression form of the rule body.
    #[serde(rename = "pattern-regex")]
    pub(crate) pattern_regex: Option<Spanned<String>>,
    /// Legacy conjunction: all clauses must hold at the same match site.
    pub(crate) patterns: Option<Spanned<Vec<RawLegacyClause>>>,
    /// Legacy disjunction: any one alternative suffices.
    #[serde(rename = "pattern-either")]
    pub(crate) pattern_either: Option<Spanned<Vec<RawLegacyFormulaObject>>>,
    /// Modern `match:` syntax, which supersedes the legacy `pattern*` keys.
    #[serde(rename = "match")]
    pub(crate) match_formula: Option<Spanned<RawMatchFormula>>,
    /// Dependency precondition block, validated by `project_depends_on`.
    #[serde(rename = "r2c-internal-project-depends-on")]
    pub(crate) project_depends_on: Option<Spanned<Value>>,
    /// Language that extracted content is analysed as, for extract mode.
    #[serde(rename = "dest-language")]
    pub(crate) dest_language: Option<Spanned<String>>,
    /// Metavariable whose captured text is re-analysed by extract mode.
    pub(crate) extract: Option<Spanned<String>>,
    /// Join mode configuration, retained untyped so rules using it are not
    /// rejected outright.
    pub(crate) join: Option<Spanned<Value>>,
    /// Taint mode configuration, recognized but not yet modelled.
    pub(crate) taint: Option<Spanned<Value>>,
    /// Taint sources, retained untyped alongside `taint`.
    #[serde(rename = "pattern-sources")]
    pub(crate) pattern_sources: Option<Spanned<Value>>,
    /// Taint sanitizers, retained untyped alongside `taint`.
    #[serde(rename = "pattern-sanitizers")]
    pub(crate) pattern_sanitizers: Option<Spanned<Value>>,
    /// Taint sinks, retained untyped alongside `taint`.
    #[serde(rename = "pattern-sinks")]
    pub(crate) pattern_sinks: Option<Spanned<Value>>,
}

/// One element of a legacy `patterns:` list, which may mix formula objects
/// with metavariable constraint entries.
#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub(crate) enum RawLegacyClause {
    /// A nested formula object contributing to the conjunction.
    Formula(RawLegacyFormulaObject),
    /// A metavariable constraint, normalized in a later pass.
    Constraint(Value),
}

/// Operand of a legacy unary operator such as `pattern-not`, which accepts
/// either a bare pattern string or a nested formula object.
#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub(crate) enum RawLegacyValue {
    /// A bare pattern written inline.
    String(String),
    /// A nested formula; boxed to keep the enum small despite recursion.
    Formula(Box<RawLegacyFormulaObject>),
}

/// Legacy formula mapping, in which each present key contributes one operand
/// and the operands are implicitly conjoined. Serde rejects unknown operator
/// fields; conversion checks that the mapping is semantically valid and
/// reports violations as diagnostic reports.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct RawLegacyFormulaObject {
    /// Positive syntactic pattern.
    pub(crate) pattern: Option<String>,
    /// Positive regular-expression match over the raw source text.
    #[serde(rename = "pattern-regex")]
    pub(crate) pattern_regex: Option<String>,
    /// Nested conjunction of clauses.
    pub(crate) patterns: Option<Vec<RawLegacyClause>>,
    /// Nested disjunction of alternatives.
    #[serde(rename = "pattern-either")]
    pub(crate) pattern_either: Option<Vec<Self>>,
    /// Negation: the match site must not also match this operand.
    #[serde(rename = "pattern-not")]
    pub(crate) pattern_not: Option<RawLegacyValue>,
    /// Context restriction: the match must occur inside this operand.
    #[serde(rename = "pattern-inside")]
    pub(crate) pattern_inside: Option<RawLegacyValue>,
    /// Context exclusion: the match must not occur inside this operand.
    #[serde(rename = "pattern-not-inside")]
    pub(crate) pattern_not_inside: Option<RawLegacyValue>,
    /// Negated regular-expression match over the raw source text.
    #[serde(rename = "pattern-not-regex")]
    pub(crate) pattern_not_regex: Option<String>,
    /// Unscoped search that may match anywhere in the file rather than at the
    /// current focus.
    #[serde(rename = "semgrep-internal-pattern-anywhere")]
    pub(crate) anywhere: Option<RawLegacyValue>,
}

/// Operand of the modern `match:` syntax: either a bare pattern string or a
/// mapping of operators.
#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub(crate) enum RawMatchFormula {
    /// A bare pattern written inline.
    String(String),
    /// An operator mapping; boxed to keep the enum small despite recursion.
    Object(Box<RawMatchFormulaObject>),
}

/// Operator mapping of the modern `match:` syntax. Serde rejects unknown
/// operator fields. Exactly one core operator key may be present; conversion
/// enforces that semantic constraint and reports violations as diagnostic
/// reports.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct RawMatchFormulaObject {
    /// Positive syntactic pattern.
    pub(crate) pattern: Option<String>,
    /// Positive regular-expression match over the raw source text.
    pub(crate) regex: Option<String>,
    /// Conjunction: every operand must hold.
    pub(crate) all: Option<Vec<RawMatchFormula>>,
    /// Disjunction: at least one operand must hold.
    pub(crate) any: Option<Vec<RawMatchFormula>>,
    /// Negation of a single operand.
    pub(crate) not: Option<Box<RawMatchFormula>>,
    /// Context restriction: the match must occur inside this operand.
    pub(crate) inside: Option<Box<RawMatchFormula>>,
    /// Unscoped search that may match anywhere in the file.
    pub(crate) anywhere: Option<Box<RawMatchFormula>>,
    /// Metavariable conditions on the core operator's matches, normalized
    /// separately.
    #[serde(rename = "where")]
    pub(crate) where_clauses: Option<Vec<Value>>,
    /// Name bound to the matched region so later clauses can refer to it.
    #[serde(rename = "as")]
    pub(crate) as_name: Option<String>,
    /// Autofix replacement template for the matched region.
    pub(crate) fix: Option<String>,
}

mod convert;

pub(crate) use convert::{
    convert_match_formula_object,
    parse_mode,
    parse_severity,
    push_optional_legacy_formula,
    push_optional_legacy_sequence_formula,
    schema_error,
    singleton_formula,
};
