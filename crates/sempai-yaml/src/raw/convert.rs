//! Conversion of the raw YAML shapes in [`super`] into the validated `model`
//! types.
//!
//! Every conversion is fallible: the raw layer accepts any syntactically valid
//! document, and it is here that semantic constraints — such as a formula
//! object naming exactly one operator — are enforced and reported as
//! `DiagnosticReport`s.
use sempai_core::{DiagnosticCode, DiagnosticReport, SourceSpan};
use serde_saphyr::Spanned;

use super::{
    RawLegacyClause,
    RawLegacyFormulaObject,
    RawLegacyValue,
    RawMatchFormula,
    RawMatchFormulaObject,
};
use crate::model::{
    LegacyClause,
    LegacyFormula,
    LegacyValue,
    MatchFormula,
    RuleMode,
    RuleSeverity,
};

/// Builds a schema validation diagnostic with a fixed code and a single
/// remediation note, so all schema failures read consistently.
pub(crate) fn schema_error(
    message: String,
    span: Option<SourceSpan>,
    note: &str,
) -> DiagnosticReport {
    DiagnosticReport::validation_error(
        DiagnosticCode::ESempaiSchemaInvalid,
        message,
        span,
        vec![note.to_owned()],
    )
}

/// Reduces a set of collected legacy operands to the single formula a legacy
/// object is allowed to define.
///
/// # Errors
///
/// Returns the diagnostic produced by `make_error` when the object defined no
/// operator or more than one; the operand count is passed through so the caller
/// can word the two cases differently.
pub(crate) fn singleton_formula(
    mut formulas: Vec<LegacyFormula>,
    make_error: impl FnOnce(usize) -> DiagnosticReport,
) -> Result<LegacyFormula, DiagnosticReport> {
    match formulas.len() {
        1 => Ok(formulas.remove(0)),
        len => Err(make_error(len)),
    }
}

impl TryFrom<RawLegacyClause> for LegacyClause {
    type Error = DiagnosticReport;

    fn try_from(value: RawLegacyClause) -> Result<Self, Self::Error> {
        match value {
            RawLegacyClause::Formula(formula) => Ok(Self::Formula(formula.try_into()?)),
            RawLegacyClause::Constraint(constraint) => Ok(Self::Constraint(constraint)),
        }
    }
}

impl TryFrom<Box<RawLegacyFormulaObject>> for LegacyFormula {
    type Error = DiagnosticReport;

    fn try_from(value: Box<RawLegacyFormulaObject>) -> Result<Self, Self::Error> {
        (*value).try_into()
    }
}

impl TryFrom<RawLegacyFormulaObject> for LegacyFormula {
    type Error = DiagnosticReport;

    fn try_from(value: RawLegacyFormulaObject) -> Result<Self, Self::Error> {
        convert_legacy_formula_object(value, None)
    }
}

/// Converts a legacy formula mapping into the single [`LegacyFormula`] it
/// denotes, recursing through nested clauses. `span` locates the mapping for
/// diagnostics and may be `None` when no source position is known.
///
/// # Errors
///
/// Returns a schema diagnostic when the mapping names no supported operator or
/// names more than one, and propagates any failure from converting a nested
/// clause.
pub(crate) fn convert_legacy_formula_object(
    value: RawLegacyFormulaObject,
    span: Option<SourceSpan>,
) -> Result<LegacyFormula, DiagnosticReport> {
    let mut formulas = Vec::new();
    push_optional_legacy_formula(&mut formulas, value.pattern, LegacyFormula::Pattern);
    push_optional_legacy_formula(
        &mut formulas,
        value.pattern_regex,
        LegacyFormula::PatternRegex,
    );
    push_optional_legacy_sequence_formula(&mut formulas, value.patterns, LegacyFormula::Patterns)?;
    push_optional_legacy_sequence_formula(
        &mut formulas,
        value.pattern_either,
        LegacyFormula::PatternEither,
    )?;
    push_optional_legacy_value_formula(
        &mut formulas,
        value.pattern_not,
        LegacyFormula::PatternNot,
    )?;
    push_optional_legacy_value_formula(
        &mut formulas,
        value.pattern_inside,
        LegacyFormula::PatternInside,
    )?;
    push_optional_legacy_value_formula(
        &mut formulas,
        value.pattern_not_inside,
        LegacyFormula::PatternNotInside,
    )?;
    push_optional_legacy_formula(
        &mut formulas,
        value.pattern_not_regex,
        LegacyFormula::PatternNotRegex,
    );
    push_optional_legacy_value_formula(&mut formulas, value.anywhere, LegacyFormula::Anywhere)?;

    singleton_formula(formulas, |len| match len {
        0 => schema_error(
            String::from("legacy formula object is empty"),
            span.clone(),
            "add a supported legacy operator",
        ),
        _ => schema_error(
            String::from("legacy formula object defines multiple operators"),
            span,
            "keep only one operator per legacy object",
        ),
    })
}

