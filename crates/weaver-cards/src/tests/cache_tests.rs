//! Unit tests for card extraction cache behaviour.

use std::path::Path;
use std::sync::{Arc, Barrier};
use std::thread;

use rstest::rstest;
use weaver_syntax::SupportedLanguage;

use crate::cache::{CardCache, CardCacheAddress, CardCacheKey};
use crate::tests::fixtures;
use crate::{
    CardExtractionInput, DetailLevel, ParserRegistry, TreeSitterCardExtractor, content_hash,
};

fn rust_path() -> &'static Path {
    Path::new("/tmp/weaver-cards-tests/cache.rs")
}

fn rust_source(name: &str) -> String {
    format!("fn {name}() -> usize {{\n    1\n}}\n")
}

fn extract_card(extractor: &TreeSitterCardExtractor, source: &str) -> crate::SymbolCard {
    extractor
        .extract(CardExtractionInput {
            path: rust_path(),
            source,
            line: 1,
            column: 4,
            detail: DetailLevel::Structure,
        })
        .expect("card extraction should succeed")
}

#[rstest]
fn cache_hit_returns_same_card() {
    let cache = CardCache::new(4);
    let card = fixtures::build_card_at_level(DetailLevel::Structure);
    let key = CardCacheKey::new(
        rust_path(),
        "fn greet() {}\n",
        CardCacheAddress {
            language: SupportedLanguage::Rust,
            detail: DetailLevel::Structure,
            line: 1,
            column: 4,
        },
    );

    cache.insert(key.clone(), card.clone());

    assert_eq!(cache.get(&key), Some(card));
    assert_eq!(cache.stats().hits, 1);
}

#[rstest]
fn cache_miss_returns_none() {
    let cache = CardCache::new(4);
    let key = CardCacheKey::new(
        rust_path(),
        "fn greet() {}\n",
        CardCacheAddress {
            language: SupportedLanguage::Rust,
            detail: DetailLevel::Structure,
            line: 1,
            column: 4,
        },
    );

    assert_eq!(cache.get(&key), None);
    assert_eq!(cache.stats().misses, 1);
}

#[rstest]
#[should_panic(expected = "card cache capacity must be greater than zero")]
fn zero_capacity_cache_panics() {
    drop(CardCache::new(0));
}

#[rstest]
fn content_change_invalidates_cache() {
    let extractor = TreeSitterCardExtractor::with_cache_capacity(8);
    let first = extract_card(&extractor, &rust_source("greet"));
    let second = extract_card(&extractor, &rust_source("welcome"));

    assert_ne!(first.etag, second.etag);
    assert_eq!(extractor.cache_len(), 1);
    assert_eq!(extractor.cache_stats().misses, 2);
}

#[rstest]
fn path_invalidation_removes_all_entries() {
    let cache = CardCache::new(8);
    let first_key = CardCacheKey::new(
        rust_path(),
        "fn first() {}\n",
        CardCacheAddress {
            language: SupportedLanguage::Rust,
            detail: DetailLevel::Structure,
            line: 1,
            column: 4,
        },
    );
    let second_key = CardCacheKey::new(
        rust_path(),
        "fn first() {}\n",
        CardCacheAddress {
            language: SupportedLanguage::Rust,
            detail: DetailLevel::Signature,
            line: 1,
            column: 4,
        },
    );
    cache.insert(
        first_key.clone(),
        fixtures::build_card_at_level(DetailLevel::Structure),
    );
    cache.insert(
        second_key.clone(),
        fixtures::build_card_at_level(DetailLevel::Signature),
    );

    cache.invalidate(rust_path());

    assert!(cache.is_empty());
    assert_eq!(cache.get(&first_key), None);
    assert_eq!(cache.get(&second_key), None);
}

