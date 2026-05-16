# Weaver developer's guide

This guide documents internal development concerns: toolchain baselines,
configuration framework internals, and implementation details that contributors
need but operators do not. For user-facing behaviour see the
[user's guide](users-guide.md).

## Workspace baseline

The workspace targets `ortho_config` v0.8.0 and Rust 1.88.

## Adding or renaming public commands

ADR 007 makes the 0.1.0 public command surface resource-first and generated
from OrthoConfig-backed command metadata plus Weaver-owned semantic adapters.
Do not add new public command grammar by hand in `clap`, daemon routing, manual
pages, help text, and docs independently.

When adding or renaming a public command:

1. Update the OrthoConfig-backed command metadata or the Weaver semantic
   command-surface adapter, depending on whether the change is reusable
   command-contract machinery or Weaver-specific semantic behaviour.
2. Declare the capability ID the command exposes, or reuse an existing
   capability such as `definition.get`, `references.list`, `diagnostics.list`,
   `symbol.rename`, `symbol.move`, or `patch.apply`.
3. Provide localized human message IDs, copy-pasteable examples, and the human
   renderer layout hints needed for default output, `--plain`, colour control,
   and terminal-width fallbacks.
4. Provide JSON success and error schemas. Protocol identifiers, field names,
   schema versions, enum values, capability IDs, and exit classes are stable
   and non-localized.
5. Declare selector support, mutability, async behaviour, pagination bounds,
   profile interaction, delivery support, and feedback exposure.
6. Update or regenerate documentation snippets, `context --json`
   introspection, skill-manifest references, manpage inputs, shell completions,
   and tests from the same metadata.
7. Run the drift, vocabulary, renderer, JSON, bounded-output, capability, and
   example gates before merging the command change.

Provider names are not the ordinary public workflow. Commands should expose
semantic resources and capabilities, while Rope, rust-analyzer, Sempai,
Tree-sitter, Language Server Protocol (LSP) servers, and built-in helpers stay
behind deterministic provider routing. Provider names may appear in provenance,
diagnostics, `--verbose` output, policy, and expert overrides.

Sections below that document `observe`, `act`, `verify`, or `act refactor`
describe the current prototype implementation. They are useful for maintaining
shipped code, but they do not define the 0.1.0 public command contract.

## Sempai overview

Sempai is Weaver's Semgrep-compatible query engine facade. It parses rule YAML,
normalizes supported search syntax into a canonical formula model, and prepares
per-language query plans for later execution.

## Sempai query pipeline (prototype archive milestone 4.1.5)

- Canonical model (`sempai_core::formula`):
  - `Formula` enum: `Atom`, `Not`, `Inside`, `Anywhere`, `And`, `Or`
  - `Atom` enum: `Pattern`, `Regex`, `TreeSitterQuery`
  - `Decorated<T>` wrapper: `where_clauses`, `as_name`, `fix`, `span`
- Normalization (`crates/sempai/src/normalize.rs`):
  - Legacy syntax: `pattern*`, `patterns`, `pattern-either`,
    `pattern-not`, `pattern-not-inside`, `pattern-not-regex`, and
    `semgrep-internal-pattern-anywhere`
  - v2 `match` syntax: `pattern`, `regex`, `all`, `any`, `not`, `inside`,
    `anywhere`, and decorated metadata propagation
  - Special handling: `r2c-internal-project-depends-on` lowers to
    `(__NONEXISTENT_NODE__) @_dependency_check`
- Semantic validation (`crates/sempai/src/semantic_check.rs`):
  - `InvalidNotInOr`
  - `MissingPositiveTermInAnd`
  - Span precedence rules: node span -> first child -> fallback
- Engine wiring (`crates/sempai/src/engine.rs`):
  - Parse -> validate modes -> normalize -> validate semantics -> compile
    per-language `QueryPlan`
  - `QueryPlan::formula()` exposure for tests and integration

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

## Graph-slice handler architecture

The `observe graph-slice` command handler lives in
`crates/weaverd/src/dispatch/observe/graph_slice.rs`. It is the sole
implementation of the stable same-file slice contract.

### Entry point

`handle(request, writer, backends)` is the public entry point wired by the
router. It delegates to `build_response` which owns all domain logic.

### Same-file discovery

`discover_same_file_cards(request, document, entry_symbol_id, backends)` drives
sibling discovery:

1. `candidate_positions(source)` yields `(line, column)` pairs for the first
   non-whitespace character of each non-blank line, capped at
   `MAX_SAME_FILE_DISCOVERY_POSITIONS` to bound runtime cost.
2. `extract_same_file_card` calls the card extractor at each position,
   returning `None` for benign misses (`NoSymbolAtPosition`,
   `UnsupportedLanguage`) and `Err` for unexpected failures.
3. Candidates matching the entry symbol ID are filtered out; remaining cards
   are deduplicated by `symbol_id` using a `BTreeMap`.

### Budget and spillover

`apply_card_budget(entry_card, sibling_cards, max_cards)` partitions the sorted
sibling list into an included set (up to `max_cards − 1` siblings plus the
entry card) and a `SliceSpillover` frontier. The `apply_card_budget` branch for
`max_cards == 0` is only a defensive internal check; the public request parser
rejects `--max-cards 0` before dispatch, so callers cannot reach the
zero-budget `SliceSpillover` path through the CLI.

### Enrichment ordering

LSP semantic enrichment is applied **after** budget truncation so that only
cards included in the response pay the enrichment cost. The entry card is
enriched before discovery; included sibling cards are enriched immediately
after `apply_card_budget` returns.

### Error mapping

`map_extraction_error` converts `CardExtractionError` variants into structured
`GraphSliceResponse::Refusal` payloads. IO failures reading the source file are
mapped to `DispatchError::invalid_arguments` because the caller is responsible
for supplying a valid URI pointing to a readable file.

### Stable card ordering

`stable_card_order` imposes a deterministic total order over `SymbolCard`
values so that slice responses are reproducible regardless of extraction order.

## E2E test support for graph-slice

The end-to-end graph-slice test harness lives in
`crates/weaver-e2e/tests/graph_slice_snapshots.rs` and is backed by helpers in
`crates/weaver-e2e/tests/test_support/mod.rs`.

### `GraphSliceRequest`

`GraphSliceRequest<'a>` carries the parameters for one
`weaver observe graph-slice` CLI invocation: `uri`, `line`, `column`,
`entry_detail`, `node_detail`, and an optional `max_cards` budget.

### `run_graph_slice`

`run_graph_slice(daemon, request)` invokes the CLI via the test daemon socket
and returns a `Transcript` containing `stdout` (the JSONL response envelope)
and `stderr`.

### `fixture_uri`

`fixture_uri(temp_dir, case)` materializes a `CardFixtureCase` source file into
`temp_dir` and returns its `file://` URI so that snapshot tests operate on a
real filesystem path.

### `assert_named_snapshot`

`assert_named_snapshot(name, content)` wraps `insta::assert_snapshot!` with an
explicit snapshot name, storing results under
`crates/weaver-e2e/tests/snapshots/<name>.snap`.

### Fixture batteries

`crates/weaver-e2e/src/graph_slice_fixtures/` re-exports `PYTHON_CASES` (20
entries) and `RUST_CASES` (20 entries) from the shared `card_fixtures`
catalogue. `GraphSliceFixtureCase` is a type alias for `CardFixtureCase`.

### Snapshot test structure

`graph_slice_snapshots.rs` contains four `#[rstest]` tests:

- `graph_slice_semantic_snapshots_cover_python_and_rust_fixture_battery` —
  runs all 40 fixture cases and asserts both explicit structural fields and a
  named insta snapshot.
- `graph_slice_truncation_snapshots` — exercises the `max_cards=1` budget for
  two multi-symbol fixtures and asserts exactly one card with truncated
  spillover.
- `graph_slice_refusal_snapshots` — exercises the unsupported-language refusal
  path and asserts `refusal.reason == "unsupported_language"`.
- `graph_slice_refusal_position_out_of_range` — exercises the refusal path for
  an out-of-range position and asserts
  `refusal.reason == "position_out_of_range"`.

## Public API additions in prototype archive milestone 7.2.1

### `handle` signature — `FusionBackends` parameter

`handle(request, writer, backends)` now accepts
`&mut FusionBackends<SemanticBackendProvider>` as a third argument (wired by
the router from `BackendManager`). This parameter provides access to the card
extractor and LSP enrichment backend. It is consumed by `build_response` and
passed through to `discover_same_file_cards` and `enrich_card_if_requested`.

### Trait derivations enabling deterministic ordering

Two enums gained `PartialOrd` and `Ord` so that `stable_card_order` can sort
cards without a custom comparator:

| Type             | Crate          | New derives         |
| ---------------- | -------------- | ------------------- |
| `CardSymbolKind` | `weaver-cards` | `PartialOrd`, `Ord` |
| `SymbolKind`     | `weaver-graph` | `PartialOrd`, `Ord` |

*Table: New derive traits for `CardSymbolKind` and `SymbolKind`*

The derived order follows Rust's default discriminant ordering (declaration
order in the `enum`). Tests in each crate's `ordering_tests` module lock this
contract.

