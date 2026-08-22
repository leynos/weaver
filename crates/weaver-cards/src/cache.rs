//! Shared cache and parser-pool infrastructure for card extraction.

use std::{
    collections::{HashMap, HashSet},
    num::NonZeroUsize,
    path::{Path, PathBuf},
    sync::{
        Arc,
        Condvar,
        Mutex,
        MutexGuard,
        atomic::{AtomicU64, Ordering},
    },
};

use lru::LruCache;
use sha2::{Digest, Sha256};
use weaver_syntax::{ParseResult, Parser, SupportedLanguage, SyntaxError};

use crate::{DetailLevel, SymbolCard};

/// Default maximum number of cached cards per extractor instance.
pub const DEFAULT_CACHE_CAPACITY: usize = 128;

/// Composite cache key for symbol-card lookups.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct CardCacheKey {
    /// Source file the card was extracted from.
    path: PathBuf,
    /// SHA-256 digest of the source text, so a stale revision misses the cache.
    content_hash: [u8; 32],
    /// Detected source language, since the same span parses differently per grammar.
    language: SupportedLanguage,
    /// Requested detail level, since a summary and a full card are distinct entries.
    detail: DetailLevel,
    /// One-based request line.
    line: u32,
    /// One-based request column.
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
    pub fn path(&self) -> &Path { &self.path }

    /// Returns the content hash associated with the key.
    #[must_use]
    pub const fn content_hash(&self) -> &[u8; 32] { &self.content_hash }
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
    /// Bounded store of extracted cards, evicted least-recently-used.
    inner: Mutex<LruCache<CardCacheKey, Arc<SymbolCard>>>,
    /// Keys currently being populated by another thread, so concurrent misses
    /// on the same key wait rather than duplicate the extraction work.
    in_flight: Mutex<HashSet<CardCacheKey>>,
    /// Signalled when a key leaves `in_flight`, waking threads blocked on it.
    in_flight_ready: Condvar,
    /// Running count of successful lookups.
    hits: AtomicU64,
    /// Running count of failed lookups.
    misses: AtomicU64,
}

impl CardCache {
    /// Creates a cache with the given maximum entry count.
    ///
    /// # Panics
    ///
    /// Panics if `capacity` is zero.
    #[must_use]
    pub fn new(capacity: usize) -> Self {
        Self {
            inner: Mutex::new(LruCache::new(non_zero_capacity(capacity))),
            in_flight: Mutex::new(HashSet::new()),
            in_flight_ready: Condvar::new(),
            hits: AtomicU64::new(0),
            misses: AtomicU64::new(0),
        }
    }

    /// Returns a cached card if present.
    #[must_use]
    pub fn get(&self, key: &CardCacheKey) -> Option<SymbolCard> {
        self.get_shared(key).as_deref().cloned()
    }

    /// Returns a cached card if present, keeping the stored payload shared.
    #[must_use]
    pub fn get_shared(&self, key: &CardCacheKey) -> Option<Arc<SymbolCard>> {
        let Ok(mut guard) = self.inner.lock() else {
            self.record_miss();
            return None;
        };
        let card = guard.get(key).cloned();
        if card.is_some() {
            self.record_hit();
        } else {
            self.record_miss();
        }
        card
    }

    /// Inserts a card into the cache.
    pub fn insert(&self, key: CardCacheKey, card: SymbolCard) {
        self.insert_shared(key, Arc::new(card));
    }

    /// Inserts a shared card into the cache.
    pub fn insert_shared(&self, key: CardCacheKey, card: Arc<SymbolCard>) {
        if let Ok(mut guard) = self.inner.lock() {
            guard.put(key, card);
        }
    }

