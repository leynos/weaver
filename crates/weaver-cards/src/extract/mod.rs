//! Tree-sitter-backed symbol card extraction.

mod attachments;
mod fingerprint;
mod languages;

use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use thiserror::Error;
use weaver_syntax::{Parser, SupportedLanguage};

use crate::{
    AttachmentsInfo, DetailLevel, DocInfo, ImportInterstitialInfo, InterstitialInfo, MetricsInfo,
    NormalizedAttachments, SignatureInfo, StructureInfo, SymbolCard, SymbolIdentity, SymbolRef,
};
use crate::{CardLanguage, CardSymbolKind, Provenance};

/// Deterministic placeholder timestamp used until revision-based caching lands.
const EXTRACTED_AT_PLACEHOLDER: &str = "1970-01-01T00:00:00Z";

/// Input required to extract a Tree-sitter-backed symbol card.
#[derive(Debug, Clone, Copy)]
pub struct CardExtractionInput<'a> {
    /// Path of the source file being analysed.
    pub path: &'a Path,
    /// Source text of the file.
    pub source: &'a str,
    /// One-based line position requested by the caller.
    pub line: u32,
    /// One-based column position requested by the caller.
    pub column: u32,
    /// Requested card detail level.
    pub detail: DetailLevel,
}

/// Failure modes for Tree-sitter card extraction.
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum CardExtractionError {
    /// The file extension does not map to a supported Tree-sitter language.
    #[error("unsupported language for path: {path}")]
    UnsupportedLanguage {
        /// Unsupported source path.
        path: PathBuf,
    },
    /// The requested position is outside the source text.
    #[error("requested position {line}:{column} is outside the source text")]
    PositionOutOfRange {
        /// One-based request line.
        line: u32,
        /// One-based request column.
        column: u32,
    },
    /// No eligible symbol or interstitial card covers the requested position.
    #[error("no symbol found at {line}:{column}")]
    NoSymbolAtPosition {
        /// One-based request line.
        line: u32,
        /// One-based request column.
        column: u32,
    },
    /// Tree-sitter failed to initialise or parse the source file.
    #[error("Tree-sitter parse failed for {language}: {message}")]
    Parse {
        /// Language name being parsed.
        language: String,
        /// Parser failure message.
        message: String,
    },
}

/// Tree-sitter-first extractor for `observe get-card`.
#[derive(Debug, Default, Clone, Copy)]
pub struct TreeSitterCardExtractor;

impl TreeSitterCardExtractor {
    /// Creates a new extractor.
    #[must_use]
    pub const fn new() -> Self {
        Self
    }

    /// Extracts a symbol card for the requested position.
    ///
    /// # Errors
    ///
    /// Returns [`CardExtractionError`] when the path is unsupported, the
    /// position is invalid, parsing fails, or no symbol matches the position.
    pub fn extract(
        &self,
        input: CardExtractionInput<'_>,
    ) -> Result<SymbolCard, CardExtractionError> {
        let language = SupportedLanguage::from_path(input.path).ok_or_else(|| {
            CardExtractionError::UnsupportedLanguage {
                path: input.path.to_path_buf(),
            }
        })?;
        let mut parser = Parser::new(language).map_err(|error| CardExtractionError::Parse {
            language: String::from(language.as_str()),
            message: error.to_string(),
        })?;
        let parse = parser
            .parse(input.source)
            .map_err(|error| CardExtractionError::Parse {
                language: String::from(language.as_str()),
                message: error.to_string(),
            })?;
        let position_byte = position_to_byte(input.source, input.line, input.column)?;
        let mut entities = languages::collect_entities(language, parse.root_node(), input.source);
        entities.sort_by_key(|candidate| candidate.byte_range.start);
        let interstitial =
            languages::collect_import_interstitial(language, parse.root_node(), input.source);
        let module_candidate = build_module_candidate(input.path, input.source, interstitial);

        let selected = select_candidate(&entities, module_candidate.as_ref(), position_byte)
            .ok_or(CardExtractionError::NoSymbolAtPosition {
                line: input.line,
                column: input.column,
            })?;

        Ok(build_card(
            selected,
            CardBuildContext {
                language,
                path: input.path,
                detail: input.detail,
                source: input.source,
            },
        ))
    }
}

#[derive(Debug, Clone)]
struct EntityCandidate {
    kind: CardSymbolKind,
    name: String,
    container: Option<String>,
    byte_range: std::ops::Range<usize>,
    range: crate::SourceRange,
    signature_display: Option<String>,
    params: Vec<crate::ParamInfo>,
    returns: String,
    locals: Vec<crate::LocalInfo>,
    branches: Vec<crate::BranchInfo>,
    decorators: Vec<String>,
    attachment_anchor: Option<usize>,
    docstring: Option<String>,
    lines: u32,
    structure_fingerprint: String,
    interstitial: Option<InterstitialCandidate>,
}

#[derive(Debug, Clone)]
struct InterstitialCandidate {
    byte_range: std::ops::Range<usize>,
    raw: String,
    normalized: Vec<String>,
    groups: Vec<Vec<String>>,
}

