# 7.1.4 Cache integration for card extraction keyed by URI

This ExecPlan (execution plan) is a living document. The sections
`Constraints`, `Tolerances`, `Risks`, `Progress`, `Surprises & Discoveries`,
`Decision Log`, and `Outcomes & Retrospective` must be kept up to date as work
proceeds.

Status: DONE

## Purpose / big picture

After this change, repeated `observe get-card` requests for the same file at
the same revision will return cached symbol cards without reparsing the source
text, significantly reducing latency for AI agent workflows that issue many
card requests against an unchanged working tree. When file contents change (as
detected by a content hash), the cache invalidates deterministically and the
next request re-extracts from scratch, preserving correctness.

Observable behaviour after implementation:

1. A `get-card` request against a supported file returns a symbol card exactly
   as it does today (no change to the JSON schema or response envelope).
2. A second identical `get-card` request for the same file and revision returns
   the same card from cache without invoking Tree-sitter parsing. Integration
   tests verify this by asserting cache hit counts.
3. When the file is modified on disk (different content hash), the next
   `get-card` request re-extracts the card and the stale entry is evicted.
   Integration tests verify deterministic invalidation.
4. Cache misses (first request or after invalidation) produce byte-identical
   output to the pre-cache implementation, ensuring correctness.
5. The `provenance.extracted_at` field is populated with the actual extraction
   timestamp (replacing the placeholder `"1970-01-01T00:00:00Z"`), and cache
   hits preserve the original extraction timestamp.
6. The LRU eviction policy keeps memory bounded: the cache holds at most a
   configurable number of entries (default 128) per language.
7. A battery of 20 Python and 20 Rust language example fixtures exercises a
   range of symbol scenarios in end-to-end tests using `assert_cmd` and `insta`
   snapshot assertions.
8. `make check-fmt`, `make lint`, and `make test` all exit 0.

## Constraints

1. The public JSON schema of `SymbolCard`, `GetCardResponse`, `DetailLevel`,
   `Provenance`, and all types in `crates/weaver-cards/src/` is locked from
   roadmaps 7.1.1 and 7.1.2. The cache must not alter serialized field names,
   shapes, or ordering. The only schema-visible change is that
   `provenance.extracted_at` transitions from the placeholder to a real ISO
   8601 timestamp.
2. The workspace enforces strict linting: `unwrap_used`, `expect_used`,
   `indexing_slicing`, `string_slice`, `allow_attributes`, `missing_docs`,
   `missing_const_for_fn`, `cognitive_complexity`, and the 400-line file limit
   for code files. All Clippy warnings are denied.
3. Every new Rust module must begin with a `//!` comment, and all public items
   must have `///` rustdoc comments.
4. `GetCardResponse` is `#[non_exhaustive]`, so matches in `weaverd` must
   retain a wildcard arm.
5. Behaviour tests must use `rstest-bdd` v0.5.0 with the `world` fixture
   convention established in the codebase.
6. The Tree-sitter extraction layer in `crates/weaver-cards/` must remain
   independent of the Language Server Protocol (LSP). LSP enrichment is a
   post-extraction concern owned by the daemon handler in `crates/weaverd/`.
7. en-GB-oxendict spelling ("-ize" / "-yse" / "-our") for comments and
   documentation.
8. No single code file may exceed 400 lines.
9. The existing `TreeSitterSyntacticLock` in
   `crates/weaver-syntax/src/syntactic_lock.rs` already implements a
   `Mutex<HashMap<SupportedLanguage, Arc<Mutex<Parser>>>>` parser registry
   pattern. The card cache should reuse this proven pattern rather than
   inventing a new one.
10. Earlier roadmap drafts stated "Requires 4.3.4" (parse-cache adapter).
   Since 4.3.4 is not yet implemented, this plan implements the cache layer
   directly in the card extraction path as a self-contained prerequisite,
   establishing the URI + language + revision keying contract that 4.3.4 will
   later generalize.

## Tolerances (exception triggers)

- Scope: if implementation requires changes to more than 25 files (net), stop
  and escalate.
- Interface: if `SymbolCard`, `GetCardResponse`, or `DetailLevel` require
  schema changes beyond the `extracted_at` timestamp fix, stop and escalate.
- Dependencies: one new workspace dependency (`lru`) is expected. If more than
  one new external dependency is required, stop and escalate.
- Iterations: if tests still fail after 5 attempts at fixing a single issue,
  stop and escalate.