### `schema_version` field on `GraphSliceResponse` variants

Both `GraphSliceResponse::Success` and `GraphSliceResponse::Refusal` now carry
a `schema_version: String` field set to `"graph_slice.v1"`. All constructors
(including `not_yet_implemented`) populate this field. Cucumber contract tests
in `crates/weaver-cards/tests/features/graph_slice_schema.feature` assert its
presence.

### `TestDaemon` API changes

`TestDaemon::join(mut self)` now takes ownership of the optional
`join_handle: Option<thread::JoinHandle<()>>`, joins the daemon thread, and
calls `std::panic::resume_unwind(payload)` if the thread panicked, preserving
the original panic payload for diagnostics. It then calls `cache_stats()` after
the thread finishes.

`join_handle` is stored as `Option<thread::JoinHandle<()>>` (previously
`thread::JoinHandle<()>`) to allow the join to consume the handle via
`Option::take`.

### `test_support` visibility promotions

The following fields were promoted from `pub(crate)` to `pub` to allow access
from the new `graph_slice_snapshots.rs` test binary:

| Struct               | Fields promoted                                 |
| -------------------- | ----------------------------------------------- |
| `CacheTranscript`    | `first`, `second`, `cache_hits`, `cache_misses` |
| `GetCardRequest<'a>` | `uri`, `line`, `column`, `detail`               |