#[derive(Debug, Clone)]
struct LeadingAttachments {
    doc_comments: Vec<String>,
    decorators: Vec<String>,
}

impl LeadingAttachments {
    const fn is_empty(&self) -> bool {
        self.doc_comments.is_empty() && self.decorators.is_empty()
    }
}

#[derive(Debug, Clone)]
struct ImportBlock {
    byte_start: usize,
    byte_end: usize,
    normalized: Vec<String>,
}

#[derive(Clone, Copy)]
struct CardBuildContext<'a> {
    language: SupportedLanguage,
    path: &'a Path,
    detail: DetailLevel,
    source: &'a str,
}

fn build_module_candidate(
    path: &Path,
    source: &str,
    interstitial: Option<InterstitialCandidate>,
) -> Option<EntityCandidate> {
    if source.is_empty() {
        return None;
    }

    let line_count = usize_to_u32(source.lines().count());
    let end_column = source
        .lines()
        .last()
        .map_or(0, |line| usize_to_u32(line.len()));
    Some(EntityCandidate {
        kind: CardSymbolKind::Module,
        name: module_name(path),
        container: None,
        byte_range: 0..source.len(),
        range: crate::SourceRange {
            start: crate::SourcePosition { line: 0, column: 0 },
            end: crate::SourcePosition {
                line: line_count.saturating_sub(1),
                column: end_column,
            },
        },
        signature_display: None,
        params: Vec::new(),
        returns: String::new(),
        locals: Vec::new(),
        branches: Vec::new(),
        decorators: Vec::new(),
        attachment_anchor: Some(0),
        docstring: None,
        lines: line_count.max(1),
        structure_fingerprint: String::from("module"),
        interstitial,
    })
}

fn module_name(path: &Path) -> String {
    path.file_stem()
        .and_then(|stem| stem.to_str())
        .map(String::from)
        .or_else(|| {
            path.file_name()
                .and_then(|name| name.to_str())
                .map(String::from)
        })
        .unwrap_or_else(|| String::from("module"))
}

fn select_candidate<'a>(
    entities: &'a [EntityCandidate],
    module_candidate: Option<&'a EntityCandidate>,
    byte: usize,
) -> Option<&'a EntityCandidate> {
    let entity = entities
        .iter()
        .filter(|candidate| contains_byte(candidate, byte))
        .min_by_key(|candidate| {
            candidate
                .byte_range
                .end
                .saturating_sub(candidate.byte_range.start)
        });
    if entity.is_some() {
        return entity;
    }

    module_candidate.and_then(|candidate| {
        candidate
            .interstitial
            .as_ref()
            .filter(|interstitial| {
                byte >= interstitial.byte_range.start && byte <= interstitial.byte_range.end
            })
            .map(|_| candidate)
            .or_else(|| entities.is_empty().then_some(candidate))
    })
}

const fn contains_byte(candidate: &EntityCandidate, byte: usize) -> bool {
    byte >= candidate.byte_range.start && byte <= candidate.byte_range.end
}

fn build_card(candidate: &EntityCandidate, context: CardBuildContext<'_>) -> SymbolCard {
    let symbol_id = fingerprint::symbol_id(candidate, context.language, context.path);
    let attachments = leading_attachments(candidate, context.source, context.language);
    let doc = build_doc(candidate, &attachments, context.detail);
    let attachment_info = build_attachment_info(&attachments, context.detail);
    let signature = build_signature(candidate, context.detail);
    let structure = build_structure(candidate, context.detail);
    let metrics = build_metrics(candidate, context.detail);
    let interstitial = build_interstitial(candidate);

    SymbolCard {
        card_version: 1,
        symbol: SymbolIdentity {
            symbol_id: symbol_id.clone(),
            symbol_ref: SymbolRef {
                uri: file_uri(context.path),
                range: candidate.range.clone(),
                language: to_card_language(context.language),
                kind: candidate.kind,
                name: candidate.name.clone(),
                container: candidate.container.clone(),
            },
        },
        signature,
        doc,
        attachments: attachment_info,
        structure,
        lsp: None,
        metrics,
        deps: None,
        interstitial,
        provenance: Provenance {
            extracted_at: String::from(EXTRACTED_AT_PLACEHOLDER),
            sources: provenance_sources(context.detail),
        },
        etag: Some(symbol_id),
    }
}

fn leading_attachments(
    candidate: &EntityCandidate,
    source: &str,
    language: SupportedLanguage,
) -> LeadingAttachments {
    let decorators: Vec<attachments::Decorator> =
        candidate.decorators.iter().map(Into::into).collect();
    candidate.attachment_anchor.map_or_else(
        || LeadingAttachments {
            doc_comments: Vec::new(),
            decorators: candidate.decorators.clone(),
        },
        |anchor| attachments::collect_leading_attachments(source, language, anchor, &decorators),
    )
}