/// Appends an optional string-backed legacy operator to `formulas`.
pub(crate) fn push_optional_legacy_formula(
    formulas: &mut Vec<LegacyFormula>,
    value: Option<String>,
    constructor: fn(String) -> LegacyFormula,
) {
    if let Some(text) = value {
        formulas.push(constructor(text));
    }
}

/// Appends an optional sequence-backed legacy operator to `formulas`, converting
/// each element on the way; absent operands leave `formulas` untouched.
///
/// # Errors
///
/// Propagates the first element conversion failure, in which case nothing is
/// appended.
pub(crate) fn push_optional_legacy_sequence_formula<T, U>(
    formulas: &mut Vec<LegacyFormula>,
    value: Option<Vec<T>>,
    constructor: fn(Vec<U>) -> LegacyFormula,
) -> Result<(), DiagnosticReport>
where
    T: TryInto<U, Error = DiagnosticReport>,
{
    if let Some(items) = value {
        formulas.push(constructor(
            items
                .into_iter()
                .map(TryInto::try_into)
                .collect::<Result<Vec<_>, _>>()?,
        ));
    }
    Ok(())
}

/// Converts an optional unary operand and appends the resulting formula,
/// leaving `formulas` untouched when the operand is absent.
fn push_optional_legacy_value_formula(
    formulas: &mut Vec<LegacyFormula>,
    value: Option<RawLegacyValue>,
    constructor: fn(Box<LegacyValue>) -> LegacyFormula,
) -> Result<(), DiagnosticReport> {
    if let Some(inner) = value {
        formulas.push(constructor(Box::new(inner.try_into()?)));
    }
    Ok(())
}

impl TryFrom<RawLegacyValue> for LegacyValue {
    type Error = DiagnosticReport;

    fn try_from(value: RawLegacyValue) -> Result<Self, Self::Error> {
        match value {
            RawLegacyValue::String(text) => Ok(Self::String(text)),
            RawLegacyValue::Formula(formula) => Ok(Self::Formula(formula.try_into()?)),
        }
    }
}

impl TryFrom<RawMatchFormula> for MatchFormula {
    type Error = DiagnosticReport;

    fn try_from(value: RawMatchFormula) -> Result<Self, Self::Error> {
        match value {
            RawMatchFormula::String(pattern) => Ok(Self::Pattern(pattern)),
            RawMatchFormula::Object(object) => object.try_into(),
        }
    }
}

impl TryFrom<Box<RawMatchFormulaObject>> for MatchFormula {
    type Error = DiagnosticReport;

    fn try_from(value: Box<RawMatchFormulaObject>) -> Result<Self, Self::Error> {
        (*value).try_into()
    }
}