- Ambiguity: if multiple valid cache key designs exist and the choice
  materially affects correctness or performance, stop and present options.

## Risks

- Risk: The `lru` crate may not be in the workspace dependency set yet.
  Severity: low. Likelihood: high (confirmed — it is absent). Mitigation: add
  `lru = "0.12"` to the workspace `Cargo.toml` `[workspace.dependencies]` and
  reference it from `crates/weaver-cards/Cargo.toml`. This is a well-known,
  audited crate with minimal transitive dependencies.

- Risk: Cache key design — using content hash vs file modification time.
  Severity: medium. Likelihood: low. Mitigation: use a Blake3 or SHA-256 hash
  of the file content as the revision component of the cache key, consistent
  with the roadmap's "blob hash" language. This is deterministic and
  filesystem-metadata-independent.

- Risk: Thread safety of the cache in the daemon. The `weaverd` handler
  currently processes requests sequentially on a single connection, but may
  eventually process concurrent requests. Severity: low. Likelihood: low.
  Mitigation: wrap the cache in `Mutex` following the `TreeSitterSyntacticLock`
  pattern. This is safe for the current sequential model and ready for future
  concurrency.

- Risk: File size limits. The `get_card.rs` handler is 148 lines and
  `extract/mod.rs` is 356 lines. Adding cache logic inline could push files
  past 400 lines. Severity: low. Likelihood: medium. Mitigation: implement the
  cache as a new module `crates/weaver-cards/src/cache.rs` and integrate it
  through a thin facade in the handler.

- Risk: String cloning reduction may touch many extraction helpers. Severity:
  low. Likelihood: medium. Mitigation: focus cloning reduction on the
  highest-impact paths (entity candidate collection, card building) and measure
  with targeted unit tests rather than attempting a full-crate refactor.

- Risk: The 20+20 language example battery is a significant test data
  authoring effort. Severity: low. Likelihood: medium. Mitigation: organize
  examples as static `&str` constants in dedicated fixture modules, following
  the pattern in `crates/weaver-e2e/src/fixtures.rs`.

## Progress

- [x] Stage 0: Pre-flight validation (confirm build, tests, and lint pass on
  the current branch).
- [x] Stage A: Add `lru` workspace dependency and create the cache module in
  `crates/weaver-cards/src/cache.rs`.
- [x] Stage B: Integrate parser registry with card extraction, reusing the
  `TreeSitterSyntacticLock` pattern for parser pooling.
- [x] Stage C: Wire cache into the `weaverd` handler, replacing direct
  `TreeSitterCardExtractor::extract()` calls with cache-aware extraction.
- [x] Stage D: Replace the `EXTRACTED_AT_PLACEHOLDER` with real timestamps and
  ensure cache hits preserve the original extraction timestamp.
- [x] Stage E: Reduce unnecessary string cloning in card and region extraction
  hot paths.
- [x] Stage F: Write unit tests for cache behaviour (hit, miss, invalidation,
  eviction, correctness).
- [x] Stage G: Write BDD tests using `rstest-bdd` v0.5.0 covering cache happy
  and unhappy paths.
- [x] Stage H: Author 20 Python and 20 Rust language example fixtures for
  end-to-end testing.
- [x] Stage I: Write end-to-end tests using `assert_cmd` and `insta` covering
  the full range of symbol scenarios.
- [x] Stage J: Update `docs/users-guide.md` with cache behaviour documentation.
- [x] Stage K: Update `docs/jacquard-card-first-symbol-graph-design.md` with
  any design decisions taken.
- [x] Stage L: Mark roadmap 7.1.4 as done and run full validation suite.

## Surprises & discoveries

- Housing a shared `TreeSitterCardExtractor` inside
  `SemanticBackendProvider` gave the daemon one long-lived cache and parser
  registry without widening router or handler signatures.
- The workspace already used the `time` crate in `weaver-build-util`, so
  promoting it to a workspace dependency for runtime RFC 3339 formatting was
  lower risk than hand-writing UTC timestamp logic.
- `weaver-e2e` could not rely on Cargo's automatic binary-path injection for
  the `weaver` binary because that binary is defined in `weaver-cli`. The
  snapshot harness now bootstraps it once with
  `cargo build -p weaver-cli --bin weaver` and reuses `target/debug/weaver`.

## Decision log