*Table: Visibility promotions in `test_support`*

`GraphSliceRequest<'a>` was added as a new `pub(crate)` struct with fields
`uri`, `line`, `column`, `entry_detail`, `node_detail`, and
`max_cards: Option<u32>`.

## CLI help and preflight internals

### 2.1 CLI help rendering architecture

The runtime parser strips `--config-path`, `--daemon-socket`, `--log-filter`,
`--log-format`, `--capability-overrides`, and `--locale` from `argv` before it
hands control to clap. This keeps the runtime `Cli::command()` definition
strict: the base clap command describes only runtime domains, operations, and
structured subcommands, so configuration flags never appear in the parser that
handles ordinary execution.

`crates/weaver-cli/src/help.rs` provides the documentation-facing layer.
`help::command()` starts from `Cli::command()`, adds the explicit
`--config-path` argument, then iterates over
`Config::get_doc_metadata().fields` to build the remaining shared configuration
flags dynamically from `ortho_config` metadata. Each visible field becomes a
`clap::Arg` with the correct long flag and value name for the generated help
surface. The help-only parser deliberately does not attach config value
validators because runtime config parsing owns case handling and validation.

The augmented command is used in both places that need truthful help text:

- runtime `--help` rendering, where the CLI prints help without invoking the
  configuration loader or starting the daemon;