fn build_doc(
    candidate: &EntityCandidate,
    attachments: &LeadingAttachments,
    detail: DetailLevel,
) -> Option<DocInfo> {
    if detail < DetailLevel::Structure {
        return None;
    }

    let doc_text = candidate.docstring.clone().or_else(|| {
        (!attachments.doc_comments.is_empty()).then(|| attachments.doc_comments.join("\n"))
    })?;
    Some(DocInfo {
        docstring: doc_text.clone(),
        summary: summarise(&doc_text),
        source: String::from("tree_sitter"),
    })
}

fn build_attachment_info(
    attachments: &LeadingAttachments,
    detail: DetailLevel,
) -> Option<AttachmentsInfo> {
    if detail < DetailLevel::Structure {
        return None;
    }
    if attachments.is_empty() {
        return None;
    }

    Some(AttachmentsInfo {
        doc_comments: attachments.doc_comments.clone(),
        decorators: attachments.decorators.clone(),
        normalized: {
            let decorators: Vec<attachments::Decorator> =
                attachments.decorators.iter().map(Into::into).collect();
            NormalizedAttachments {
                decorators: attachments::normalised_decorators(&decorators),
            }
        },
        bundle_rule: String::from("leading_trivia"),
    })
}

fn build_signature(candidate: &EntityCandidate, detail: DetailLevel) -> Option<SignatureInfo> {
    if detail < DetailLevel::Signature {
        return None;
    }

    candidate
        .signature_display
        .as_ref()
        .map(|display| SignatureInfo {
            display: display.clone(),
            params: candidate.params.clone(),
            returns: candidate.returns.clone(),
        })
}

fn build_structure(candidate: &EntityCandidate, detail: DetailLevel) -> Option<StructureInfo> {
    (detail >= DetailLevel::Structure).then(|| StructureInfo {
        locals: candidate.locals.clone(),
        branches: candidate.branches.clone(),
    })
}

fn build_metrics(candidate: &EntityCandidate, detail: DetailLevel) -> Option<MetricsInfo> {
    (detail >= DetailLevel::Structure).then(|| MetricsInfo {
        lines: candidate.lines,
        cyclomatic: usize_to_u32(candidate.branches.len()).saturating_add(1),
        fan_in: None,
        fan_out: None,
    })
}

fn build_interstitial(candidate: &EntityCandidate) -> Option<InterstitialInfo> {
    candidate
        .interstitial
        .as_ref()
        .map(|block| InterstitialInfo {
            imports: ImportInterstitialInfo {
                raw: block.raw.clone(),
                normalized: block.normalized.clone(),
                groups: block.groups.clone(),
                source: String::from("tree_sitter"),
            },
        })
}

fn provenance_sources(detail: DetailLevel) -> Vec<String> {
    static TREE_SITTER_ONLY: OnceLock<Vec<String>> = OnceLock::new();
    let base = TREE_SITTER_ONLY.get_or_init(|| vec![String::from("tree_sitter")]);
    let mut sources = base.clone();
    if detail >= DetailLevel::Semantic {
        sources.push(String::from("tree_sitter_degraded_semantic"));
    }
    if detail >= DetailLevel::Full {
        sources.push(String::from("tree_sitter_degraded_full"));
    }
    sources
}

const fn to_card_language(language: SupportedLanguage) -> CardLanguage {
    match language {
        SupportedLanguage::Rust => CardLanguage::Rust,
        SupportedLanguage::Python => CardLanguage::Python,
        SupportedLanguage::TypeScript => CardLanguage::TypeScript,
    }
}

fn usize_to_u32(value: usize) -> u32 {
    u32::try_from(value).unwrap_or(u32::MAX)
}

fn file_uri(path: &Path) -> String {
    let display = path.to_string_lossy().replace('\\', "/");
    if display.starts_with('/') {
        format!("file://{display}")
    } else {
        format!("file:///{display}")
    }
}

fn summarise(text: &str) -> String {
    text.lines()
        .find_map(|line| {
            let trimmed = line.trim();
            (!trimmed.is_empty()).then(|| String::from(trimmed))
        })
        .unwrap_or_else(|| String::from(text.trim()))
}

fn position_to_byte(source: &str, line: u32, column: u32) -> Result<usize, CardExtractionError> {
    if line == 0 || column == 0 {
        return Err(CardExtractionError::PositionOutOfRange { line, column });
    }

    let Some(target_line) = source.lines().nth((line - 1) as usize) else {
        return Err(CardExtractionError::PositionOutOfRange { line, column });
    };
    if column as usize > target_line.chars().count().saturating_add(1) {
        return Err(CardExtractionError::PositionOutOfRange { line, column });
    }

    let mut byte_offset = 0usize;
    for current_line in source.lines().take((line - 1) as usize) {
        byte_offset = byte_offset
            .saturating_add(current_line.len())
            .saturating_add(1);
    }

    let column_offset = target_line
        .char_indices()
        .nth((column - 1) as usize)
        .map_or(target_line.len(), |(offset, _)| offset);
    Ok(byte_offset.saturating_add(column_offset))
}