- Decision: Use content-hash-based cache keys rather than filesystem metadata.
  Rationale: the roadmap specifies "blob hash" as the revision component, which
  is deterministic and portable across filesystems. File modification times are
  unreliable in CI and across git operations. Date/Author: 2026-03-25 / plan
  author.

- Decision: Implement the cache in `crates/weaver-cards/` rather than in
  `crates/weaverd/`. Rationale: the cache holds extracted entity tables and
  symbol cards, which are domain types owned by `weaver-cards`. Placing the
  cache here keeps the daemon handler thin and allows unit testing without
  daemon infrastructure. The handler passes a `&CardCache` reference to the
  extractor. Date/Author: 2026-03-25 / plan author.

- Decision: Use the `lru` crate (v0.12) rather than implementing a custom LRU.
  Rationale: `lru` is a well-established, audited crate with zero unsafe code
  and minimal dependencies. It provides `LruCache<K, V>` with O(1) get/put and
  configurable capacity. The roadmap explicitly calls for LRU policy.
  Date/Author: 2026-03-25 / plan author.

- Decision: Cache at the `SymbolCard` level (post-extraction) rather than at
  the `ParseResult` level (post-parse). Rationale: caching the fully-extracted
  card avoids re-running entity collection, candidate selection, and card
  building on cache hits. Parser reuse is handled separately via a parser
  registry (following the `TreeSitterSyntacticLock` pattern). Both layers
  contribute to performance: parser registry avoids grammar loading costs, and
  the card cache avoids extraction costs. Date/Author: 2026-03-25 / plan author.

- Decision: Cache key is a composite of (file path, content hash, language,
  detail level, line, column). Rationale: different positions in the same file
  yield different cards (different symbols), and different detail levels yield
  different card payloads. Including all six components ensures cache
  correctness. The content hash serves as the revision discriminator.
  Date/Author: 2026-03-25 / plan author.

- Decision: Share one extractor instance through `SemanticBackendProvider`
  instead of threading separate cache and parser-registry references through
  router call stacks. Rationale: this preserved existing handler signatures
  while still giving the daemon one shared cache and parser pool across all
  requests. Date/Author: 2026-03-26 / implementation.

## Outcomes & retrospective

- Implemented `crates/weaver-cards/src/cache.rs` with `CardCache`,
  `ParserRegistry`, SHA-256 content hashing, hit/miss stats, and stale-revision
  invalidation by path plus content hash.
- `TreeSitterCardExtractor` is now stateful and cache-aware, reuses parsers
  across requests, and emits real RFC 3339 timestamps in
  `provenance.extracted_at`.
- `weaverd` now shares one extractor instance via
  `SemanticBackendProvider`, and daemon tests assert cache hits, preserved
  timestamps, and deterministic invalidation after file edits.
- Added unit tests, BDD coverage, and `weaver-e2e` CLI snapshots covering 20
  Python fixtures, 20 Rust fixtures, refusal cases, detail-level variants, and
  repeated-request cache reuse.
- Full validation was run at the end of the implementation turn.

## Context and orientation

This section describes the current state of the code relevant to this task. All
paths are relative to the repository root.

### Crate layout

- `crates/weaver-cards/` — Symbol card extraction library. Owns `SymbolCard`,
  `GetCardRequest`, `GetCardResponse`, `TreeSitterCardExtractor`, and all
  extraction logic. Has no dependency on `weaverd` or LSP.
- `crates/weaverd/` — Daemon process. Routes JSONL commands. The handler at
  `crates/weaverd/src/dispatch/observe/get_card.rs` orchestrates card
  extraction, optional LSP enrichment, and response serialization.
- `crates/weaver-syntax/` — Tree-sitter wrapper. Provides `Parser`,
  `SupportedLanguage`, `ParseResult`, and the `TreeSitterSyntacticLock` parser
  registry.
- `crates/weaver-e2e/` — End-to-end test crate. Uses `assert_cmd` to spawn the
  `weaver` binary and `insta` for snapshot assertions.

### Key files

- `crates/weaver-cards/src/extract/mod.rs` (356 lines) — Contains
  `TreeSitterCardExtractor`, `CardExtractionInput`, `CardExtractionError`,
  `build_card()`, and helper functions. Currently creates a new `Parser` for
  every extraction call and uses `EXTRACTED_AT_PLACEHOLDER` for timestamps.
- `crates/weaver-cards/src/card.rs` — Defines `SymbolCard` with its `etag`
  field (content hash) already populated by `fingerprint::symbol_id()`.