- `clap_mangen` man page generation in `crates/weaver-cli/build.rs`, so the
  generated roff output stays aligned with the runtime help surface.

### 2.2 Augmented command pattern

The CLI deliberately uses two clap command shapes:

- the *runtime* command, which is strict and excludes configuration flags;
- the *help* command, which is augmented with configuration metadata for
  documentation only.

This split preserves the current runtime contract that configuration flags take
effect only when they appear before the command domain. It also avoids teaching
clap to accept post-domain configuration flags that the loader would ignore.

The augmented builder promotes clap argument IDs, long flag names, value names,
and any future possible-value metadata to `&'static str` values. Clap requires
`'static` lifetimes for dynamically constructed arguments, so the builder
intentionally leaks those bounded allocations with `Box::leak`. The leaked
strings live for the process lifetime and are intentionally never freed; the
augmented command is cached once per process, so repeated help rendering does
not allocate another set of argument metadata.

### 2.3 Preflight boundary (`crates/weaver-cli/src/preflight.rs`)

`handle_preflight` runs after `argv` splitting and before configuration
loading. At that point, the CLI has enough information to reject certain
invocations locally, without consulting the daemon or trying to load the full
configuration stack.

The preflight contract is narrow: emit actionable guidance to `stderr` when the
operator has not supplied a meaningful command shape, then return an `AppError`
that stops execution before daemon startup.

There are two preflight paths:

- bare invocation: a call with no domain, no structured subcommand, and no
  capability probe. This path emits the short domain guidance block and returns
  `AppError::BareInvocation`.
- domain guidance: a call with an unknown domain or a known domain that is
  missing its operation. This path emits contextual guidance and returns
  `AppError::PreflightGuidance`.

This boundary exists specifically to keep operator guidance local, immediate,
and side effect free.

### 2.4 `Locale` type (`crates/weaver-config/src/locale.rs`)

`Locale` is a small newtype around `ortho_config::LanguageIdentifier`. It
accepts only well-formed BCP 47 language tags, so invalid locale strings are
rejected at the shared configuration boundary instead of leaking deeper into
CLI or daemon startup.

The built-in default is `en-US`. That value is part of the current shared
configuration contract and is available from files, environment variables, and
CLI configuration flags like any other config field.

Full CLI localization bootstrap was historically deferred to prototype archive
roadmap item `3.3.1`; the forward command-surface reset carries localized
renderer work in roadmap item `13.2.1`. Weaver does not yet resolve the final
locale and use it to construct the `Localizer` before clap parse errors are
formatted. The current `Locale` type exists, so the configuration contract is
real now, and the later localization bootstrap can reuse the validated domain
value.

### 2.5 Daemon command execution glue (`crates/weaver-cli/src/runner_glue.rs`)

`runner_glue` extracts the daemon transport path from `lib.rs` so the top-level
runtime stays small enough to scan. Its two `pub(crate)` entry points are:

- **`execute_daemon_command`** — builds a `CommandRequest`, connects to the
  daemon socket (auto-starting the daemon if it is not running), writes the
  request as JSON Lines, and processes daemon response messages, returning an
  `ExitCode`. On transport or IO failure it writes a human-readable error to
  `stderr` and returns `ExitCode::FAILURE`.

- **`build_request`** — constructs a `CommandRequest` from a
  `CommandInvocation`. For `apply-patch` operations it drains `stdin` into the
  request patch field and returns `AppError::MissingPatchInput` when the
  content is empty after trimming. It also enforces the JSON Lines request size
  cap from `weaver_daemon_types::JSONL_REQUEST_MAX_LINE_BYTES`; oversized stdin
  is rejected with an early request error before patch processing starts. For
  all other operations, it constructs the request without reading `stdin`.

The module keeps connection retry logic in `start_and_retry_daemon`, which
tolerates socket-bind lag after daemon startup, and `write_error_and_fail`, a
small helper that writes a display message to `stderr` and returns
`ExitCode::FAILURE`.