/// Converts the operator fields of a `RawMatchFormulaObject` into the core
/// (undecorated) `MatchFormula` variant.  The caller is responsible for
/// ensuring exactly one operator field is `Some` before calling this.
fn build_core_match_formula(
    value: RawMatchFormulaObject,
) -> Result<MatchFormula, DiagnosticReport> {
    if let Some(pattern) = value.pattern {
        Ok(MatchFormula::PatternObject(pattern))
    } else if let Some(regex) = value.regex {
        Ok(MatchFormula::Regex(regex))
    } else if let Some(all) = value.all {
        Ok(MatchFormula::All(
            all.into_iter()
                .map(TryInto::try_into)
                .collect::<Result<Vec<_>, _>>()?,
        ))
    } else if let Some(any) = value.any {
        Ok(MatchFormula::Any(
            any.into_iter()
                .map(TryInto::try_into)
                .collect::<Result<Vec<_>, _>>()?,
        ))
    } else if let Some(not) = value.not {
        Ok(MatchFormula::Not(Box::new((*not).try_into()?)))
    } else if let Some(inside) = value.inside {
        Ok(MatchFormula::Inside(Box::new((*inside).try_into()?)))
    } else if let Some(anywhere) = value.anywhere {
        Ok(MatchFormula::Anywhere(Box::new((*anywhere).try_into()?)))
    } else {
        // Safety: caller guarantees operator_count == 1
        Err(schema_error(
            String::from("internal error: no operator found despite count check"),
            None,
            "please report this bug",
        ))
    }
}

impl TryFrom<RawMatchFormulaObject> for MatchFormula {
    type Error = DiagnosticReport;

    fn try_from(value: RawMatchFormulaObject) -> Result<Self, Self::Error> {
        convert_match_formula_object(value, None)
    }
}

/// Converts a raw `match` mapping into a validated [`MatchFormula`], wrapping
/// the core operator in a decoration layer when `where`, `as` or `fix` is also
/// present.
///
/// # Errors
///
/// Returns a schema diagnostic, blaming `span`, when the mapping names no core
/// operator or names more than one, and propagates failures from converting
/// nested operands.
pub(crate) fn convert_match_formula_object(
    value: RawMatchFormulaObject,
    span: Option<SourceSpan>,
) -> Result<MatchFormula, DiagnosticReport> {
    let operator_count = [
        value.pattern.is_some(),
        value.regex.is_some(),
        value.all.is_some(),
        value.any.is_some(),
        value.not.is_some(),
        value.inside.is_some(),
        value.anywhere.is_some(),
    ]
    .iter()
    .filter(|&&present| present)
    .count();

    if operator_count == 0 {
        return Err(schema_error(
            String::from("match formula object is empty"),
            span,
            "add a supported `match` operator",
        ));
    }

    if operator_count > 1 {
        return Err(schema_error(
            String::from("match formula object defines multiple operators"),
            span,
            "keep only one operator per match object",
        ));
    }

    let where_ = value.where_clauses.clone().unwrap_or_default();
    let as_name = value.as_name.clone();
    let fix = value.fix.clone();
    let core = build_core_match_formula(value)?;
    Ok(MatchFormula::decorated(core, where_, as_name, fix))
}

/// Decodes a `severity:` token into the typed severity.
///
/// # Errors
///
/// Returns a schema diagnostic listing every accepted token when the value is
/// not one of them, blaming `fallback_span` because the spanned value's own
/// location is not always available.
pub(crate) fn parse_severity(
    value: &Spanned<String>,
    fallback_span: Option<&SourceSpan>,
) -> Result<RuleSeverity, DiagnosticReport> {
    RuleSeverity::parse(&value.value).ok_or_else(|| {
        schema_error(
            format!("unsupported severity `{}`", value.value),
            fallback_span.cloned(),
            concat!(
                "use one of ERROR, WARNING, INFO, ",
                "INVENTORY, EXPERIMENT, CRITICAL, ",
                "HIGH, MEDIUM, or LOW"
            ),
        )
    })
}

/// Parses an optional raw rule mode string into the corresponding [`RuleMode`].
pub(crate) fn parse_mode(value: Option<&str>) -> RuleMode { RuleMode::from_optional(value) }