- `crates/weaver-cards/src/extract/fingerprint.rs` — SHA-256-based
  `symbol_id()` function that produces deterministic `sym_<hex>` identifiers.
- `crates/weaverd/src/dispatch/observe/get_card.rs` (148 lines) — Handler
  that reads the file, creates a `TreeSitterCardExtractor`, calls `extract()`,
  optionally applies LSP enrichment, and serializes the response.
- `crates/weaver-syntax/src/syntactic_lock.rs` — Contains
  `TreeSitterSyntacticLock` with the parser registry pattern:
  `Mutex<HashMap<SupportedLanguage, Arc<Mutex<Parser>>>>`.
- `crates/weaver-syntax/src/parser.rs` — `Parser` struct wrapping
  `tree_sitter::Parser`. Requires `&mut self` for `parse()`. Parsers are
  reusable across multiple files of the same language.
- `crates/weaver-e2e/src/fixtures.rs` (260 lines) — Existing language fixtures
  for grep, rewrite, and definition tests. Contains 7 Python, 5 Rust, and 7
  TypeScript constants.
- `crates/weaver-cards/tests/features/get_card_schema.feature` — Existing BDD
  feature file for card schema tests.
- `crates/weaverd/tests/features/get_card.feature` — Existing BDD feature file
  for daemon-level get-card tests.
- `docs/users-guide.md` — User-facing documentation. Already documents
  `observe get-card` syntax, arguments, and behaviour.

### Existing patterns to follow

The `TreeSitterSyntacticLock` parser registry pattern in
`crates/weaver-syntax/src/syntactic_lock.rs` is the canonical example of parser
pooling in this workspace. It wraps parsers in `Arc<Mutex<Parser>>` inside a
`Mutex<HashMap<SupportedLanguage, ...>>` for thread-safe lazy initialization.
The card cache should follow the same concurrency pattern.

For test infrastructure:

- BDD tests use `rstest-bdd` v0.5.0 with `#[given]`, `#[when]`, `#[then]`
  step macros and `#[scenario]` binding. Feature files live under
  `tests/features/` within each crate. Step definitions live alongside source
  code in `src/tests/`.
- E2E tests in `crates/weaver-e2e/` use `assert_cmd::Command` to spawn the
  `weaver` binary and `insta::assert_snapshot!` (with
  `serde_json::to_string_pretty()` for stable JSON, because
  `assert_json_snapshot!` is unavailable in this workspace).
- Language fixtures are static `&str` constants in dedicated modules.

## Plan of work

### Stage 0: Pre-flight validation

Confirm the current branch builds and passes all gates before making changes.
Run `make check-fmt`, `make lint`, and `make test` through the `pipefail` plus
`tee` pattern. If any gate fails, investigate and fix before proceeding.

### Stage A: Add LRU dependency and create the cache module

Add `lru = "0.12"` to `Cargo.toml` under `[workspace.dependencies]`. Add `lru`
to `crates/weaver-cards/Cargo.toml` dependencies (workspace reference).

Create `crates/weaver-cards/src/cache.rs` containing:

1. A `CardCacheKey` struct with fields: `path: PathBuf`,
   `content_hash: [u8; 32]`, `language: SupportedLanguage` (from
   `weaver-syntax`), `detail: DetailLevel`, `line: u32`, `column: u32`.
   Implement `Hash` and `Eq`.

2. A `CardCache` struct wrapping `Mutex<LruCache<CardCacheKey, CachedCard>>`
   where `CachedCard` holds the `SymbolCard` and the extraction timestamp.

3. Public methods:

   - `CardCache::new(capacity: usize) -> Self` — creates a cache with the
     given maximum entry count.
   - `CardCache::get(&self, key: &CardCacheKey) -> Option<SymbolCard>` — returns
     a cached card if present (cache hit).
   - `CardCache::insert(&self, key: CardCacheKey, card: SymbolCard)` — inserts
     a card into the cache, evicting the least recently used entry if at
     capacity.
   - `CardCache::invalidate(&self, path: &Path)` — removes all entries for a
     given file path (used when the file changes).
   - `CardCache::len(&self) -> usize` and `CardCache::is_empty(&self) -> bool`
     — cache size introspection.

4. A `content_hash(source: &str) -> [u8; 32]` function using SHA-256 (the
   `sha2` crate is already a dependency of `weaver-cards`).

Export `CardCache`, `CardCacheKey`, and `content_hash` from the crate root.

