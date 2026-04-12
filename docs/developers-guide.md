# Weaver developer's guide

This guide documents internal development concerns: toolchain baselines,
configuration framework internals, and implementation details that contributors
need but operators do not. For user-facing behaviour see the
[user's guide](users-guide.md).

## Workspace baseline

The workspace targets `ortho_config` v0.8.0 and Rust 1.88.

## Configuration framework internals

### `ortho_config` v0.8.0 integration

`weaver_config::Config` declares its discovery policy inline through the
`#[ortho_config(discovery(...))]` attribute. The app name, dotfile, project
file, and `--config-path` flag are all defined next to the struct, so every
consumer shares the same generated loader without bespoke builders.

The `ortho_config` v0.8.0 loader preserves the stricter discovery and parsing
model adopted in earlier releases: if any discovered configuration file fails
to parse, `ConfigDiscovery::load_first` returns an aggregated `OrthoError`.
Both the CLI and daemon bubble that error to the user instead of quietly
falling back to defaults, making misconfigurations immediately visible.

Configuration is layered with `ortho_config`, producing the precedence order
`defaults < files < environment < CLI`. File discovery honours `--config-path`
alongside the standard XDG locations, ensuring the CLI and daemon resolve
identical results regardless of which component loads the settings.

### Dependency-graph resolution

The loader uses a dependency-graph model for layered configuration sources.
Sources are merged in precedence order: built-in defaults are overridden by
discovered files, which are overridden by environment variables, which are in
turn overridden by CLI flags. When multiple configuration files are discovered,
they are merged in the order `--config-path` first, then XDG locations in
standard search order. Later sources override earlier ones field-by-field.

### TOML parsing semantics

All configuration inputs are parsed per TOML v1 rules. Anchors and tags are not
applicable (those are YAML concepts); TOML scalars are strongly typed and
preserve their declared type without implicit coercion. Boolean values must be
`true` or `false` (the string `"yes"` is rejected as an invalid boolean, as
shown in the user-facing error example).

## Card extraction cache internals

The `observe get-card` path is optimized around two shared resources in
`crates/weaver-cards/`:

- `CardCache`, an LRU cache of extracted `SymbolCard` values keyed by request
  identity and source revision.
- `ParserRegistry`, a pool of reusable Tree-sitter parsers keyed by
  `SupportedLanguage`.

`TreeSitterCardExtractor` composes both resources. In production the daemon
constructs one extractor and reuses it across requests so cards and parsers
stay warm across repeated lookups.

### `CardCache` design

`CardCache` wraps `lru::LruCache<CardCacheKey, Arc<SymbolCard>>` behind a
`Mutex`. The cached payload is stored as `Arc<SymbolCard>` so a cache hit can
reuse the existing card without cloning a potentially large structure on every
lookup.

The cache also tracks:

- `hits` and `misses` counters for integration tests and operational checks.
- `in_flight`, a `HashSet<CardCacheKey>` guarded by a `Mutex`.
- `in_flight_ready`, a `Condvar` used to serialize concurrent population of the
  same key.

The in-flight set matters because multiple concurrent requests can miss the LRU
at the same time. Without a per-key population guard, each request would parse
the file and try to insert the same card independently. The current design lets
exactly one thread compute a missing card for a given `CardCacheKey`, while
other threads wait, re-check the cache, and reuse the inserted result.

Example:

```rust
use std::sync::Arc;
use weaver_cards::{CardCache, CardCacheAddress, CardCacheKey, DetailLevel};
use weaver_syntax::SupportedLanguage;

let cache = CardCache::new(128);
let key = CardCacheKey::new(
    std::path::Path::new("src/lib.rs"),
    "fn greet() {}\n",
    CardCacheAddress {
        language: SupportedLanguage::Rust,
        detail: DetailLevel::Structure,
        line: 1,
        column: 4,
    },
);

if let Some(card) = cache.get_shared(&key) {
    assert_eq!(card.symbol.name, "greet");
}
```

`CardCache::new` rejects a zero capacity. A zero-capacity cache would silently
degrade into surprising behaviour, so callers must choose an explicit positive
bound.

### `CardCacheKey` composition

`CardCacheKey` combines the pieces of state that make one extraction result
meaningfully different from another:

- `path: PathBuf`
- `content_hash: [u8; 32]`
- `language: SupportedLanguage`
- `detail: DetailLevel`
- `line: u32`
- `column: u32`

The `content_hash` is a SHA-256 digest of the source text, not file metadata.
That keeps the key stable across timestamp-only filesystem changes while still
invalidating entries when the content changes.

`CardCacheAddress` carries the request-specific fields before key construction.
That keeps call sites explicit about which parts of a request affect cache
identity.

Example:

```rust
use weaver_cards::{CardCacheAddress, CardCacheKey, DetailLevel};
use weaver_syntax::SupportedLanguage;

let source = "def greet(name: str) -> str:\n    return f\"hi {name}\"\n";
let address = CardCacheAddress {
    language: SupportedLanguage::Python,
    detail: DetailLevel::Semantic,
    line: 1,
    column: 5,
};

let key = CardCacheKey::new(
    std::path::Path::new("/workspace/app.py"),
    source,
    address,
);

assert_eq!(key.path(), std::path::Path::new("/workspace/app.py"));
```

The path component is matched by exact `PathBuf` equality. Relative paths,
symlinked paths, and canonicalized paths do not collapse to one cache entry
unless the caller normalizes them before building the key.

### `ParserRegistry` pooling pattern

Tree-sitter parser construction is not free, so `ParserRegistry` keeps one
`Parser` per `SupportedLanguage` in a shared map:

```rust
HashMap<SupportedLanguage, Arc<Mutex<Parser>>>
```

The first request for a language creates the parser and stores it in the map.
Later requests clone the `Arc`, lock the parser for the duration of one parse,
and reuse the same initialized parser instance.

This pattern matches the cache design:

- one shared registry per long-lived extractor
- fine-grained reuse by language
- a narrow lock scope around the actual parse call

Example:

```rust
use weaver_cards::ParserRegistry;
use weaver_syntax::SupportedLanguage;

let registry = ParserRegistry::new();

let rust_tree = registry.parse(
    SupportedLanguage::Rust,
    "fn greet(name: &str) -> String { format!(\"hi {name}\") }\n",
)?;
let python_tree = registry.parse(
    SupportedLanguage::Python,
    "def greet(name: str) -> str:\n    return f\"hi {name}\"\n",
)?;

assert!(rust_tree.root_node().is_named());
assert!(python_tree.root_node().is_named());
# Ok::<(), weaver_syntax::SyntaxError>(())
```

The registry is intentionally independent of the daemon and independent of the
Language Server Protocol (LSP). Tree-sitter extraction remains the baseline
syntax pass, while LSP enrichment is added later by `weaverd` when a request
asks for semantic detail and a backend is available.

### Cache invalidation strategies

The cache uses two invalidation paths, each for a different event:

- `invalidate(path)` removes every cached card for that exact path.
- `invalidate_stale_revisions(path, current_hash)` removes entries for the same
  path whose content hash no longer matches the current source text.

`invalidate(path)` is the blunt tool used when the caller knows an entire
document identity is no longer valid. `invalidate_stale_revisions(...)` is the
steady-state path used during extraction: once a new card is computed, older
revisions for that path are evicted and the current revision stays cached.

Examples:

```rust
use std::path::Path;
use weaver_cards::CardCache;

let cache = CardCache::new(128);
let path = Path::new("/workspace/src/lib.rs");

cache.invalidate(path);
```

```rust
use std::path::Path;
use weaver_cards::{content_hash, CardCache};

let cache = CardCache::new(128);
let path = Path::new("/workspace/src/lib.rs");
let source = "fn greet() {}\n";

cache.invalidate_stale_revisions(path, &content_hash(source));
```

Because invalidation is exact-path based, callers that want symlink and
relative-path stability should normalize paths at the boundary where requests
enter the cacheable extraction flow.

### `TreeSitterCardExtractor` integration

`TreeSitterCardExtractor` is the orchestrator that ties cache keys, parser
pooling, and language-specific extraction together.

The cache-aware `extract_shared(...)` flow is:

1. Detect `SupportedLanguage` from the request path.
2. Build a `CardCacheKey` from path, source, detail, and cursor position.
3. Probe the cache with `peek_shared(...)`.
4. On a hit, record a cache hit and return the shared card immediately.
5. On a miss, acquire `lock_population(&cache_key)` so only one thread fills
   that key.
6. Re-check the cache with `get_shared(...)` in case another thread won the
   race while the current thread was waiting.
7. Parse through `ParserRegistry`, build the card, invalidate stale revisions,
   and insert the new shared card into `CardCache`.

Example:

```rust
use std::sync::Arc;
use weaver_cards::{CardCache, CardExtractionInput, ParserRegistry};
use weaver_cards::TreeSitterCardExtractor;

let extractor = TreeSitterCardExtractor::with_shared_resources(
    Arc::new(CardCache::new(256)),
    Arc::new(ParserRegistry::new()),
);

let card = extractor.extract_shared(CardExtractionInput {
    path: std::path::Path::new("/workspace/src/lib.rs"),
    source: "fn greet() {}\n",
    line: 1,
    column: 4,
    detail: weaver_cards::DetailLevel::Structure,
})?;

assert_eq!(card.symbol.name, "greet");
# Ok::<(), weaver_cards::CardExtractionError>(())
```

The plain `extract(...)` method remains available for callers that want an
owned `SymbolCard`, but the daemon and other long-lived services should prefer
shared resources and `extract_shared(...)` so cache hits do not pay for extra
deep clones.

## Graph-slice request parsing architecture

The `observe graph-slice` request parsing pipeline lives in
`crates/weaver-cards/src/graph_slice/` and is split across three internal
modules:

- `request.rs` — public schema types (`GraphSliceRequest`,
  `SliceDirection`, `SliceEdgeType`, `GraphSliceError`) and the
  `GraphSliceRequest::parse(&[String])` entry point.
- `parse.rs` — the `RequestBuilder` accumulator and `Flag` enum that
  drives the flag-dispatch loop.
- `parse_helpers.rs` — value-level parsing functions that validate and
  convert raw string arguments into typed values.

### `RequestBuilder` accumulator pattern

`RequestBuilder` is a private struct with `Option<T>` fields for every flag.
The parse flow is:

1. `GraphSliceRequest::parse(args)` creates an empty `RequestBuilder`
   and iterates over `args` using a `Peekable` iterator.
2. Each `--flag` token is dispatched through
   `try_apply_known_flag(flag, iter)`, which delegates to
   `try_apply_traversal_flag`, `try_apply_budget_flag`, and
   `try_apply_detail_flag` in order.
3. Each sub-dispatcher calls `require_arg_value(iter, Flag::*)` to
   consume the next token, then passes the resulting `RawValue` to a typed
   parser (e.g. `parse_uri`, `parse_position`, `parse_u32`).
4. After all tokens are consumed, `build()` resolves defaults for
   missing fields and constructs the final `GraphSliceRequest`.

Unknown flags return `GraphSliceError::UnknownFlag`. Bare positional tokens
return `GraphSliceError::UnknownArgument`.

### `Flag` enum

`Flag` identifies which CLI flag is being processed. It implements `Display` to
produce the `--flag-name` string for error messages and `Into<String>` for
`GraphSliceError` construction. The enum lives in `parse.rs` with
`pub(in crate::graph_slice)` visibility so `parse_helpers.rs` can reference it.

### `parse_helpers` module

`parse_helpers.rs` contains pure parsing functions that accept a `RawValue` (a
flag–value pair) and return a typed result or `GraphSliceError`. The key
helpers are:

- `require_arg_value` — consumes the next iterator token, rejecting
  `--`-prefixed tokens as missing values.
- `parse_uri` — validates a `file://` Uniform Resource Identifier (URI) prefix.
- `parse_position` — splits `LINE:COL` and validates 1-indexed values.
- `parse_u32` — parses a non-negative integer.
- `parse_direction` — delegates to `SliceDirection::from_str`.
- `parse_edge_types` — splits a comma-separated list and parses each
  token into a `SliceEdgeType`.
- `parse_confidence` — parses a float and validates the `[0.0, 1.0]`
  range.
- `parse_detail` — delegates to `DetailLevel::from_str`.

Each helper produces a `GraphSliceError::InvalidValue` with the originating
flag name and a descriptive message on failure, so callers do not need to
format error context themselves.
