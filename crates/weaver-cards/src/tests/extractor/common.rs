//! Shared fixtures and helpers for extractor-focused tests.

use std::path::Path;

use crate::{
    CardExtractionError, CardExtractionInput, CardSymbolKind, DetailLevel, TreeSitterCardExtractor,
};

#[derive(Clone, Copy)]
pub(super) struct ExtractRequest<'a> {
    pub(super) path: &'a Path,
    pub(super) source: &'a str,
    pub(super) line: u32,
    pub(super) column: u32,
    pub(super) detail: DetailLevel,
}

#[derive(Clone, Copy)]
pub(super) struct SymbolExpectation<'a> {
    pub(super) request: ExtractRequest<'a>,
    pub(super) expected_kind: CardSymbolKind,
    pub(super) expected_name: &'a str,
    pub(super) expected_container: Option<&'a str>,
}

#[derive(Clone, Copy)]
pub(super) struct CaseSpec {
    pub(super) path: &'static Path,
    pub(super) source: &'static str,
    pub(super) line: u32,
    pub(super) column: u32,
    pub(super) kind: CardSymbolKind,
    pub(super) name: &'static str,
    pub(super) container: Option<&'static str>,
}

impl From<CaseSpec> for SymbolExpectation<'static> {
    fn from(s: CaseSpec) -> Self {
        SymbolExpectation {
            request: ExtractRequest {
                path: s.path,
                source: s.source,
                line: s.line,
                column: s.column,
                detail: DetailLevel::Structure,
            },
            expected_kind: s.kind,
            expected_name: s.name,
            expected_container: s.container,
        }
    }
}

impl<'a> From<ExtractRequest<'a>> for CardExtractionInput<'a> {
    fn from(r: ExtractRequest<'a>) -> Self {
        CardExtractionInput {
            path: r.path,
            source: r.source,
            line: r.line,
            column: r.column,
            detail: r.detail,
        }
    }
}

pub(super) fn run_extraction(
    request: ExtractRequest<'_>,
) -> Result<crate::SymbolCard, CardExtractionError> {
    let path = super::super::absolute_test_path(request.path);
    TreeSitterCardExtractor::new().extract(
        ExtractRequest {
            path: &path,
            ..request
        }
        .into(),
    )
}

pub(super) fn extract(request: ExtractRequest<'_>) -> crate::SymbolCard {
    run_extraction(request).expect("card extraction should succeed")
}

pub(super) fn extract_error(request: ExtractRequest<'_>) -> CardExtractionError {
    run_extraction(request).expect_err("card extraction should fail")
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ExpectedError {
    UnsupportedLanguage,
    PositionOutOfRange,
    NoSymbolAtPosition,
}

pub(super) fn error_matches(err: &CardExtractionError, expected: ExpectedError) -> bool {
    matches!(
        (err, expected),
        (
            CardExtractionError::UnsupportedLanguage { .. },
            ExpectedError::UnsupportedLanguage
        ) | (
            CardExtractionError::PositionOutOfRange { .. },
            ExpectedError::PositionOutOfRange
        ) | (
            CardExtractionError::NoSymbolAtPosition { .. },
            ExpectedError::NoSymbolAtPosition
        )
    )
}