## Test infrastructure for rename-symbol coverage

### `test-support` feature (`weaver-plugins`)

The `weaver-plugins` crate exposes shared contract fixtures behind the
`test-support` Cargo feature. Activate it in a crate's `[dev-dependencies]` to
access:

- `RenameSymbolRequestFixture` / `RenameSymbolResponseFixture` — type aliases
  for the request and response sides of the contract.
- `rename_symbol_request_fixtures()` / `rename_symbol_response_fixtures()` —
  the canonical fixture collections consumed by plugin contract tests.
- `rename_symbol_request_fixture_named(name)` /
  `rename_symbol_response_fixture_named(name)` — look up a single named request
  or response fixture by key and panic when the requested fixture name is
  unknown.
- `validate_rename_symbol_request_fixture(fixture)` /
  `validate_rename_symbol_response_fixture(fixture)` — run contract validation
  without panicking and return `Result<(), PluginError>` so callers can inspect
  the exact contract failure.
- `assert_rename_symbol_request_fixture_contract` /
  `assert_rename_symbol_response_fixture_contract` — assertion helpers that
  validate a fixture against the `RenameSymbolContract` and panic with a
  descriptive message on failure.

```toml
[dev-dependencies]
weaver-plugins = { path = "../weaver-plugins", features = ["test-support"] }
```

Typical lookup and validation usage:

```rust
use weaver_plugins::{
    rename_symbol_request_fixture_named, validate_rename_symbol_request_fixture,
};

let fixture = rename_symbol_request_fixture_named("valid_request");
let result = validate_rename_symbol_request_fixture(&fixture);
assert!(result.is_ok(), "fixture should satisfy the shared contract");
```

### `FakeDaemon` (`weaver-e2e/tests/test_support/daemon_harness.rs`)

`FakeDaemon` is a lightweight in-process TCP server used by end-to-end snapshot
tests. It binds an ephemeral port, records incoming JSON request payloads, and
writes deterministic responses so that tests run without a real daemon process.

Typical usage:

```rust
let daemon = FakeDaemon::start(1, "renamed_symbol").expect("fake daemon should start");
let endpoint = daemon.endpoint(); // pass to --daemon-socket
// … run CLI command …
let requests = daemon.requests();
daemon.join();
```

Pass `endpoint()` to the `--daemon-socket` flag of the `weaver` binary under
test. Call `join()` after the CLI exits to assert that the background thread
did not panic.

### Request-routing helpers (`weaver-e2e/tests/test_support/refactor_routing.rs`)

`refactor_routing` provides the routing logic used inside `FakeDaemon` to
produce capability-resolution payloads:

- `request_arguments(&serde_json::Value)` — extracts the daemon request's flat
  CLI-style argument vector, for example a list containing `--refactoring`,
  `rename`, `--file`, `src/main.py`, `new_name=renamed_symbol`, and `offset=4`.
- `argument_value(arguments, "--file")` — returns the value paired with a flag
  from that flat argument vector, normalizing access to values such as
  `Some("src/main.py")`.
- `language_for_extension(&Path)` — maps `.py` → `"python"`, `.rs` →
  `"rust"`.
- `automatic_resolution_payload(&Path)` — builds the `stderr` JSON for
  automatic provider selection.
- `provider_mismatch_payload(&Path, RequestedProvider)` — builds the `stderr`
  JSON for an explicit-provider mismatch refusal.
- `write_refactor_response(writer, Operation, arguments, renamed_symbol)` —
  orchestrates the full response sequence (optional `stderr` stream, `stdout`
  payload, exit record).
- `write_stdout_exit(writer, payload, status)` — emits the `stdout` stream
  record and the trailing exit envelope used by `FakeDaemon`, for example
  `{"kind":"stream","stream":"stdout","data":"..."}` followed by
  `{"kind":"exit","status":0}`.
