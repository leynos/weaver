//! Schema-aligned rule models exposed by `sempai_yaml`.

use sempai_core::SourceSpan;
use serde_json::Value;

#[path = "project_depends_on.rs"]
pub mod project_depends_on;

pub use project_depends_on::ProjectDependsOnPayload;

/// A parsed Semgrep-compatible YAML rule file.
///
/// # Example
///
/// ```
/// use sempai_yaml::parse_rule_file;
///
/// let yaml = concat!(
///     "rules:\n",
///     "  - id: demo.rule\n",
///     "    message: detect foo\n",
///     "    languages: [python]\n",
///     "    severity: ERROR\n",
///     "    pattern: foo($X)\n",
/// );
///
/// let file = parse_rule_file(yaml, None).expect("valid rule file");
/// assert_eq!(file.rules().len(), 1);
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuleFile {
    /// Rules in the order they appear under the document's `rules:` key.
    pub(crate) rules: Vec<Rule>,
}

impl RuleFile {
    /// Creates a parsed rule file from the supplied rules.
    #[must_use]
    #[expect(
        clippy::missing_const_for_fn,
        reason = "Vec requires heap allocation and cannot be used in const contexts"
    )]
    pub fn new(rules: Vec<Rule>) -> Self { Self { rules } }

    /// Returns the parsed rules.
    #[must_use]
    pub fn rules(&self) -> &[Rule] { &self.rules }
}

/// A parsed Semgrep-compatible rule.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Rule {
    /// Stable identifier from the rule's `id:` key, used to key findings and
    /// suppressions; required, so it is unwrapped during validation.
    pub(crate) id: String,
    /// Analysis family selected by `mode:`, defaulting to search when the key
    /// is absent.
    pub(crate) mode: RuleMode,
    /// Coarse span of the whole rule mapping, absent when the source map could
    /// not locate the rule.
    pub(crate) span: Option<SourceSpan>,
    /// Span of the `mode:` value alone, so mode-specific diagnostics can point
    /// at the offending key rather than the whole rule.
    pub(crate) mode_span: Option<SourceSpan>,
    /// Finding text from `message:`; optional because the schema tolerates
    /// rules that only need to match.
    pub(crate) message: Option<String>,
    /// Languages from `languages:`, empty when the key is omitted.
    pub(crate) languages: Vec<String>,
    /// Decoded `severity:` token; `None` leaves the choice to the caller's
    /// default policy.
    pub(crate) severity: Option<RuleSeverity>,
    /// Oldest Semgrep release the rule declares support for, from
    /// `min-version:`.
    pub(crate) min_version: Option<String>,
    /// Newest Semgrep release the rule declares support for, from
    /// `max-version:`.
    pub(crate) max_version: Option<String>,
    /// The rule body, whose variant is fixed by [`Rule::mode`].
    pub(crate) principal: RulePrincipal,
}

impl Rule {
    /// Returns the stable rule identifier.
    #[must_use]
    pub fn id(&self) -> &str { &self.id }

    /// Returns the coarse span of the full rule object when known.
    #[must_use]
    pub const fn rule_span(&self) -> Option<&SourceSpan> { self.span.as_ref() }

    /// Returns the parsed rule mode.
    #[must_use]
    pub const fn mode(&self) -> &RuleMode { &self.mode }

    /// Returns the source span of the `mode` field when known.
    #[must_use]
    pub const fn mode_span(&self) -> Option<&SourceSpan> { self.mode_span.as_ref() }

    /// Returns the user-facing rule message when present.
    #[must_use]
    pub fn message(&self) -> Option<&str> { self.message.as_deref() }

    /// Returns the declared host languages.
    #[must_use]
    pub fn languages(&self) -> &[String] { &self.languages }

    /// Returns the declared severity when present.
    #[must_use]
    pub const fn severity(&self) -> Option<&RuleSeverity> { self.severity.as_ref() }

    /// Returns the minimum Semgrep version constraint when present.
    #[must_use]
    pub fn min_version(&self) -> Option<&str> { self.min_version.as_deref() }

    /// Returns the maximum Semgrep version constraint when present.
    #[must_use]
    pub fn max_version(&self) -> Option<&str> { self.max_version.as_deref() }

    /// Returns the parsed query principal.
    #[must_use]
    pub const fn principal(&self) -> &RulePrincipal { &self.principal }
}

/// Parsed Semgrep rule modes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RuleMode {
    /// Default search mode.
    Search,
    /// Taint mode.
    Taint,
    /// Join mode.
    Join,
    /// Extract mode.
    Extract,
    /// Forward-compatible mode string not yet modelled by Sempai.
    Other(String),
}

impl std::fmt::Display for RuleMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Search => write!(f, "Search"),
            Self::Taint => write!(f, "Taint"),
            Self::Join => write!(f, "Join"),
            Self::Extract => write!(f, "Extract"),
            Self::Other(mode) => write!(f, "{mode}"),
        }
    }
}

impl RuleMode {
    /// Parses a mode string into a rule mode.
    #[must_use]
    pub fn from_optional(raw: Option<&str>) -> Self {
        match raw {
            None | Some("search") => Self::Search,
            Some("taint") => Self::Taint,
            Some("join") => Self::Join,
            Some("extract") => Self::Extract,
            Some(other) => Self::Other(other.to_owned()),
        }
    }
}

/// Parsed Semgrep severities.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RuleSeverity {
    /// `ERROR`
    Error,
    /// `WARNING`
    Warning,
    /// `INFO`
    Info,
    /// `INVENTORY`
    Inventory,
    /// `EXPERIMENT`
    Experiment,
    /// `CRITICAL`
    Critical,
    /// `HIGH`
    High,
    /// `MEDIUM`
    Medium,
    /// `LOW`
    Low,
}