    /// Acquires an in-flight population lock for a single cache key.
    ///
    /// Only one thread may hold the population lock for a given key at a time.
    #[must_use]
    pub(crate) fn lock_population(&self, key: &CardCacheKey) -> CachePopulationGuard<'_> {
        let mut guard = recover_guard(self.in_flight.lock());
        while guard.contains(key) {
            guard = recover_guard(self.in_flight_ready.wait(guard));
        }
        guard.insert(key.clone());
        drop(guard);
        CachePopulationGuard {
            cache: self,
            key: key.clone(),
        }
    }

    /// Invalidates all entries associated with the given path.
    ///
    /// Path matching is based on the exact `PathBuf` stored in the cache key.
    /// Callers that need symlink or relative-path canonicalisation must do so
    /// before building cache keys.
    pub fn invalidate(&self, path: &Path) { self.evict_matching(|key| key.path() == path); }

    /// Invalidates cached entries for older revisions of the same path.
    pub fn invalidate_stale_revisions(&self, path: &Path, current_hash: &[u8; 32]) {
        self.evict_matching(|key| key.path() == path && key.content_hash() != current_hash);
    }

    /// Returns a cached card without updating hit/miss counters.
    #[must_use]
    pub(crate) fn peek_shared(&self, key: &CardCacheKey) -> Option<Arc<SymbolCard>> {
        self.inner
            .lock()
            .ok()
            .and_then(|mut guard| guard.get(key).cloned())
    }

    /// Records a successful lookup for the `stats()` counters.
    pub(crate) fn record_hit(&self) { self.hits.fetch_add(1, Ordering::Relaxed); }

    /// Records a failed lookup for the `stats()` counters.
    pub(crate) fn record_miss(&self) { self.misses.fetch_add(1, Ordering::Relaxed); }

    /// Removes every entry whose key satisfies `predicate`.
    ///
    /// Backs [`Self::invalidate`] and [`Self::invalidate_stale_revisions`].
    fn evict_matching<F>(&self, predicate: F)
    where
        F: Fn(&CardCacheKey) -> bool,
    {
        if let Ok(mut guard) = self.inner.lock() {
            // Collect keys first because `lru::LruCache` cannot be mutated
            // while it is being iterated.
            let stale_keys: Vec<CardCacheKey> = guard
                .iter()
                .filter(|(key, _)| predicate(key))
                .map(|(key, _)| key.clone())
                .collect();
            for key in stale_keys {
                drop(guard.pop(&key));
            }
        }
    }

    /// Returns the number of cached entries.
    #[must_use]
    pub fn len(&self) -> usize { self.inner.lock().map_or(0, |guard| guard.len()) }

    /// Returns `true` when the cache holds no entries.
    #[must_use]
    pub fn is_empty(&self) -> bool { self.len() == 0 }

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
    fn default() -> Self { Self::new(DEFAULT_CACHE_CAPACITY) }
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

/// Guard that serializes cache population for a single [`CardCacheKey`].
pub(crate) struct CachePopulationGuard<'a> {
    /// Cache the guard releases the in-flight lock on when dropped.
    cache: &'a CardCache,
    /// Key held for the duration of the population lock.
    key: CardCacheKey,
}

impl Drop for CachePopulationGuard<'_> {
    fn drop(&mut self) {
        let mut guard = recover_guard(self.cache.in_flight.lock());
        guard.remove(&self.key);
        self.cache.in_flight_ready.notify_all();
    }
}

/// Pool of reusable Tree-sitter parsers keyed by language.
pub struct ParserRegistry {
    /// One shared, mutex-guarded parser per language, created lazily on first use.
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

    /// Returns the shared parser for `language`, constructing and caching one
    /// on first request.
    ///
    /// # Errors
    ///
    /// Returns an internal error if the registry mutex has been poisoned, or a
    /// parser initialisation error if a new parser cannot be built.
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
    fn default() -> Self { Self::new() }
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
pub fn content_hash(source: &str) -> [u8; 32] { Sha256::digest(source.as_bytes()).into() }

/// Converts a capacity to `NonZeroUsize` for [`LruCache::new`].
///
/// # Panics
///
/// Panics if `capacity` is zero.
fn non_zero_capacity(capacity: usize) -> NonZeroUsize {
    assert!(
        capacity > 0,
        "card cache capacity must be greater than zero"
    );
    let Some(non_zero) = NonZeroUsize::new(capacity) else {
        panic!("card cache capacity must be greater than zero");
    };
    non_zero
}

/// Unwraps a lock result, recovering the guard from a poisoned mutex instead
/// of propagating the panic that poisoned it.
///
/// The cache treats poisoning as non-fatal: a panicking holder should not
/// permanently wedge population tracking for every other thread.
fn recover_guard<'a, T>(
    result: Result<MutexGuard<'a, T>, std::sync::PoisonError<MutexGuard<'a, T>>>,
) -> MutexGuard<'a, T> {
    match result {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    }
}