- `response_payload_for_operation(Operation, renamed_symbol)` — returns the
  per-operation `stdout` JSON payload.

The `RequestedProvider` and `Operation` enums replace stringly-typed parameters
to reduce the risk of typos in test fixtures.

Typical routing flow inside the fake daemon:

```rust
let arguments = request_arguments(&parsed_request);
let file = argument_value(&arguments, "--file").expect("refactor requests need --file");
let payload = automatic_resolution_payload(std::path::Path::new(file));
```

### `refactor_helpers` (`weaverd/src/dispatch/act/refactor/refactor_helpers.rs`)

`refactor_helpers` is a `#[cfg(test)]` support module for the daemon-side
`act refactor` tests. It is split into small inline modules and then
re-exported at the top-level so sibling test modules can import a compact test
API instead of reaching into several implementation details.

The inline modules are:

- `builders` — request and backend constructors such as `command_request(...)`,
  `build_backends(...)`, `standard_rename_args_for_provider(...)`, and
  `configure_request(...)`.
- `resolutions` — pure constructors for capability-resolution envelopes,
  including `selected_resolution(...)`, `refused_resolution(...)`, and
  `rejected_candidate(...)`.
- `rollback` — runtime test doubles used by rollback-oriented tests.
- `content` — deterministic file-content and diff fixtures, including
  `original_content_for(...)`, `updated_content_for(...)`, and routed patch
  helpers such as `routed_diff_for(...)`.

`RollbackRuntime` and `ExecuteResult` model the two daemon interactions that
rollback tests need to control:

- `RollbackRuntime` implements `RefactorPluginRuntime`, so tests can inject a
  predetermined resolution result and a predetermined plugin execution result
  without spawning a real plugin.
- `ExecuteResult` distinguishes between a successful plugin response
  (`ExecuteResult::Success(PluginResponse)`) and a missing-plugin failure
  (`ExecuteResult::MissingPlugin(&'static str)`), which is enough to exercise
  the rollback and error-reporting paths in `handle(...)`.

The `rollback_tests` module uses these abstractions to assert failure-path
invariants for `act refactor`: the command exits with status `1`, the target
file content remains unchanged, and stderr contains the expected refusal or
runtime error text. That module is intentionally focused on rollback semantics,
while `tests.rs`, `contract_tests.rs`, and `behaviour.rs` cover other aspects
of the refactor handler.

Typical usage pattern in daemon tests:

```rust
let runtime = selected_runtime(
    SelectedResolution {
        capability: weaver_plugins::CapabilityId::RenameSymbol,
        language: "python",
        provider: "rope",
        selection_mode: super::resolution::SelectionMode::ExplicitProvider,
        requested_provider: Some("rope"),
    },
    ExecuteResult::MissingPlugin("rope"),
);

let request = command_request(standard_rename_args_for_provider("notes.py", "rope"));
let mut backends = build_backends(&socket_path);
```

That pattern lets a test build a request, inject a deterministic runtime, and
then call `handle(...)` to assert on exit status, stderr, and any preserved
workspace content. Tests that need fixture content or diff payloads layer in
the `content` helpers instead of hand-writing patch strings.

### `requirements` (`weaverd/src/dispatch/act/refactor/requirements.rs`)

`requirements` is the single source of truth for the operator-facing contract
of `act refactor`. It is a non-test module consumed by both the
argument-parsing layer and the test suite to keep validation, guidance text,
and supported-value lists in one place.

The module exposes seven `pub(crate)` functions. The exact signatures live in
Rustdoc; this section records each helper's contract, so the Markdown remains
readable after automated wrapping:

- `supported_provider_names() -> &'static [&'static str]` — returns the
  canonical slice of accepted provider names (e.g. `rope`, `rust-analyzer`),
  sourced from the built-in provider manifest catalogue.
- `supported_refactoring_names() -> &'static [&'static str]` — returns the
  canonical slice of accepted user-facing refactoring names (e.g. `rename`).