### Stage B: Integrate parser registry with card extraction

Extend `TreeSitterCardExtractor` to optionally accept a parser registry, so
parsers are reused across extraction calls instead of being created fresh each
time.

1. Add a `ParserRegistry` struct in `crates/weaver-cards/src/cache.rs` (or a
   sibling module if `cache.rs` grows too large) following the
   `TreeSitterSyntacticLock` pattern:
   `Mutex<HashMap<SupportedLanguage, Arc<Mutex<Parser>>>>`.

2. Add a method `TreeSitterCardExtractor::extract_with_registry()` that
   accepts both `CardExtractionInput` and a `&ParserRegistry`, borrowing a
   parser from the registry instead of creating one. The existing `extract()`
   method remains unchanged for backward compatibility and tests that do not
   need caching.

3. The parser registry lazily initializes parsers on first use and returns
   them to the pool after extraction completes.

### Stage C: Wire cache into the daemon handler

Modify `crates/weaverd/src/dispatch/observe/get_card.rs` to use the cache:

1. The handler receives a `&CardCache` (and a `&ParserRegistry`) from the
   daemon's shared state. These are initialized once at daemon startup in
   `crates/weaverd/src/backends.rs` or a neighbouring module.

2. The handler flow becomes:

   - Parse request arguments (unchanged).
   - Resolve URI to path (unchanged).
   - Read file source (unchanged).
   - Compute `content_hash(source)`.
   - Construct `CardCacheKey` from path, hash, language, detail, line, column.
   - Check `cache.get(&key)`: if hit, use the cached card.
   - On miss: extract using `extract_with_registry()`, insert into cache.
   - Apply LSP enrichment if `detail >= Semantic` (unchanged, applied to both
     cached and fresh cards — LSP enrichment is not cached because it depends
     on server state).
   - Serialize and write response (unchanged).

3. Update `router.rs` to pass the cache and registry references to the handler.

### Stage D: Replace timestamp placeholder with real timestamps

1. In `crates/weaver-cards/src/extract/mod.rs`, remove the
   `EXTRACTED_AT_PLACEHOLDER` constant.

2. In `build_card()`, generate a real ISO 8601 timestamp for
   `provenance.extracted_at`. Use `std::time::SystemTime` and format manually
   (the workspace does not depend on `chrono` or `time`; a minimal UTC
   formatter is sufficient, or add a lightweight dependency if needed — but
   prefer a small handwritten formatter to avoid a new dependency).

3. In `CachedCard`, store the extraction timestamp alongside the card so that
   cache hits return the original extraction time, not the current time.

### Stage E: Reduce string cloning in hot paths

Review and reduce unnecessary `String::clone()` calls in the extraction
pipeline. Focus on:

1. `build_card()` in `extract/mod.rs` — the `symbol_id` is cloned for both
   `SymbolIdentity.symbol_id` and `etag`. Consider computing once and moving
   into the struct fields.

2. `provenance_sources()` in `extract/utils.rs` — currently clones a
   `OnceLock<Vec<String>>` base vector on every call. Consider returning a
   `Cow<'static, [String]>` or a small fixed-size array.

3. `leading_attachments()` — clones decorator vectors. Evaluate whether
   borrowing is feasible within the attachment bundling pipeline.

4. Entity candidate collection in `extract/languages/*.rs` — evaluate whether
   `String` fields on `EntityCandidate` can be replaced with borrowed
   references or interned strings for the hot paths. If lifetimes become too
   complex, document the decision and defer.

Do not chase diminishing returns. Focus on the paths that are hit on every
extraction and measure the impact conceptually (number of allocations removed).

### Stage F: Unit tests for cache behaviour

In `crates/weaver-cards/src/tests/`, create a new test module `cache_tests.rs`
containing:

1. `cache_hit_returns_same_card` — extract a card, insert into cache, retrieve,
   and assert structural equality.
2. `cache_miss_returns_none` — query for a key not in cache.
3. `content_change_invalidates_cache` — insert a card, change the source hash,
   and assert the old key misses.
4. `path_invalidation_removes_all_entries` — insert multiple cards for the same
   path (different positions), call `invalidate(path)`, and assert all are gone.
5. `lru_eviction_removes_oldest` — fill cache to capacity, insert one more,
   and assert the least recently used entry is evicted.
6. `cache_preserves_extraction_timestamp` — verify that cache hits return the
   original `extracted_at`, not a fresh timestamp.