impl RuleSeverity {
    /// Parses a schema-aligned severity string.
    #[must_use]
    pub fn parse(raw: &str) -> Option<Self> {
        match raw {
            "ERROR" => Some(Self::Error),
            "WARNING" => Some(Self::Warning),
            "INFO" => Some(Self::Info),
            "INVENTORY" => Some(Self::Inventory),
            "EXPERIMENT" => Some(Self::Experiment),
            "CRITICAL" => Some(Self::Critical),
            "HIGH" => Some(Self::High),
            "MEDIUM" => Some(Self::Medium),
            "LOW" => Some(Self::Low),
            _ => None,
        }
    }
}

/// The primary query form associated with a rule.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RulePrincipal {
    /// Search mode rule using either legacy or v2 syntax.
    Search(SearchQueryPrincipal),
    /// Extract mode rule using a legacy search principal plus extraction data.
    Extract(ExtractQueryPrincipal),
    /// Taint mode rule preserved for later semantic handling.
    Taint(TaintQueryPrincipal),
    /// Join mode rule preserved for later semantic handling.
    Join(Value),
}

/// Search rule principal in either legacy or v2 syntax.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SearchQueryPrincipal {
    /// Legacy top-level query operators.
    Legacy(LegacyFormula),
    /// v2 `match` query syntax.
    Match(MatchFormula),
    /// Semgrep compatibility key preserved for later dependency semantics.
    ProjectDependsOn(ProjectDependsOnPayload),
}

/// Extract rule principal.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExtractQueryPrincipal {
    /// Language the extracted text is re-analysed as, from `dest-language:`.
    pub(crate) dest_language: String,
    /// Metavariable naming the capture whose text is extracted, from
    /// `extract:`.
    pub(crate) extract: String,
    /// Legacy formula that locates the code to extract from.
    pub(crate) query: LegacyFormula,
}

impl ExtractQueryPrincipal {
    /// Returns the extracted destination language.
    #[must_use]
    pub fn dest_language(&self) -> &str { &self.dest_language }

    /// Returns the extraction template.
    #[must_use]
    pub fn extract(&self) -> &str { &self.extract }

    /// Returns the legacy search principal used to locate the extraction input.
    #[must_use]
    pub const fn query(&self) -> &LegacyFormula { &self.query }
}

/// Taint rule principal retained for later normalization.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TaintQueryPrincipal {
    /// New-style top-level `taint` object.
    New(Value),
    /// Old-style top-level `pattern-sources` / `pattern-sinks` form.
    Legacy {
        /// Rule sources.
        sources: Value,
        /// Rule sanitizers, when present.
        sanitizers: Option<Value>,
        /// Rule sinks.
        sinks: Value,
    },
}

/// Parsed legacy Semgrep query syntax.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LegacyFormula {
    /// `pattern: "..."`
    Pattern(String),
    /// `pattern-regex: "..."`
    PatternRegex(String),
    /// `patterns: [...]`
    Patterns(Vec<LegacyClause>),
    /// `pattern-either: [...]`
    PatternEither(Vec<Self>),
    /// `pattern-not: ...`
    PatternNot(Box<LegacyValue>),
    /// `pattern-inside: ...`
    PatternInside(Box<LegacyValue>),
    /// `pattern-not-inside: ...`
    PatternNotInside(Box<LegacyValue>),
    /// `pattern-not-regex: "..."`
    PatternNotRegex(String),
    /// `semgrep-internal-pattern-anywhere: ...`
    Anywhere(Box<LegacyValue>),
}

/// Legacy nested formula value that may be a string or object.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LegacyValue {
    /// String shorthand.
    String(String),
    /// Object form.
    Formula(LegacyFormula),
}

/// An item inside a legacy `patterns` array.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LegacyClause {
    /// Nested legacy formula.
    Formula(LegacyFormula),
    /// Any schema-aligned constraint object not yet normalized by Sempai.
    Constraint(Value),
}

/// Parsed v2 `match` formula.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MatchFormula {
    /// String shorthand treated as `pattern`.
    Pattern(String),
    /// `pattern: "..."`
    PatternObject(String),
    /// `regex: "..."`
    Regex(String),
    /// `all: [...]`
    All(Vec<Self>),
    /// `any: [...]`
    Any(Vec<Self>),
    /// `not: ...`
    Not(Box<Self>),
    /// `inside: ...`
    Inside(Box<Self>),
    /// `anywhere: ...`
    Anywhere(Box<Self>),
    /// Query object with optional `where`, `as`, or `fix`.
    Decorated {
        /// The core formula branch.
        formula: Box<Self>,
        /// Raw `where` clauses preserved for later normalization.
        where_clauses: Vec<Value>,
        /// Optional alias name.
        as_name: Option<String>,
        /// Optional in-formula fix text.
        fix: Option<String>,
    },
}

impl MatchFormula {
    /// Reports whether any decoration key is present, and so whether a
    /// [`MatchFormula::Decorated`] wrapper is needed at all.
    #[inline]
    const fn has_decoration(
        where_clauses: &[Value],
        as_name: Option<&str>,
        fix: Option<&str>,
    ) -> bool {
        !where_clauses.is_empty() || as_name.is_some() || fix.is_some()
    }

    /// Wraps a core match formula with optional decoration.
    #[must_use]
    pub fn decorated(
        formula: Self,
        where_clauses: Vec<Value>,
        as_name: Option<String>,
        fix: Option<String>,
    ) -> Self {
        if Self::has_decoration(&where_clauses, as_name.as_deref(), fix.as_deref()) {
            Self::Decorated {
                formula: Box::new(formula),
                where_clauses,
                as_name,
                fix,
            }
        } else {
            formula
        }
    }
}
