//! Stateful extractor wrapper for cache-aware Tree-sitter card extraction.

use std::sync::Arc;

use weaver_syntax::SupportedLanguage;

use super::{CardExtractionError, CardExtractionInput, extract_for_language};
use crate::cache::{CardCache, CardCacheAddress, CardCacheKey, ParserRegistry};

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
        Self::with_shared_resources(
            Arc::new(CardCache::default()),
            Arc::new(ParserRegistry::default()),
        )
    }

    /// Creates a new extractor with a custom cache capacity.
    ///
    /// # Panics
    ///
    /// Panics if `capacity` is zero.
    #[must_use]
    pub fn with_cache_capacity(capacity: usize) -> Self {
        Self::with_shared_resources(
            Arc::new(CardCache::new(capacity)),
            Arc::new(ParserRegistry::default()),
        )
    }

    /// Creates a new extractor backed by caller-supplied shared resources.
    #[must_use]
    pub const fn with_shared_resources(
        cache: Arc<CardCache>,
        parsers: Arc<ParserRegistry>,
    ) -> Self {
        Self { cache, parsers }
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
    ) -> Result<crate::SymbolCard, CardExtractionError> {
        self.extract_shared(input).map(|card| card.as_ref().clone())
    }

    /// Extracts a symbol card for the requested position, reusing a shared
    /// payload when the cache already contains the card.
    ///
    /// # Errors
    ///
    /// Returns [`CardExtractionError`] when the path is unsupported, the
    /// position is invalid, parsing fails, or no symbol matches the position.
    pub fn extract_shared(
        &self,
        input: CardExtractionInput<'_>,
    ) -> Result<Arc<crate::SymbolCard>, CardExtractionError> {
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

        let _population_guard = self.cache.lock_population(&cache_key);
        if let Some(card) = self.cache.get_shared(&cache_key) {
            return Ok(card);
        }

        let card = Arc::new(extract_for_language(
            input,
            language,
            |supported_language| {
                self.parsers
                    .parse(supported_language, input.source)
                    .map_err(|error| CardExtractionError::Parse {
                        language: String::from(supported_language.as_str()),
                        message: error.to_string(),
                    })
            },
        )?);
        self.cache
            .invalidate_stale_revisions(input.path, cache_key.content_hash());
        self.cache.insert_shared(cache_key, Arc::clone(&card));
        Ok(card)
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
    pub(crate) fn invalidate_path(&self, path: &std::path::Path) {
        self.cache.invalidate(path);
    }

    #[cfg(test)]
    pub(crate) fn parser_identity(
        &self,
        language: SupportedLanguage,
    ) -> Result<usize, weaver_syntax::SyntaxError> {
        self.parsers.parser_identity(language)
    }

    #[cfg(test)]
    pub(crate) fn extract_with_parser_for_test<F>(
        input: CardExtractionInput<'_>,
        parser: F,
    ) -> Result<crate::SymbolCard, CardExtractionError>
    where
        F: FnOnce(SupportedLanguage) -> Result<weaver_syntax::ParseResult, CardExtractionError>,
    {
        super::extract_with_parser_for_test(input, parser)
    }
}

impl Default for TreeSitterCardExtractor {
    fn default() -> Self {
        Self::new()
    }
}