7. `content_hash_is_deterministic` — verify that the same source text produces
   the same hash on repeated calls.
8. `content_hash_differs_for_different_sources` — verify that different source
   texts produce different hashes.
9. `parser_registry_reuses_parsers` — verify that two extraction calls for the
   same language reuse the same parser instance.
10. `cache_correctness_after_invalidation` — extract, invalidate, re-extract
   with modified source, and assert the new card reflects the modifications.

### Stage G: BDD tests with `rstest-bdd` v0.5.0

Create `crates/weaver-cards/tests/features/card_cache.feature` with scenarios:

1. "Cache hit for unchanged file" — Given a source file and a `get-card`
   extraction, When the same extraction is repeated, Then the cache returns the
   same card without reparsing.
2. "Cache miss on first request" — Given an empty cache, When a `get-card`
   extraction is performed, Then the card is extracted from source.
3. "Cache invalidation on content change" — Given a cached card for a file,
   When the file content changes, Then the next extraction produces a fresh
   card.
4. "LRU eviction under memory pressure" — Given a cache at capacity, When a
   new entry is inserted, Then the least recently used entry is evicted.
5. "Cache preserves extraction timestamp" — Given a cached card, When the
   cache is queried again, Then the `extracted_at` field matches the original
   extraction.

Implement step definitions in
`crates/weaver-cards/src/tests/cache_behaviour.rs` following the `RefCell`
world fixture pattern used elsewhere in the codebase.

Add unhappy path scenarios:

1. "Unsupported language bypasses cache" — When extraction fails with
   `UnsupportedLanguage`, Then no cache entry is created.
2. "Position out of range bypasses cache" — When extraction fails with
   `PositionOutOfRange`, Then no cache entry is created.

### Stage H: Author 20 Python and 20 Rust language example fixtures

Create `crates/weaver-e2e/src/card_fixtures.rs` (or extend
`crates/weaver-e2e/src/fixtures.rs` if it stays under 400 lines) containing 40
static `&str` constants, 20 for Python and 20 for Rust. Each fixture exercises
a different symbol scenario. The fixtures should cover:

**Python (20 examples):**

1. Simple function with parameters and return annotation.
2. Function with default parameter values.
3. Function with `*args` and `**kwargs`.
4. Async function definition.
5. Class with `__init__` and methods.
6. Class with class methods and static methods (`@classmethod`,
   `@staticmethod`).
7. Class with property decorators (`@property`).
8. Nested function (closure).
9. Lambda expression.
10. Generator function (`yield`).
11. Module-level variable assignment.
12. Module with import block (stdlib and third-party).
13. Function with docstring (Google style).
14. Function with docstring (NumPy style).
15. Dataclass with fields and methods (`@dataclass`).
16. Abstract base class with abstract methods.
17. Function with complex type annotations (`Union`, `Optional`, `Dict`).
18. Decorator stack (multiple decorators on one function).
19. Function with control flow (if/elif/else, for, while, try/except).
20. Empty file with only imports and module docstring.

**Rust (20 examples):**

1. Simple function with parameters and return type.
2. Function with generics and trait bounds.
3. Async function definition.
4. Struct definition with fields.
5. Enum definition with variants (unit, tuple, struct).
6. Impl block with methods (inherent).
7. Trait definition with methods.
8. Trait implementation for a struct.
9. Module-level constant and static.
10. Function with lifetime parameters.
11. Closure assigned to a variable.
12. Function with complex control flow (match, if let, for, while, loop).
13. Function with doc comments (`///` and `//!`).
14. Derive macro usage (`#[derive(...)]`) on a struct.
15. Attribute macros on functions (`#[test]`, `#[cfg(...)]`).
16. Type alias definition.
17. Use/import block (stdlib and external crates).
18. Function returning `Result` with error handling.
19. Struct with tuple fields (newtype pattern).
20. Empty module with only `use` statements and a module doc comment.

Each fixture should be short (5–30 lines), self-contained, and syntactically
valid.

### Stage I: End-to-end tests with `assert_cmd` and `insta`

Create `crates/weaver-e2e/tests/get_card_snapshots.rs` containing end-to-end
tests that:

1. Write each fixture to a temporary file.
2. Spawn a `weaverd` test daemon (or use the fake TCP server pattern from
   existing e2e tests).
3. Send `observe get-card` requests via the `weaver` binary using
   `assert_cmd::Command`.
