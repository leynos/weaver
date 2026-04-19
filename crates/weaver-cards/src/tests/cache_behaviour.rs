//! Behaviour tests for cache-backed card extraction.

use std::path::PathBuf;

use rstest::fixture;
use rstest_bdd_macros::{given, scenario, then, when};

use crate::{CardExtractionError, CardExtractionInput, DetailLevel, TreeSitterCardExtractor};

fn rust_fixture_source(name: &str) -> String { format!("fn {name}() -> usize {{\n    1\n}}\n") }

#[derive(Default)]
struct CacheWorld {
    extractor: Option<TreeSitterCardExtractor>,
    path: Option<PathBuf>,
    source: Option<String>,
    line: u32,
    column: u32,
    cards: Vec<crate::SymbolCard>,
    error: Option<CardExtractionError>,
}

impl CacheWorld {
    fn request(&self) -> CardExtractionInput<'_> {
        CardExtractionInput {
            path: self.path.as_deref().expect("path should be set"),
            source: self.source.as_deref().expect("source should be set"),
            line: self.line,
            column: self.column,
            detail: DetailLevel::Structure,
        }
    }

    fn extractor(&self) -> &TreeSitterCardExtractor {
        self.extractor.as_ref().expect("extractor should be set")
    }
}

#[fixture]
fn world() -> CacheWorld {
    CacheWorld {
        line: 1,
        column: 4,
        ..CacheWorld::default()
    }
}

#[given("a fresh cache-backed extractor")]
fn given_fresh_extractor(world: &mut CacheWorld) {
    world.extractor = Some(TreeSitterCardExtractor::with_cache_capacity(8));
}

#[given("a cache-backed extractor with capacity {capacity}")]
fn given_sized_extractor(world: &mut CacheWorld, capacity: usize) {
    world.extractor = Some(TreeSitterCardExtractor::with_cache_capacity(capacity));
}

#[given("a Rust cache fixture named {name}")]
fn given_rust_fixture(world: &mut CacheWorld, name: String) {
    let fixture_name = name.trim_matches('"');
    world.path = Some(PathBuf::from("/tmp/weaver-cards-tests/cache_behaviour.rs"));
    world.source = Some(rust_fixture_source(fixture_name));
}

#[given("a second Rust cache fixture named {name} is extracted once")]
fn given_second_fixture_extracted(world: &mut CacheWorld, name: String) {
    let fixture_name = name.trim_matches('"');
    world.path = Some(PathBuf::from(
        "/tmp/weaver-cards-tests/cache_behaviour_second.rs",
    ));
    world.source = Some(rust_fixture_source(fixture_name));
    when_extracted_once(world);
}

#[given("an unsupported cache fixture")]
fn given_unsupported_fixture(world: &mut CacheWorld) {
    world.path = Some(PathBuf::from("/tmp/weaver-cards-tests/cache_behaviour.txt"));
    world.source = Some(String::from("plain text\n"));
}

#[given("the request position is {line}:{column}")]
fn given_request_position(world: &mut CacheWorld, line: u32, column: u32) {
    world.line = line;
    world.column = column;
}

#[when("the fixture is extracted twice")]
fn when_extracted_twice(world: &mut CacheWorld) {
    when_extracted_once(world);
    when_extracted_once(world);
}

#[when("the fixture is extracted once")]
fn when_extracted_once(world: &mut CacheWorld) {
    let result = world.extractor().extract(world.request());
    match result {
        Ok(card) => {
            world.cards.push(card);
            world.error = None;
        }
        Err(error) => world.error = Some(error),
    }
}

#[when("the Rust cache fixture changes to {name}")]
fn when_fixture_changes(world: &mut CacheWorld, name: String) {
    let fixture_name = name.trim_matches('"');
    world.source = Some(rust_fixture_source(fixture_name));
}

#[when("extraction fails")]
fn when_extraction_fails(world: &mut CacheWorld) {
    when_extracted_once(world);
    assert!(world.error.is_some(), "expected extraction to fail");
}

#[then("the cache records {hits} hit")]
#[then("the cache records {hits} hits")]
fn then_cache_hits(world: &mut CacheWorld, hits: u64) {
    assert_eq!(world.extractor().cache_stats().hits, hits);
}

#[then("the cache records {misses} miss")]
#[then("the cache records {misses} misses")]
fn then_cache_misses(world: &mut CacheWorld, misses: u64) {
    assert_eq!(world.extractor().cache_stats().misses, misses);
}

#[then("the extracted_at timestamp is preserved across requests")]
fn then_timestamp_preserved(world: &mut CacheWorld) {
    assert_eq!(world.cards.len(), 2);
    let first_timestamp = world
        .cards
        .first()
        .map(|card| card.provenance.extracted_at.as_str())
        .expect("first cached card should exist");
    let second_timestamp = world
        .cards
        .get(1)
        .map(|card| card.provenance.extracted_at.as_str())
        .expect("second cached card should exist");
    assert_eq!(
        first_timestamp, second_timestamp,
        "cached extraction should preserve the original timestamp"
    );
}

#[then("the cache stores {entries} entry")]
#[then("the cache stores {entries} entries")]
fn then_cache_entries(world: &mut CacheWorld, entries: usize) {
    assert_eq!(world.extractor().cache_len(), entries);
}

#[scenario(path = "tests/features/card_cache.feature")]
fn card_cache_behaviour(#[from(world)] world: CacheWorld) { let _ = world; }
