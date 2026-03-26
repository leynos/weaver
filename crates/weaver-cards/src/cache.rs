//! Shared cache and parser-pool infrastructure for card extraction.

use std::collections::HashMap;
use std::num::NonZeroUsize;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use lru::LruCache;
use sha2::{Digest, Sha256};
use weaver_syntax::{ParseResult, Parser, SupportedLanguage, SyntaxError};

use crate::{DetailLevel, SymbolCard};

/// Default maximum number of cached cards per extractor instance.
pub const DEFAULT_CACHE_CAPACITY: usize = 128;

/// Composite cache key for symbol-card lookups.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct CardCacheKey {
    path: PathBuf,
    content_hash: [u8; 32],
    language: SupportedLanguage,
    detail: DetailLevel,
    line: u32,
    column: u32,
}

/// Request-specific fields that participate in cache addressing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct CardCacheAddress {
    /// Detected source language.
    pub language: SupportedLanguage,
    /// Requested detail level.
    pub detail: DetailLevel,
    /// One-based request line.
    pub line: u32,
    /// One-based request column.
    pub column: u32,
}

impl CardCacheKey {
    /// Builds a cache key for the supplied extraction request.
    #[must_use]
    pub fn new(path: &Path, source: &str, address: CardCacheAddress) -> Self {
        Self {
            path: path.to_path_buf(),
            content_hash: content_hash(source),
            language: address.language,
            detail: address.detail,
            line: address.line,
            column: address.column,
        }
    }

    /// Returns the source path associated with the key.
    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Returns the content hash associated with the key.
    #[must_use]
    pub const fn content_hash(&self) -> &[u8; 32] {
        &self.content_hash
    }
}

#[derive(Debug, Clone)]
struct CachedCard {
    card: SymbolCard,
}

/// Point-in-time cache counters.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CacheStats {
    /// Number of successful cache lookups.
    pub hits: u64,
    /// Number of failed cache lookups.
    pub misses: u64,
}

/// LRU cache for extracted symbol cards.
pub struct CardCache {
    inner: Mutex<LruCache<CardCacheKey, CachedCard>>,
    hits: AtomicU64,
    misses: AtomicU64,
}

impl CardCache {
    /// Creates a cache with the given maximum entry count.
    #[must_use]
    pub fn new(capacity: usize) -> Self {
        Self {
            inner: Mutex::new(LruCache::new(non_zero_capacity(capacity))),
            hits: AtomicU64::new(0),
            misses: AtomicU64::new(0),
        }
    }

    /// Returns a cached card if present.
    #[must_use]
    pub fn get(&self, key: &CardCacheKey) -> Option<SymbolCard> {
        let mut guard = self.inner.lock().ok()?;
        let card = guard.get(key).cloned().map(|entry| entry.card);
        if card.is_some() {
            self.hits.fetch_add(1, Ordering::Relaxed);
        } else {
            self.misses.fetch_add(1, Ordering::Relaxed);
        }
        card
    }

    /// Inserts a card into the cache.
    pub fn insert(&self, key: CardCacheKey, card: SymbolCard) {
        if let Ok(mut guard) = self.inner.lock() {
            guard.put(key, CachedCard { card });
        }
    }

    /// Invalidates all entries associated with the given path.
    pub fn invalidate(&self, path: &Path) {
        self.evict_matching(|key| key.path() == path);
    }

    /// Invalidates cached entries for older revisions of the same path.
    pub fn invalidate_stale_revisions(&self, path: &Path, current_hash: &[u8; 32]) {
        self.evict_matching(|key| key.path() == path && key.content_hash() != current_hash);
    }

    fn evict_matching<F>(&self, predicate: F)
    where
        F: Fn(&CardCacheKey) -> bool,
    {
        if let Ok(mut guard) = self.inner.lock() {
            let stale_keys: Vec<CardCacheKey> = guard
                .iter()
                .filter(|(key, _)| predicate(key))
                .map(|(key, _)| key.clone())
                .collect();
            for key in stale_keys {
                let _ = guard.pop(&key);
            }
        }
    }

    /// Returns the number of cached entries.
    #[must_use]
    pub fn len(&self) -> usize {
        self.inner.lock().map_or(0, |guard| guard.len())
    }

    /// Returns `true` when the cache holds no entries.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Returns the current hit/miss counters.
    #[must_use]
    pub fn stats(&self) -> CacheStats {
        CacheStats {
            hits: self.hits.load(Ordering::Relaxed),
            misses: self.misses.load(Ordering::Relaxed),
        }
    }
}

impl Default for CardCache {
    fn default() -> Self {
        Self::new(DEFAULT_CACHE_CAPACITY)
    }
}

impl std::fmt::Debug for CardCache {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("CardCache")
            .field("len", &self.len())
            .field("stats", &self.stats())
            .finish()
    }
}

/// Pool of reusable Tree-sitter parsers keyed by language.
pub struct ParserRegistry {
    parsers: Mutex<HashMap<SupportedLanguage, Arc<Mutex<Parser>>>>,
}

impl ParserRegistry {
    /// Creates an empty parser registry.
    #[must_use]
    pub fn new() -> Self {
        Self {
            parsers: Mutex::new(HashMap::new()),
        }
    }

    /// Parses the source using a cached parser for the given language.
    ///
    /// # Errors
    ///
    /// Returns a parser initialisation or parse error, or an internal error if
    /// one of the parser mutexes has been poisoned.
    pub fn parse(
        &self,
        language: SupportedLanguage,
        source: &str,
    ) -> Result<ParseResult, SyntaxError> {
        let parser = self.parser(language)?;
        let mut guard = parser
            .lock()
            .map_err(|_| SyntaxError::internal_error("card parser lock poisoned"))?;
        guard.parse(source)
    }

    fn parser(&self, language: SupportedLanguage) -> Result<Arc<Mutex<Parser>>, SyntaxError> {
        let mut guard = self
            .parsers
            .lock()
            .map_err(|_| SyntaxError::internal_error("card parser registry lock poisoned"))?;
        if let Some(parser) = guard.get(&language) {
            return Ok(Arc::clone(parser));
        }

        let parser = Arc::new(Mutex::new(Parser::new(language)?));
        guard.insert(language, Arc::clone(&parser));
        Ok(parser)
    }

    #[cfg(test)]
    pub(crate) fn parser_identity(
        &self,
        language: SupportedLanguage,
    ) -> Result<usize, SyntaxError> {
        let parser = self.parser(language)?;
        Ok(Arc::as_ptr(&parser) as usize)
    }
}

impl Default for ParserRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Debug for ParserRegistry {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let languages = self
            .parsers
            .lock()
            .map(|guard| guard.keys().copied().collect::<Vec<_>>())
            .unwrap_or_default();
        formatter
            .debug_struct("ParserRegistry")
            .field("languages", &languages)
            .finish()
    }
}

/// Computes the SHA-256 content hash used for cache keying.
#[must_use]
pub fn content_hash(source: &str) -> [u8; 32] {
    Sha256::digest(source.as_bytes()).into()
}

fn non_zero_capacity(capacity: usize) -> NonZeroUsize {
    NonZeroUsize::new(capacity).unwrap_or(NonZeroUsize::MIN)
}