4. Capture the JSON response and assert it using
   `insta::assert_snapshot!` with `serde_json::to_string_pretty()`.

Structure the tests as parameterized `rstest` cases over the fixture constants,
generating one snapshot per fixture. Use descriptive snapshot names that
include the language and scenario (e.g.,
`get_card_snapshots__python_simple_function`,
`get_card_snapshots__rust_struct_definition`).

Include tests for:

- Success cards at different detail levels (`minimal`, `signature`,
  `structure`).
- Refusal responses for unsupported languages and invalid positions.
- Repeated requests returning identical output (cache correctness).

### Stage J: Update user's guide

In `docs/users-guide.md`, in the `observe get-card` section, add a paragraph
explaining:

- Card extraction results are cached by file content. Repeated requests for the
  same file at the same content revision return cached results.
- Cache entries are automatically invalidated when file contents change.
- The cache uses an LRU eviction policy to bound memory usage.
- The `provenance.extracted_at` timestamp reflects when the card was first
  extracted (not when the cache was queried).

### Stage K: Update design document

In `docs/jacquard-card-first-symbol-graph-design.md`, in the "Performance
considerations" section (lines 953–965), add a note recording:

- The cache key design (path, content hash, language, detail, line, column).
- The decision to cache at the `SymbolCard` level rather than the `ParseResult`
  level.
- The decision to use SHA-256 content hashing (already used for symbol
  fingerprints) as the revision discriminator.

### Stage L: Mark roadmap and run full validation

1. In `docs/roadmap.md`, change the `[ ]` checkboxes for 7.1.4 and its sub-items
   to `[x]`.
2. Run the full validation suite:

```sh
set -o pipefail; make check-fmt 2>&1 | tee /tmp/7-1-4-check-fmt.log
set -o pipefail; make lint 2>&1 | tee /tmp/7-1-4-lint.log
set -o pipefail; make test 2>&1 | tee /tmp/7-1-4-test.log
```

All three must exit 0. If any fail, fix before marking complete.

## Concrete steps

### Pre-flight

```sh
cd /home/user/project
set -o pipefail; make check-fmt 2>&1 | tee /tmp/7-1-4-preflight-check-fmt.log
set -o pipefail; make lint 2>&1 | tee /tmp/7-1-4-preflight-lint.log
set -o pipefail; make test 2>&1 | tee /tmp/7-1-4-preflight-test.log
```

Expected: all three exit 0.

### Add workspace dependency

In `Cargo.toml` at the workspace root, under `[workspace.dependencies]`, add:

```toml
lru = "0.12"
```

In `crates/weaver-cards/Cargo.toml`, under `[dependencies]`, add:

```toml
lru = { workspace = true }
```

Verify compilation:

```sh
cargo check -p weaver-cards
```

### Create cache module

Create `crates/weaver-cards/src/cache.rs` and register it in
`crates/weaver-cards/src/lib.rs`. Export `CardCache`, `CardCacheKey`,
`ParserRegistry`, and `content_hash`.

### Wire into handler

Modify `crates/weaverd/src/dispatch/observe/get_card.rs` to accept and use the
cache. Update `crates/weaverd/src/dispatch/router.rs` to pass the cache
reference. Initialize the cache and parser registry in the daemon's shared
state.

### Run validation after each stage

After each stage, run:

```sh
set -o pipefail; make check-fmt 2>&1 | tee /tmp/7-1-4-stage-X-check-fmt.log
set -o pipefail; make lint 2>&1 | tee /tmp/7-1-4-stage-X-lint.log
set -o pipefail; make test 2>&1 | tee /tmp/7-1-4-stage-X-test.log
```

Replace `X` with the stage letter. All three must exit 0 before proceeding.

## Validation and acceptance

Quality criteria (what "done" means):

- Tests: `make test` passes with all new unit tests, BDD scenarios, and e2e
  snapshot tests green.
- Lint: `make check-fmt` and `make lint` exit 0.
- Cache behaviour: integration tests assert cache hit counts, invalidation
  correctness, and output stability.
- E2E coverage: 40 language fixture scenarios (20 Python, 20 Rust) produce
  snapshot-locked output.
- Documentation: `docs/users-guide.md` documents cache behaviour.
  `docs/jacquard-card-first-symbol-graph-design.md` records design decisions.
- Roadmap: 7.1.4 sub-items are marked `[x]`.

Quality method (validation steps):