#[rstest]
fn lru_eviction_removes_oldest() {
    let cache = CardCache::new(2);
    let first_key = CardCacheKey::new(
        Path::new("/tmp/weaver-cards-tests/first.rs"),
        "fn first() {}\n",
        CardCacheAddress {
            language: SupportedLanguage::Rust,
            detail: DetailLevel::Structure,
            line: 1,
            column: 4,
        },
    );
    let second_key = CardCacheKey::new(
        Path::new("/tmp/weaver-cards-tests/second.rs"),
        "fn second() {}\n",
        CardCacheAddress {
            language: SupportedLanguage::Rust,
            detail: DetailLevel::Structure,
            line: 1,
            column: 4,
        },
    );
    let third_key = CardCacheKey::new(
        Path::new("/tmp/weaver-cards-tests/third.rs"),
        "fn third() {}\n",
        CardCacheAddress {
            language: SupportedLanguage::Rust,
            detail: DetailLevel::Structure,
            line: 1,
            column: 4,
        },
    );

    cache.insert(
        first_key.clone(),
        fixtures::build_card_at_level(DetailLevel::Structure),
    );
    cache.insert(
        second_key.clone(),
        fixtures::build_card_at_level(DetailLevel::Structure),
    );
    assert!(cache.get(&second_key).is_some());
    cache.insert(
        third_key.clone(),
        fixtures::build_card_at_level(DetailLevel::Structure),
    );

    assert_eq!(cache.get(&first_key), None);
    assert!(cache.get(&second_key).is_some());
    assert!(cache.get(&third_key).is_some());
}

#[rstest]
fn cache_preserves_extraction_timestamp() {
    let extractor = TreeSitterCardExtractor::with_cache_capacity(8);
    let source = rust_source("greet");

    let first = extract_card(&extractor, &source);
    let second = extract_card(&extractor, &source);

    assert_eq!(
        first.provenance.extracted_at,
        second.provenance.extracted_at
    );
    assert_eq!(extractor.cache_stats().hits, 1);
}

#[rstest]
fn content_hash_is_deterministic() {
    assert_eq!(
        content_hash("fn greet() {}\n"),
        content_hash("fn greet() {}\n")
    );
}

#[rstest]
fn content_hash_differs_for_different_sources() {
    assert_ne!(
        content_hash("fn first() {}\n"),
        content_hash("fn second() {}\n")
    );
}

#[rstest]
fn parser_registry_reuses_parsers() {
    let registry = ParserRegistry::new();
    let first = registry
        .parser_identity(SupportedLanguage::Rust)
        .expect("parser identity should resolve");
    let second = registry
        .parser_identity(SupportedLanguage::Rust)
        .expect("parser identity should resolve");

    assert_eq!(first, second);
}

#[rstest]
fn cache_correctness_after_invalidation() {
    let extractor = TreeSitterCardExtractor::with_cache_capacity(8);
    let original_source = rust_source("greet");
    let updated_source = rust_source("welcome");

    let original = extract_card(&extractor, &original_source);
    extractor.invalidate_path(rust_path());
    let updated = extract_card(&extractor, &updated_source);

    assert_ne!(
        original.symbol.symbol_ref.name,
        updated.symbol.symbol_ref.name
    );
    assert_eq!(extractor.cache_len(), 1);
}

#[rstest]
fn extractor_reuses_parser_and_cache_for_identical_requests() {
    let extractor = TreeSitterCardExtractor::with_cache_capacity(8);
    let source = rust_source("greet");

    let parser_before = extractor
        .parser_identity(SupportedLanguage::Rust)
        .expect("parser identity should resolve");
    let _ = extract_card(&extractor, &source);
    let parser_after_first = extractor
        .parser_identity(SupportedLanguage::Rust)
        .expect("parser identity should resolve");
    let _ = extract_card(&extractor, &source);
    let parser_after_second = extractor
        .parser_identity(SupportedLanguage::Rust)
        .expect("parser identity should resolve");

    assert_eq!(parser_before, parser_after_first);
    assert_eq!(parser_after_first, parser_after_second);
    assert_eq!(extractor.cache_stats().hits, 1);
}

#[rstest]
fn identical_concurrent_requests_only_parse_once() {
    let extractor = Arc::new(TreeSitterCardExtractor::with_cache_capacity(8));
    let barrier = Arc::new(Barrier::new(2));
    let source = rust_source("greet");

    let handles: Vec<_> = (0..2)
        .map(|_| {
            let shared_extractor = Arc::clone(&extractor);
            let shared_barrier = Arc::clone(&barrier);
            let request_source = source.clone();
            thread::spawn(move || {
                shared_barrier.wait();
                extract_card(&shared_extractor, &request_source)
            })
        })
        .collect();

    let cards: Vec<_> = handles
        .into_iter()
        .map(|handle| handle.join().expect("thread should complete"))
        .collect();

    let first = cards.first().expect("first card should exist");
    let second = cards.get(1).expect("second card should exist");
    assert_eq!(first, second);
    assert_eq!(extractor.cache_stats().hits, 1);
    assert_eq!(extractor.cache_stats().misses, 1);
}