- `validate_provider(provider: &str) -> Result<(), DispatchError>` — delegates
  to `validate_value("provider", supported_provider_names(), provider)` and
  returns `DispatchError::InvalidArguments` with
  `act refactor does not support provider '<value>'` plus the shared guidance
  block when `provider` is not in `supported_provider_names()`.
- `validate_refactoring(refactoring: &str) -> Result<(), DispatchError>` —
  delegates to
  `validate_value("refactoring", supported_refactoring_names(), refactoring)`
  and returns `DispatchError::InvalidArguments` with
  `act refactor does not support refactoring '<value>'` plus the shared
  guidance block when `refactoring` is not in `supported_refactoring_names()`.
- `effective_operation(...)` — maps a user-facing refactoring name to the
  underlying plugin capability operation string (`"rename"` →
  `"rename-symbol"`), returning the same unsupported-refactoring
  `DispatchError::InvalidArguments` as `validate_refactoring(…)` for unknown
  user-facing names.
- `capability_for_operation(...)` — maps a capability operation string to its
  `CapabilityId` variant (`"rename-symbol"` → `CapabilityId::RenameSymbol`),
  returning `DispatchError::InvalidArguments` with
  `act refactor does not support capability resolution for '<operation>'` and
  the supported capability-operation tokens for unknown operations.
- `missing_requirements_error() -> DispatchError` — builds the deterministic
  `DispatchError::InvalidArguments` with `act refactor requires ...`, every
  required flag (`--provider <plugin>`, `--refactoring <operation>`,
  `--file <path>`), valid provider and refactoring values, and a next-command
  example derived from the first supported provider/refactoring or the
  `<plugin>` / `<operation>` placeholders. Called by the argument-builder when
  one or more required flags are absent.

## Dispatch lifecycle observability internals

This section documents the dispatch and startup-observability helpers added for
daemon request handling and CLI lifecycle guidance. The pieces are intentionally
small: dispatch owns request telemetry at the daemon boundary, while lifecycle
monitoring owns the runtime files that explain daemon readiness.

### `DispatchConnectionHandler` (`weaverd/src/dispatch/handler/`)

`crates/weaverd/src/dispatch/handler/` is split into three modules:

- `mod.rs` owns orchestration. It implements `DispatchConnectionHandler`,
  wires the transport `ConnectionHandler` trait, reads one JSONL request,
  validates it, routes it through `DomainRouter`, and writes response or exit
  records.
- `reader.rs` owns bounded JSONL reading. It reads from `ConnectionStream`,
  retries interrupted reads, preserves partial requests at EOF, and rejects
  lines above `JSONL_REQUEST_MAX_LINE_BYTES`.
- `structured_event.rs` owns structured dispatch event serialisation and
  emission. It builds JSON payloads, redacts request bodies, and sends events
  through tracing.

`DispatchConnectionHandler::new` takes four constructor arguments:

- `backends: BackendManager` gives routed commands access to shared daemon
  backends.
- `workspace_root: PathBuf` is passed into `DomainRouter::new` so request
  routing can resolve workspace-relative operations consistently.
- `endpoint: impl Into<String>` records the socket or TCP endpoint handling
  the request. Structured dispatch events include this value for operator
  correlation.
- `runtime_dir: PathBuf` records the daemon runtime directory. Structured
  dispatch events derive the `weaverd.health` health-snapshot path from it, so
  logs and CLI guidance point at the same runtime artefacts.

The connection flow is:

1. `handle` delegates to the synchronous dispatch path.
2. The receive-request path (`receive_request` in design discussion, implemented
   by `read_request`) reads bytes, parses `CommandRequest`, validates the
   command, and emits rejection events for read, parse, and validation failures.
3. Valid requests emit a `dispatching_request` event with domain, operation,
   endpoint, runtime directory, and request size metadata.
4. `route_request` calls `DomainRouter::route` through `BackendManager` and
   writes either the domain result status or a structured error response.