```sh
set -o pipefail; make check-fmt 2>&1 | tee /tmp/7-1-4-final-check-fmt.log
set -o pipefail; make lint 2>&1 | tee /tmp/7-1-4-final-lint.log
set -o pipefail; make test 2>&1 | tee /tmp/7-1-4-final-test.log
```

All three exit 0. Review `/tmp/7-1-4-final-test.log` for the new test names.

## Idempotence and recovery

All stages are re-runnable. The cache module is additive (new files). The
handler modification is a controlled edit to an existing file. If a stage fails
partway through, fix the issue and re-run from the beginning of that stage. No
destructive operations are involved.

The `lru` dependency addition is idempotent — adding it twice has no effect.
Cache state is ephemeral (lives only in daemon memory), so restarting the
daemon clears the cache without side effects.

## Artifacts and notes

### Cache key structure

```rust
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct CardCacheKey {
    path: PathBuf,
    content_hash: [u8; 32],
    language: SupportedLanguage,
    detail: DetailLevel,
    line: u32,
    column: u32,
}
```

### CardCache structure

```rust
pub struct CardCache {
    inner: Mutex<LruCache<CardCacheKey, CachedCard>>,
}

struct CachedCard {
    card: SymbolCard,
    extracted_at: String,
}
```

### ParserRegistry structure

```rust
pub struct ParserRegistry {
    parsers: Mutex<HashMap<SupportedLanguage, Arc<Mutex<Parser>>>>,
}
```

### Content hash function

```rust
pub fn content_hash(source: &str) -> [u8; 32] {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(source.as_bytes());
    let result = hasher.finalize();
    let mut hash = [0u8; 32];
    hash.copy_from_slice(&result);
    hash
}
```

## Interfaces and dependencies

### New workspace dependency

- `lru = "0.12"` — LRU cache implementation. Zero unsafe code, minimal
  transitive dependencies.

### New public types in `crates/weaver-cards/`

In `crates/weaver-cards/src/cache.rs`:

```rust
/// LRU cache for extracted symbol cards, keyed by file path, content
/// hash, language, detail level, and position.
pub struct CardCache { /* ... */ }

impl CardCache {
    /// Creates a cache with the given maximum entry count.
    pub fn new(capacity: usize) -> Self;

    /// Returns a cached card if present (cache hit).
    pub fn get(&self, key: &CardCacheKey) -> Option<SymbolCard>;

    /// Inserts a card into the cache.
    pub fn insert(&self, key: CardCacheKey, card: SymbolCard);

    /// Removes all entries for a given file path.
    pub fn invalidate(&self, path: &Path);

    /// Returns the number of entries in the cache.
    pub fn len(&self) -> usize;

    /// Returns true if the cache contains no entries.
    pub fn is_empty(&self) -> bool;
}

/// Composite cache key for card lookups.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct CardCacheKey {
    pub path: PathBuf,
    pub content_hash: [u8; 32],
    pub language: SupportedLanguage,
    pub detail: DetailLevel,
    pub line: u32,
    pub column: u32,
}

/// Registry of reusable Tree-sitter parsers, one per language.
pub struct ParserRegistry { /* ... */ }

impl ParserRegistry {
    /// Creates an empty parser registry.
    pub fn new() -> Self;

    /// Returns a locked parser for the given language, creating one
    /// if not already cached.
    pub fn get_or_create(
        &self,
        language: SupportedLanguage,
    ) -> Result<Arc<Mutex<Parser>>, CardExtractionError>;
}

/// Computes a SHA-256 content hash for cache keying.
pub fn content_hash(source: &str) -> [u8; 32];
```

### Modified handler signature

In `crates/weaverd/src/dispatch/observe/get_card.rs`:

```rust
pub fn handle<W: Write>(
    request: &CommandRequest,
    writer: &mut ResponseWriter<W>,
    backends: &mut FusionBackends<SemanticBackendProvider>,
    cache: &CardCache,
    registry: &ParserRegistry,
) -> Result<DispatchResult, DispatchError>;
```

### New test files

- `crates/weaver-cards/src/tests/cache_tests.rs` — Unit tests for cache
  behaviour.
- `crates/weaver-cards/src/tests/cache_behaviour.rs` — BDD step definitions.
- `crates/weaver-cards/tests/features/card_cache.feature` — BDD feature file.
- `crates/weaver-e2e/src/card_fixtures.rs` — 40 language example fixtures.
- `crates/weaver-e2e/tests/get_card_snapshots.rs` — E2E snapshot tests.
