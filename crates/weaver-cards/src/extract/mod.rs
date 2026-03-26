//! Tree-sitter-backed symbol card extraction.

mod attachments;
mod candidates;
mod fingerprint;
mod languages;
mod positions;
mod utils;

use std::path::{Path, PathBuf};
use std::sync::Arc;

use thiserror::Error;
use weaver_syntax::SupportedLanguage;

use crate::Provenance;
use crate::cache::{CardCache, CardCacheAddress, CardCacheKey, ParserRegistry};
use crate::timestamp::extraction_timestamp_now;
use crate::{
    AttachmentsInfo, DetailLevel, DocInfo, ImportInterstitialInfo, InterstitialInfo, MetricsInfo,
    NormalizedAttachments, SignatureInfo, StructureInfo, SymbolCard, SymbolIdentity, SymbolRef,
};
pub(super) use candidates::{EntityCandidate, InterstitialCandidate};
use candidates::{build_module_candidate, select_candidate};
use positions::{position_to_byte, usize_to_u32};
use utils::{file_uri, provenance_sources, to_card_language};

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
    /// The source path cannot be represented as a valid file URI.
    #[error("card extraction requires an absolute path: {path}")]
    InvalidPath {
        /// Invalid source path.
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
#[derive(Debug, Clone)]
pub struct TreeSitterCardExtractor {
    cache: Arc<CardCache>,
    parsers: Arc<ParserRegistry>,
}

impl TreeSitterCardExtractor {
    /// Creates a new extractor.
    #[must_use]
    pub fn new() -> Self {
        Self {
            cache: Arc::new(CardCache::default()),
            parsers: Arc::new(ParserRegistry::default()),
        }
    }

    /// Creates a new extractor with a custom cache capacity.
    #[must_use]
    pub fn with_cache_capacity(capacity: usize) -> Self {
        Self {
            cache: Arc::new(CardCache::new(capacity)),
            parsers: Arc::new(ParserRegistry::default()),
        }
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
        let cache_key = CardCacheKey::new(
            input.path,
            input.source,
            CardCacheAddress {
                language,
                detail: input.detail,
                line: input.line,
                column: input.column,
            },
        );
        if let Some(card) = self.cache.get(&cache_key) {
            return Ok(card);
        }

        let card = Self::extract_for_language(input, language, |supported_language| {
            self.parsers
                .parse(supported_language, input.source)
                .map_err(|error| CardExtractionError::Parse {
                    language: String::from(supported_language.as_str()),
                    message: error.to_string(),
                })
        })?;
        self.cache
            .invalidate_stale_revisions(input.path, cache_key.content_hash());
        self.cache.insert(cache_key, card.clone());
        Ok(card)
    }

    fn extract_for_language<F>(
        input: CardExtractionInput<'_>,
        language: SupportedLanguage,
        parser: F,
    ) -> Result<SymbolCard, CardExtractionError>
    where
        F: FnOnce(SupportedLanguage) -> Result<weaver_syntax::ParseResult, CardExtractionError>,
    {
        let parse = parser(language)?;
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

        build_card(
            selected,
            CardBuildContext {
                language,
                path: input.path,
                detail: input.detail,
                source: input.source,
            },
        )
    }

    #[cfg(test)]
    pub(crate) fn extract_with_parser_for_test<F>(
        input: CardExtractionInput<'_>,
        parser: F,
    ) -> Result<SymbolCard, CardExtractionError>
    where
        F: FnOnce(SupportedLanguage) -> Result<weaver_syntax::ParseResult, CardExtractionError>,
    {
        let language = SupportedLanguage::from_path(input.path).ok_or_else(|| {
            CardExtractionError::UnsupportedLanguage {
                path: input.path.to_path_buf(),
            }
        })?;
        Self::extract_for_language(input, language, parser)
    }

    /// Returns cache hit/miss counters for this extractor instance.
    #[must_use]
    pub fn cache_stats(&self) -> crate::CacheStats {
        self.cache.stats()
    }

    /// Returns the number of cached entries held by this extractor.
    #[must_use]
    pub fn cache_len(&self) -> usize {
        self.cache.len()
    }

    #[cfg(test)]
    pub(crate) fn invalidate_path(&self, path: &Path) {
        self.cache.invalidate(path);
    }

    #[cfg(test)]
    pub(crate) fn parser_identity(
        &self,
        language: SupportedLanguage,
    ) -> Result<usize, weaver_syntax::SyntaxError> {
        self.parsers.parser_identity(language)
    }
}

impl Default for TreeSitterCardExtractor {
    fn default() -> Self {
        Self::new()
    }
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
fn build_card(
    candidate: &EntityCandidate,
    context: CardBuildContext<'_>,
) -> Result<SymbolCard, CardExtractionError> {
    let symbol_id = fingerprint::symbol_id(candidate, context.language, context.path);
    let attachments = leading_attachments(candidate, context.source, context.language);
    let doc = build_doc(candidate, &attachments, context.detail);
    let attachment_info = build_attachment_info(&attachments, context.detail);
    let signature = build_signature(candidate, context.detail);
    let structure = build_structure(candidate, context.detail);
    let metrics = build_metrics(candidate, context.detail);
    let interstitial = build_interstitial(candidate);

    Ok(SymbolCard {
        card_version: 1,
        symbol: SymbolIdentity {
            symbol_id: symbol_id.clone(),
            symbol_ref: SymbolRef {
                uri: file_uri(context.path)?,
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
            extracted_at: extraction_timestamp_now(),
            sources: provenance_sources(context.detail),
        },
        etag: Some(symbol_id),
    })
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

fn summarise(text: &str) -> String {
    text.lines()
        .find_map(|line| {
            let trimmed = line.trim();
            (!trimmed.is_empty()).then(|| String::from(trimmed))
        })
        .unwrap_or_else(|| String::from(text.trim()))
}