Event emission stays in the receive-request path because the handler still has
the raw request size, endpoint, runtime directory, and failure classification
at that point. Extracting emission away from that boundary would either pass a
large context object through parse and validation helpers, or lose the
distinction between client disconnects, read failures, malformed JSON, invalid
commands, and oversized requests.

### Structured dispatch events

`StructuredEventMetadata` carries optional domain, operation, size, and
maximum-size fields for structured dispatch logs. Its constructors and builder
methods keep event construction explicit:

- `none()` creates metadata with no domain or operation, used when the request
  was not parseable enough to identify a command.
- `new(domain, operation)` records the command target for routed or validated
  requests.
- `with_size(size)` records the observed request size.
- `with_max_size(max_size)` records the configured upper bound for size
  failures.
- `extend_payload(payload)` appends whichever metadata fields are present to a
  JSON object.

`StructuredDispatchEvent` contains the event name, endpoint, runtime directory,
metadata, and five optional sensitive request fields:

- `patch`
- `body`
- `source`
- `env`
- `full_payload`

`serialize_structured_event` always includes `event`, `endpoint`,
`runtime_dir`, and the derived `weaverd.health` path, then extends the payload
with metadata. If any sensitive field is present, the serializer writes the
field with the `"<redacted>"` marker rather than the original content. That
keeps structured logs useful for diagnosing dispatch failures without exposing
patch contents, request bodies, source text, environment values, or complete
payloads.

`emit_structured_event` formats the event as JSON and logs it to
`DISPATCH_TARGET`. It uses `tracing::error!` when `is_error` is true and
`tracing::info!` otherwise. Both paths include the event name, human message,
and serialized payload under the same target so downstream tracing filters can
collect dispatch telemetry consistently.

### `monitoring_readers` (`weaver-cli/src/lifecycle/monitoring_readers.rs`)

`monitoring_readers` centralizes runtime-file reads for daemon lifecycle
monitoring. It exists so health and PID files share the same missing-file,
empty-file, read-error, and parse-error semantics.

The `define_reader!` macro generates concrete typed readers from a small
contract: the return type, the `LifecycleError` variant for I/O failures, the
variant for parse failures, and the parser expression. The generated reader
trims file content, treats empty content as absent, and maps parse failures to
the configured semantic lifecycle error.

`read_optional_file` reads a file from the already-open runtime directory. It
returns `Ok(None)` for `NotFound`, because missing runtime files are expected
during daemon startup and shutdown. Other I/O errors are returned for the
caller to classify.

`read_and_parse` combines optional-file reading with parse handling. It maps
read errors through the caller-provided read-error constructor and delegates
non-empty content to the caller-provided parse closure.

The concrete readers have these return semantics:

- `read_health` returns `Ok(Some(HealthSnapshot))` when `weaverd.health` is
  present and valid JSON, `Ok(None)` when the file is missing or empty,
  `Err(LifecycleError::ReadHealth { .. })` when reading fails, and
  `Err(LifecycleError::ParseHealth { .. })` when JSON parsing fails.
- `read_pid` returns `Ok(Some(u32))` when the PID file is present and contains
  a valid integer, `Ok(None)` when it is missing or empty,
  `Err(LifecycleError::ReadPid { .. })` when reading fails, and
  `Err(LifecycleError::ParsePid { .. })` when integer parsing fails.

### `runtime_dir` in lifecycle errors

`LifecycleError::LaunchDaemon` and `LifecycleError::StartupFailed` both carry
`runtime_dir: PathBuf`. The CLI keeps this path with the error because startup
guidance is produced after the lower-level lifecycle operation has already
failed. Carrying the runtime directory in the error lets actionable guidance
derive the same `weaverd.health` path that monitoring uses and surface runtime
artefact inspection as a next diagnostic step.

For launch failures, guidance can tell the operator where to inspect runtime
artefacts even when the daemon process never starts. For startup failures,
guidance derives `runtime_dir.join("weaverd.health")` and points directly at
the health snapshot that should explain why readiness was not reached.
