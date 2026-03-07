# 7.1.1 Define stable JSON Lines (JSONL) request and response schemas for `observe get-card`

This ExecPlan (execution plan) is a living document. The sections
`Constraints`, `Tolerances`, `Risks`, `Progress`, `Surprises & Discoveries`,
`Decision Log`, and `Outcomes & Retrospective` must be kept up to date as work
proceeds.

Status: COMPLETE

## Purpose / big picture

After this change, a downstream consumer (whether a future handler in
`weaverd`, a test harness, or a documentation generator) can import a set of
stable, serde-annotated Rust types that describe every field, variant, and
version marker in the `observe get-card` request and response payloads. These
types lock down the JSON shapes described in
`docs/jacquard-card-first-symbol-graph-design.md` so that later tasks (7.1.2
Tree-sitter extraction, 7.1.3 LSP enrichment) implement against a well-defined
contract rather than inventing the schema ad hoc.

Observable outcome after all stages complete:

```plaintext
make check-fmt   # exits 0
make lint         # exits 0 (includes cargo doc --workspace --no-deps -D warnings)
make test         # exits 0, including all new weaver-cards and weaverd tests
make markdownlint # exits 0
make nixie        # exits 0
```

Specifically:

1. A new crate `weaver-cards` exists in `crates/weaver-cards/` and is
   registered as a workspace member. It exports types for `SymbolCard`,
   `SymbolRef`, `SymbolId`, `DetailLevel`, `GetCardRequest`, `GetCardResponse`
   (success and refusal variants), and all nested sub-structures.
2. Insta snapshot tests in `weaver-cards` lock the JSON shape of a fully
   populated success card, a minimal card, and a refusal payload. These
   snapshots are byte-identical across runs for unchanged inputs.
3. Behaviour-driven development (BDD) feature scenarios in
   `weaver-cards` exercise the request parsing and response construction via
   `rstest-bdd` v0.5.0.
4. `weaverd` adds `"get-card"` to the
   `DomainRoutingContext::OBSERVE.known_operations` list so that
   `observe get-card` is recognized by the router.
5. Because no Tree-sitter extraction exists yet (that is 7.1.2), the handler
   in `weaverd` returns a structured refusal response (a
   `GetCardResponse::Refusal` variant) rather than a bare "not yet implemented"
   string. This exercises the schema types in the dispatch path and produces a
   JSON payload that tells the caller exactly why no card was produced.
6. `docs/users-guide.md` is updated with `observe get-card` command
   documentation including syntax, arguments, and response format.
7. Roadmap item 7.1.1 in `docs/roadmap.md` is marked complete.

This satisfies roadmap task 7.1.1 from `docs/roadmap.md`[^1] and closes #75.

## Constraints

- `make check-fmt`, `make lint`, `make test`, `make markdownlint`, and
  `make nixie` must pass after all changes. These are defined in `Makefile`
  (lines 19-35).
- The workspace uses `edition = "2024"` and `rust-version = "1.88"`
  (`Cargo.toml` lines 20-23). The new crate must inherit these via
  `edition.workspace = true`, `version.workspace = true`, and
  `rust-version.workspace = true`.
- The new crate must include `[lints] workspace = true` to inherit the strict
  lint configuration from `Cargo.toml` lines 53-127. Critical denied lints
  include: `unwrap_used`, `expect_used`, `indexing_slicing`, `string_slice`,
  `str_to_string`, `allow_attributes`, `missing_const_for_fn`,
  `must_use_candidate`, `missing_docs`, `print_stdout`, `print_stderr`,
  `panic_in_result_fn`, `shadow_reuse`, `shadow_same`, `shadow_unrelated`,
  `cognitive_complexity`.
- No panicking in library code. Stub methods must return structured errors or
  refusal payloads. Never use `todo!()`, `unimplemented!()`, or `panic!()`.
- No single source file may exceed 400 lines (`AGENTS.md` line 31).
- Every module must begin with a `//!` doc comment (`AGENTS.md` line 154).
- All public items must have `///` rustdoc comments (`AGENTS.md` line 156).
- Comments and documentation must use en-GB-oxendict spelling
  ("-ize" / "-yse" / "-our") (`AGENTS.md` line 24).
- `#[non_exhaustive]` must be used on public enums.
- Library crates use `thiserror`-derived error enums (`AGENTS.md`
  lines 220-227).
- All dependency versions use caret requirements (`AGENTS.md`
  lines 206-216).
- No new external dependencies beyond those already in
  `[workspace.dependencies]`. The needed dependencies (`serde`, `serde_json`,
  `thiserror`, `rstest`, `rstest-bdd`, `rstest-bdd-macros`, `insta`) are all
  already workspace dependencies.
- Existing crate public APIs must not change.
- Provenance timestamps use `String` (ISO 8601 format) rather than
  introducing `chrono` or `time`, since neither is a workspace dependency.

## Tolerances (exception triggers)

- Scope: if implementation requires changes to more than 25 files (net), stop
  and escalate. (Raised from 15 because new-crate scaffolding creates many
  small files plus insta snapshot files.)
- Interface: if any existing `pub` API signature in any existing crate must
  change (beyond adding `"get-card"` to the known operations list and adding a
  new module in `dispatch/observe/`), stop and escalate.
- Dependencies: if a new external dependency beyond those already in
  `[workspace.dependencies]` is required, stop and escalate.
- Iterations: if tests still fail after 5 attempts at fixing a given issue,
  stop and escalate.
- Ambiguity: if the design document is ambiguous on a type definition and the
  choice materially affects the public API, stop and present options.

## Risks

- Risk: `result_large_err = "deny"` lint may fire if `GetCardResponse` or
  `SymbolCard` is large as a `Result` error type. Severity: low. Likelihood:
  low. Mitigation: `GetCardResponse` is not used as an error type; it is
  serialized directly. The card types themselves are serialized to JSON
  strings, not returned as `Err(...)`.

- Risk: `missing_const_for_fn = "deny"` fires on constructors taking
  `String`, `Vec`, or `Option<String>`. Severity: low. Likelihood: high.
  Mitigation: Use `#[expect(clippy::missing_const_for_fn, reason = "...")]` on
  such constructors. This is the established pattern from `sempai-core`.

- Risk: The 400-line file limit may be challenged by the number of struct
  definitions. Severity: medium. Likelihood: medium. Mitigation: Split the
  types across multiple modules (`symbol.rs`, `card.rs`, `request.rs`,
  `response.rs`, `detail.rs`, `error.rs`).

- Risk: `str_to_string = "deny"` fires on `.to_string()` called on `&str`.
  Severity: low. Likelihood: high. Mitigation: Use `String::from(...)` or
  `.into()` consistently.

## Progress

- [x] Stage A: Scaffold `weaver-cards` crate with workspace registration.
- [x] Stage B: Define core schema types (`SymbolRef`, `SymbolId`,
  `DetailLevel`, `CardLanguage`, `CardSymbolKind`).
- [x] Stage C: Define `SymbolCard` and sub-structures (`SignatureInfo`,
  `DocInfo`, `StructureInfo`, `LspInfo`, `MetricsInfo`, `DepsInfo`,
  `Provenance`).
- [x] Stage D: Define `GetCardRequest`, `GetCardResponse`, refusal types,
  and `GetCardError`.
- [x] Stage E: Add insta snapshot tests locking JSON shapes.
- [x] Stage F: Add BDD feature file and behaviour tests.
- [x] Stage G: Wire `weaverd` dispatch — add `"get-card"` to known
  operations, add `get_card` handler module returning structured refusal.
- [x] Stage H: Update documentation (`docs/users-guide.md`,
  `docs/repository-layout.md`, `docs/roadmap.md`).
- [x] Stage I: Final validation and commit gating.

## Surprises & discoveries

- `CardLanguage::TypeScript` with `#[serde(rename_all = "snake_case")]`
  serialized as `"type_script"` instead of the design document's
  `"typescript"`. Fixed by adding an explicit `#[serde(rename = "typescript")]`
  on the `TypeScript` variant.

- `cargo-insta` command-line interface (CLI) was not installed
  in the environment. Snapshot tests failed on first run because no snapshot
  files existed. Resolved by running with `INSTA_UPDATE=always` environment
  variable to auto-accept new snapshots.

- The `Cargo.toml` edit tool requires a fresh `Read` call before each
  `Edit`. An initial attempt to edit the workspace `Cargo.toml` failed due to a
  stale read cache. Re-reading before editing resolved it.

## Decision log

- Decision: Place schema types in a new `weaver-cards` crate rather than
  inside `weaverd`. Rationale: The design document
  (`docs/jacquard-card-first-symbol-graph-design.md` lines 709-727) recommends
  Option A (new crates) for better testability and reduced daemon coupling. The
  workspace already follows this pattern with `weaver-graph`, `weaver-plugins`,
  `sempai-core`, etc.

- Decision: Use `String` for timestamps rather than `time::OffsetDateTime`.
  Rationale: Neither `chrono` nor `time` is a workspace dependency. Adding one
  solely for a schema definition crate is unnecessary. The `extracted_at` field
  is serialized as an ISO 8601 string.

- Decision: Model progressive detail levels via `Option` fields on a single
  `SymbolCard` struct rather than separate structs per detail level. Rationale:
  The design document defines detail levels as additive layers (`minimal` is a
  subset of `signature` which is a subset of `structure`, etc.). Using `Option`
  fields with `#[serde(skip_serializing_if = "Option::is_none")]` naturally
  produces the right JSON shape for each level. A single struct with optional
  sections is simpler than maintaining five separate structs with overlapping
  fields.

- Decision: Use internally-tagged enum `#[serde(tag = "status")]` for
  `GetCardResponse`. Rationale: The response has struct-like variants
  (`Success { card }` and `Refusal { refusal }`), which are compatible with
  internally-tagged serde. This produces a clean JSON shape where
  `"status": "success"` or `"status": "refusal"` appears at the top level
  alongside the payload fields.

- Decision: The `get-card` handler in `weaverd` returns a structured refusal
  rather than the existing `route_fallback` text-based "not yet implemented"
  message. Rationale: The acceptance criteria state "schema fixtures lock field
  names and payload shapes". Returning a typed refusal exercises the schema in
  the dispatch path and provides a richer signal to callers.

## Outcomes & retrospective

All stages completed successfully. The `weaver-cards` crate exports 25 public
types covering the full `observe get-card` schema. Six insta snapshots lock the
JSON shapes for minimal, structure, and full detail cards, plus refusal and
success response envelopes. Five BDD scenarios validate schema contracts. The
`weaverd` router recognizes `observe get-card` and dispatches to a handler that
returns a structured `GetCardResponse::Refusal` with reason
`not_yet_implemented`. All quality gates (`make check-fmt`, `make lint`,
`make test`, `make markdownlint`, `make nixie`) pass. Documentation updated in
`users-guide.md`, `repository-layout.md`, and `roadmap.md`.

Key learnings:

- Serde `rename_all = "snake_case"` splits on camelCase boundaries, so
  `TypeScript` becomes `type_script`. Use explicit `#[serde(rename = ...)]` for
  compound words that should not be split.
- The `INSTA_UPDATE=always` environment variable is a practical alternative
  to the `cargo-insta` CLI for accepting new snapshots in environments where
  the CLI is not installed.
- Splitting types across 6 modules kept every file well under the 400-line
  limit (largest was ~230 lines for `card.rs`).

## Context and orientation

### Repository structure

Weaver is a Rust workspace rooted at `./`. The workspace has 14 crates in
`crates/`. The main daemon is `crates/weaverd/`. The CLI is
`crates/weaver-cli/`. Domain-specific logic lives in separate crates:
`weaver-graph` (call graphs), `weaver-plugins` (plugin orchestration),
`weaver-syntax` (Tree-sitter), `weaver-lsp-host` (LSP management),
`sempai-core` and `sempai` (query engine).

The workspace `Cargo.toml` at `Cargo.toml` lists all members (lines 2-17) and
defines shared dependencies (lines 25-51) and lint rules (lines 53-127).

### JSONL dispatch architecture

Clients send JSONL requests to the daemon. The request shape is:

```json
{"command":{"domain":"observe","operation":"get-card"},"arguments":["--uri","file:///foo.rs","--position","10:5"]}
```

The daemon responds with `Stream` messages followed by a terminal `Exit`
message:

```json
{"kind":"stream","stream":"stdout","data":"{...card JSON...}"}
{"kind":"exit","status":0}
```

Key files:

- Request types: `crates/weaverd/src/dispatch/request.rs`
- Response writer: `crates/weaverd/src/dispatch/response.rs`
- Router: `crates/weaverd/src/dispatch/router.rs` (lines 89-98 define known
  observe operations; lines 189-199 define `route_observe()`)
- Observe handlers: `crates/weaverd/src/dispatch/observe/` (contains
  `arguments.rs`, `get_definition.rs`, `responses.rs`)

### Design document schema

The card schema is specified in
`docs/jacquard-card-first-symbol-graph-design.md`:

- Lines 138-234: Data model (`SymbolRef`, `SymbolId`, `SymbolCard` JSON)
- Lines 288-327: Progressive detail levels
  (`minimal`/`signature`/`structure`/`semantic`/`full`)
- Lines 601-614: `observe get-card` CLI arguments and response shape

### Key lint gotchas (from project experience)

- `indexing_slicing = "deny"` — use `.first()`, `.get()`, not `[0]`.
- `str_to_string = "deny"` — use `String::from()` or `.into()`.
- `missing_const_for_fn = "deny"` — use `#[expect(...)]` on constructors
  with heap types.
- `allow_attributes = "deny"` — use `#[expect(..., reason = "...")]`.
- `no_effect_underscore_binding = "deny"` — use `let _ = world;` not
  `let _world = world;`.
- `rstest-bdd` fixture matching is by parameter name (`world` not `_world`).
- `string_slice = "deny"` — use `split`/`split_once` instead of string
  indexing.
- `non_exhaustive` required on all public enums.

## Plan of work

### Stage A: Scaffold `weaver-cards` crate

Create the crate directory and register it in the workspace.

**New file: `crates/weaver-cards/Cargo.toml`**

```toml
[package]
name = "weaver-cards"
edition.workspace = true
version.workspace = true
rust-version.workspace = true

[dependencies]
serde = { workspace = true }
thiserror = { workspace = true }

[dev-dependencies]
rstest = { workspace = true }
rstest-bdd = { workspace = true }
rstest-bdd-macros = { workspace = true }
insta = { workspace = true }
serde_json = { workspace = true }

[lints]
workspace = true
```

**New file: `crates/weaver-cards/src/lib.rs`**

Crate-level `//!` doc comment explaining the crate's purpose. Declares
submodules (`symbol`, `detail`, `card`, `request`, `response`, `error`).
Re-exports all public types from `lib.rs`.

**Modify: `Cargo.toml` (workspace root)**

Add `"crates/weaver-cards"` to the `[workspace.members]` list.

**Validation:** `cargo check -p weaver-cards` succeeds.

### Stage B: Define core schema types

**New file: `crates/weaver-cards/src/symbol.rs`** (~130 lines)

Types mapped from the design doc lines 147-170:

- `SourcePosition` — `{ line: u32, column: u32 }` (zero-indexed, matching
  Tree-sitter and LSP internal representation)
- `SourceRange` — `{ start: SourcePosition, end: SourcePosition }`
- `CardSymbolKind` — `#[non_exhaustive]` enum with `#[serde(rename_all =
  "snake_case")]`: `Function`, `Method`, `Class`, `Interface`, `Type`, `Variable
   `, `Module`, `Field`
- `CardLanguage` — `#[non_exhaustive]` enum: `Rust`, `Python`, `TypeScript`
- `SymbolRef` — location-based reference: `uri`, `range`, `language`,
  `kind`, `name`, `container` (Option, skip_serializing_if is_none)
- `SymbolId` — `{ symbol_id: String }`
- `SymbolIdentity` — `{ symbol_id: String, #[serde(rename = "ref")]
  symbol_ref: SymbolRef }`

**New file: `crates/weaver-cards/src/detail.rs`** (~55 lines)

- `DetailLevel` — `#[non_exhaustive]` enum with `#[serde(rename_all =
  "snake_case")]`: `Minimal`, `Signature`, `Structure` (default), `Semantic`, `
  Full`. Implements `Default` returning `Structure`.

**Validation:** `cargo check -p weaver-cards` succeeds.

### Stage C: Define `SymbolCard` and sub-structures

**New file: `crates/weaver-cards/src/card.rs`** (~310 lines)

Types mapped from design doc lines 176-268:

- `ParamInfo` — `{ name: String, #[serde(rename = "type")] type_annotation:
  String }`
- `SignatureInfo` — `{ display: String, params: Vec<ParamInfo>, returns:
  String }`
- `DocInfo` — `{ docstring: String, summary: String, source: String }`
- `NormalizedAttachments` — `{ decorators: Vec<String> }`
- `AttachmentsInfo` — `{ doc_comments: Vec<String>, decorators:
  Vec<String>, normalized: NormalizedAttachments, bundle_rule: String
  }` (present at `structure` detail and above)
- `LocalInfo` — `{ name: String, kind: String, decl_line: u32 }`
- `BranchInfo` — `{ kind: String, line: u32 }`
- `StructureInfo` — `{ locals: Vec<LocalInfo>, branches: Vec<BranchInfo> }`
- `LspInfo` — `{ hover: String, #[serde(rename = "type")] type_info:
  String, deprecated: bool, source: String }`
- `MetricsInfo` — `{ lines: u32, cyclomatic: u32, fan_in: Option<u32>,
  fan_out: Option<u32>
  }` (fan metrics are `Option` because they are only populated at `full
  ` detail from the relational graph layer)
- `DepsInfo` — `{ calls: Vec<String>, imports: Vec<String>, config:
  Vec<String> }`
- `ImportInterstitialInfo` — `{ raw: String, normalized: Vec<String>,
  groups: Vec<Vec<String>>, source: String
  }` (import block data for file/module or interstitial cards)
- `InterstitialInfo` — `{ imports: ImportInterstitialInfo }` (present on
  file/module and interstitial cards only)
- `Provenance` — `{ extracted_at: String, sources: Vec<String> }`
- `SymbolCard` — top-level struct with `card_version: u32`,
  `symbol: SymbolIdentity`, and all other sections as `Option<T>`. Apply
  `#[serde(skip_serializing_if = "Option::is_none")]` to the optional sections.
  The `provenance` field is not optional (always present). The `etag` field is
  `Option<String>`.

All types derive `Debug, Clone, PartialEq, Eq, Serialize, Deserialize`.

**Validation:** `cargo check -p weaver-cards` succeeds.

### Stage D: Define request, response, and error types

**New file: `crates/weaver-cards/src/request.rs`** (~110 lines)

- `GetCardRequest` — `{ uri: String, line: u32, column: u32, detail:
  DetailLevel }`. The `parse(arguments: &[String]) -> Result<Self,
  GetCardError>` method follows the pattern from `
  crates/weaverd/src/dispatch/observe/arguments.rs`:
  - Iterates with a peekable iterator
  - Recognizes `--uri`, `--position`, `--detail`, `--format`
  - `--position` parsed with `split_once(':')` (not indexing)
  - `--detail` matched against known variant names
  - `--format` accepted but only `"json"` supported
  - Unknown `--` prefixed flags are silently skipped for forward
    compatibility; non-flag positional tokens produce
    `GetCardError::UnknownArgument`
  - Missing `--uri`/`--position` produce `GetCardError::MissingArgument`
  - Line and column must be >= 1 (1-indexed user-facing)

**New file: `crates/weaver-cards/src/response.rs`** (~100 lines)

- `RefusalReason` — `#[non_exhaustive]` enum with `#[serde(rename_all =
  "snake_case")]`: `NoSymbolAtPosition`, `UnsupportedLanguage`, `
  NotYetImplemented`, `BackendUnavailable`
- `CardRefusal` — `{ reason: RefusalReason, message: String,
  requested_detail: DetailLevel }`
- `GetCardResponse` — `#[serde(tag = "status", rename_all = "snake_case")]
  #[non_exhaustive]` enum: `Success { card: SymbolCard }`, `Refusal {
  refusal: CardRefusal }`
- `GetCardResponse::not_yet_implemented(detail: DetailLevel) -> Self`
  convenience constructor

**New file: `crates/weaver-cards/src/error.rs`** (~60 lines)

- `GetCardError` — `#[non_exhaustive] #[derive(thiserror::Error)]`:
  `MissingArgument { flag: String }`,
  `InvalidValue { flag: String, message: String }`,
  `UnknownArgument { argument: String }`

**Validation:** `cargo check -p weaver-cards` succeeds. Basic serde round-trip
tests pass.

### Stage E: Add insta snapshot tests

**New file: `crates/weaver-cards/src/tests/mod.rs`**

Module declarations for `snapshot_tests`, `round_trip_tests`, and `behaviour`.

**New file: `crates/weaver-cards/src/tests/snapshot_tests.rs`** (~180 lines)

Fixture builders construct example payloads and snapshot via
`insta::assert_snapshot!`:

1. `snapshot_minimal_card` — card at `minimal` detail (only `symbol` and
   `provenance` populated; all other fields `None` and absent from JSON).
2. `snapshot_structure_card` — card at `structure` detail (includes
   `signature`, `doc`, `structure`, `metrics` without `fan_in`/`fan_out`;
   `lsp`, `deps`, `etag` absent).
3. `snapshot_full_card` — card at `full` detail with all fields populated.
4. `snapshot_refusal_not_implemented` — `GetCardResponse::Refusal` with
   `NotYetImplemented` reason.
5. `snapshot_refusal_no_symbol` — `GetCardResponse::Refusal` with
   `NoSymbolAtPosition` reason.
6. `snapshot_success_response` — `GetCardResponse::Success` wrapping a
   structure-level card.

Each test serializes to JSON via `serde_json::to_string_pretty` and snapshots
the result. Snapshot files auto-generated in
`crates/weaver-cards/src/tests/snapshots/`.

**New file: `crates/weaver-cards/src/tests/round_trip_tests.rs`** (~80 lines)

Tests serialize and deserialize each type to confirm serde compatibility. Uses
`rstest` parameterization for detail levels. Verifies byte-identical
re-serialization.

**Validation:** `cargo test -p weaver-cards` passes. Snapshot files created.

### Stage F: Add BDD feature file and behaviour tests

**New file: `crates/weaver-cards/tests/features/get_card_schema.feature`** (~45
lines)

```gherkin
Feature: Symbol card schema contracts

  Scenario: Minimal detail card omits optional sections
    Given a symbol card at "minimal" detail level
    When the card is serialized to JSON
    Then the JSON contains a "card_version" field
    And the JSON contains a "symbol" field
    And the JSON contains a "provenance" field
    And the JSON does not contain a "signature" field
    And the JSON does not contain a "lsp" field

  Scenario: Structure detail card includes signature and doc
    Given a symbol card at "structure" detail level
    When the card is serialized to JSON
    Then the JSON contains a "signature" field
    And the JSON contains a "doc" field
    And the JSON contains a "structure" field
    And the JSON contains a "metrics" field

  Scenario: Refusal response includes reason code
    Given a refusal response with reason "not_yet_implemented"
    When the response is serialized to JSON
    Then the JSON contains "status" with value "refusal"
    And the JSON contains a "refusal" field
    And the refusal contains "reason" with value "not_yet_implemented"

  Scenario: Success response wraps a card
    Given a success response with a "structure" detail card
    When the response is serialized to JSON
    Then the JSON contains "status" with value "success"
    And the JSON contains a "card" field

  Scenario: Default detail level is structure
    Given a get-card request with no detail flag
    Then the detail level is "structure"
```

**New file: `crates/weaver-cards/src/tests/behaviour.rs`** (~200 lines)

BDD step implementations using `rstest-bdd-macros`, following the pattern from
`crates/weaver-plugins/src/tests/behaviour.rs`:

- `TestWorld` struct holding the current card, response, serialized JSON,
  and request.
- `#[given]`, `#[when]`, `#[then]` step functions with parameter parsing.
- Fixture builders for cards at each detail level and for refusal/success
  responses.
- `world` parameter named `world` (not `_world`); unused suppressed with
  `let _ = world;`.

**Validation:** `cargo test -p weaver-cards` passes. All BDD scenarios green.

### Stage G: Wire `weaverd` dispatch

**Modify: `crates/weaverd/src/dispatch/router.rs`**

1. Add `"get-card"` to the `DomainRoutingContext::OBSERVE.known_operations`
   array (line 91). The array becomes:

   ```rust
   known_operations: &[
       "get-definition",
       "find-references",
       "grep",
       "diagnostics",
       "call-hierarchy",
       "get-card",
   ],
   ```

2. Add a match arm in `route_observe()` (lines 196-198):

   ```rust
   "get-card" => observe::get_card::handle(request, writer),
   ```

   The `get-card` handler does NOT take `backends` because it does not start
   any backend. It returns a structured refusal.

**New file: `crates/weaverd/src/dispatch/observe/get_card.rs`** (~65 lines)

Handler module that:

1. Parses `GetCardRequest::parse(&request.arguments)`, mapping
   `GetCardError` to `DispatchError::invalid_arguments(...)`.
2. Constructs a `GetCardResponse::not_yet_implemented(request.detail)`
   refusal.
3. Serializes the response to JSON via `serde_json::to_string`.
4. Writes the JSON to `writer.write_stdout()`.
5. Returns `Ok(DispatchResult::with_status(1))`.

**Modify: `crates/weaverd/src/dispatch/observe/mod.rs`**

Add `pub mod get_card;` declaration.

**Modify: `crates/weaverd/Cargo.toml`**

Add `weaver-cards = { path = "../weaver-cards" }` to `[dependencies]`.

**Modify: `crates/weaverd/src/dispatch/router/tests.rs`**

Add a case to `invalid_arguments_message()` (line 37-50):

```rust
("observe", "get-card") => {
    Some("observe get-card should fail with InvalidArguments (no args)")
}
```

**Validation:** `cargo test -p weaverd -- router` passes. The `get-card`
operation is recognized and returns a structured refusal.

### Stage H: Update documentation

**Modify: `docs/users-guide.md`**

Add an `#### observe get-card` section after the existing
`observe call-hierarchy` section (around line 440). Content:

- Syntax: `weaver observe get-card --uri <URI> --position <LINE:COL>
  [--detail <LEVEL>]`
- Arguments: `--uri` (required), `--position` (required, 1-indexed),
  `--detail` (optional, one of `minimal`/`signature`/`structure`
  (default)/`semantic`/`full`)
- Response format: JSON object with `"status": "success"` wrapping a card,
  or `"status": "refusal"` wrapping a refusal with `reason` and `message`
- Note that Tree-sitter extraction is not yet implemented; the operation
  currently returns a structured refusal

**Modify: `docs/repository-layout.md`**

Add `weaver-cards/` to the crate listing in the `crates/` tree (line 31-45).

**Modify: `docs/roadmap.md`**

Mark 7.1.1 as complete: change `- [ ]` to `- [x]` on lines 788-795 (the main
task and both sub-tasks).

**Write: `docs/execplans/7-1-1-stable-jsonl-schemas-for-observe-get-card.md`**

Copy the finalized ExecPlan to the execplans directory.

**Validation:** `make fmt` passes. `make markdownlint` passes. `make nixie`
passes.

### Stage I: Final validation and commit gating

Run all quality gates:

```plaintext
set -o pipefail
make check-fmt 2>&1 | tee /tmp/check-fmt.log
make lint 2>&1 | tee /tmp/lint.log
make test 2>&1 | tee /tmp/test.log
make markdownlint 2>&1 | tee /tmp/markdownlint.log
make nixie 2>&1 | tee /tmp/nixie.log
```

All five must exit 0. Additionally verify:

- `cargo doc -p weaver-cards --no-deps` produces zero warnings.
- Insta snapshot files exist and are committed.
- The `observe get-card` operation is recognized by the router.

## Concrete steps

All commands run from the repository root.

### Stage A verification

```plaintext
cargo check -p weaver-cards
```

Expected: compiles with zero errors or warnings.

### Stages B-D verification

```plaintext
cargo test -p weaver-cards --lib
```

Expected: all unit tests pass.

### Stage E verification

```plaintext
cargo test -p weaver-cards -- snapshot
```

Expected: snapshot tests pass. On first run, `insta` creates snapshot files.

### Stage F verification

```plaintext
cargo test -p weaver-cards -- behaviour
```

Expected: BDD scenario tests pass.

### Stage G verification

```plaintext
cargo test -p weaverd -- router
```

Expected: all router tests pass, including `"get-card"` recognition.

### Stage I full validation

```plaintext
set -o pipefail
make check-fmt 2>&1 | tee /tmp/check-fmt.log
make lint 2>&1 | tee /tmp/lint.log
make test 2>&1 | tee /tmp/test.log
make markdownlint 2>&1 | tee /tmp/markdownlint.log
make nixie 2>&1 | tee /tmp/nixie.log
```

Expected: all five exit 0.

## Validation and acceptance

Quality criteria (what "done" means):

- Tests: `make test` passes. All new snapshot tests, unit tests, and BDD
  scenarios are green. Snapshot files are committed.
- Lint/typecheck: `make lint` passes. `cargo doc -p weaver-cards --no-deps`
  produces zero warnings.
- Formatting: `make check-fmt` passes.
- Documentation: `make markdownlint` and `make nixie` pass.
- Schema stability: The JSON output for each detail level is deterministic.
  Repeated serialization of the same fixture produces byte-identical output.
- Refusal payload: Dispatching `observe get-card --uri file:///foo.rs
  --position
  10:5` returns a JSONL response containing a `GetCardResponse::Refusal
  ` with reason `"not_yet_implemented"` and status 1.
- Field names: JSON field names match the design document
  (`card_version`, `symbol`, `signature`, `doc`, `structure`, `lsp`, `metrics`,
  `deps`, `provenance`, `etag`).
- Provenance: every non-trivial field section includes a `source` field.
- User guide: `docs/users-guide.md` documents the `observe get-card`
  command.
- Roadmap: 7.1.1 is marked complete in `docs/roadmap.md`.

Quality method (how checks are performed):

- Run
  `make check-fmt && make lint && make test && make markdownlint && make nixie`
  and confirm exit 0.
- Inspect snapshot files in `crates/weaver-cards/src/tests/snapshots/` to
  verify JSON shapes match the design document.

## Idempotence and recovery

All stages are additive. Re-running any stage produces the same result. If a
stage fails partway through, fix the issue and re-run. No destructive
operations are involved. If insta snapshots need updating after a deliberate
schema change, run `cargo insta review` to accept the new snapshots.

## Artifacts and notes

### Expected JSON shape: minimal card

```json
{
  "card_version": 1,
  "symbol": {
    "symbol_id": "sym_abc123",
    "ref": {
      "uri": "file:///src/main.rs",
      "range": {
        "start": { "line": 10, "column": 0 },
        "end": { "line": 42, "column": 1 }
      },
      "language": "rust",
      "kind": "function",
      "name": "process_request"
    }
  },
  "provenance": {
    "extracted_at": "2026-03-03T12:34:56Z",
    "sources": ["tree_sitter"]
  }
}
```

### Expected JSON shape: refusal

```json
{
  "status": "refusal",
  "refusal": {
    "reason": "not_yet_implemented",
    "message": "observe get-card: Tree-sitter card extraction is not yet implemented",
    "requested_detail": "structure"
  }
}
```

### Expected JSON shape: success response at structure detail

```json
{
  "status": "success",
  "card": {
    "card_version": 1,
    "symbol": {
      "symbol_id": "sym_abc123",
      "ref": {
        "uri": "file:///src/main.rs",
        "range": {
          "start": { "line": 10, "column": 0 },
          "end": { "line": 42, "column": 1 }
        },
        "language": "rust",
        "kind": "function",
        "name": "process_request",
        "container": "handlers"
      }
    },
    "signature": {
      "display": "fn process_request(req: &Request) -> Response",
      "params": [
        { "name": "req", "type": "&Request" }
      ],
      "returns": "Response"
    },
    "doc": {
      "docstring": "Processes an incoming request and returns a response.",
      "summary": "Processes an incoming request and returns a response.",
      "source": "tree_sitter"
    },
    "structure": {
      "locals": [
        { "name": "result", "kind": "variable", "decl_line": 15 }
      ],
      "branches": [
        { "kind": "if", "line": 18 },
        { "kind": "match", "line": 25 }
      ]
    },
    "metrics": {
      "lines": 33,
      "cyclomatic": 5
    },
    "provenance": {
      "extracted_at": "2026-03-03T12:34:56Z",
      "sources": ["tree_sitter"]
    }
  }
}
```

Note: `lsp`, `deps`, `etag`, and `fan_in`/`fan_out` in `metrics` are absent
because they are `None` at `structure` detail level.

## Interfaces and dependencies

### New crate: `weaver-cards`

Dependencies (all from `[workspace.dependencies]`):

- `serde` (with `derive` feature) — serialization
- `thiserror` — error types

Dev-dependencies (all from `[workspace.dependencies]`):

- `rstest` — parameterized tests
- `rstest-bdd` — BDD framework
- `rstest-bdd-macros` — BDD macros
- `insta` — snapshot testing
- `serde_json` — JSON serialization in tests

Public API surface (types exported from `lib.rs`):

From `symbol.rs`: `SourcePosition`, `SourceRange`, `CardSymbolKind`,
`CardLanguage`, `SymbolRef`, `SymbolId`, `SymbolIdentity`

From `detail.rs`: `DetailLevel`

From `card.rs`: `SymbolCard`, `SignatureInfo`, `ParamInfo`, `DocInfo`,
`LocalInfo`, `BranchInfo`, `StructureInfo`, `LspInfo`, `MetricsInfo`,
`DepsInfo`, `Provenance`

From `request.rs`: `GetCardRequest`

From `response.rs`: `GetCardResponse`, `CardRefusal`, `RefusalReason`

From `error.rs`: `GetCardError`

### Modified crate: `weaverd`

New dependency: `weaver-cards = { path = "../weaver-cards" }`

New module: `crates/weaverd/src/dispatch/observe/get_card.rs`

Modified files:

- `crates/weaverd/src/dispatch/router.rs` — add `"get-card"` to known ops
  and match arm
- `crates/weaverd/src/dispatch/observe/mod.rs` — add `pub mod get_card;`
- `crates/weaverd/Cargo.toml` — add dependency
- `crates/weaverd/src/dispatch/router/tests.rs` — extend
  `invalid_arguments_message`

### File inventory

New files (~15):

1. `crates/weaver-cards/Cargo.toml`
2. `crates/weaver-cards/src/lib.rs`
3. `crates/weaver-cards/src/symbol.rs`
4. `crates/weaver-cards/src/detail.rs`
5. `crates/weaver-cards/src/card.rs`
6. `crates/weaver-cards/src/request.rs`
7. `crates/weaver-cards/src/response.rs`
8. `crates/weaver-cards/src/error.rs`
9. `crates/weaver-cards/src/tests/mod.rs`
10. `crates/weaver-cards/src/tests/snapshot_tests.rs`
11. `crates/weaver-cards/src/tests/round_trip_tests.rs`
12. `crates/weaver-cards/src/tests/behaviour.rs`
13. `crates/weaver-cards/tests/features/get_card_schema.feature`
14. `crates/weaverd/src/dispatch/observe/get_card.rs`
15. `docs/execplans/7-1-1-stable-jsonl-schemas-for-observe-get-card.md`

Plus insta snapshot files auto-generated in
`crates/weaver-cards/src/tests/snapshots/`.

Modified files (8):

1. `Cargo.toml` (workspace root) — add member
2. `crates/weaverd/Cargo.toml` — add dependency
3. `crates/weaverd/src/dispatch/router.rs` — add known op and match arm
4. `crates/weaverd/src/dispatch/observe/mod.rs` — add module declaration
5. `crates/weaverd/src/dispatch/router/tests.rs` — extend helper
6. `docs/roadmap.md` — mark 7.1.1 complete
7. `docs/repository-layout.md` — add crate listing
8. `docs/users-guide.md` — add `observe get-card` documentation

Total: ~23 file touches (15 new + 8 modified) plus auto-generated snapshot
files. Within the 25-file tolerance.

### Critical reference files

- `docs/jacquard-card-first-symbol-graph-design.md` — source of truth for
  the `SymbolCard` JSON schema, detail levels, and command surface (lines
  138-614)
- `crates/weaverd/src/dispatch/router.rs` — router where `"get-card"` must
  be added (lines 89-199)
- `crates/weaverd/src/dispatch/observe/arguments.rs` — pattern to follow
  for argument parsing (`GetDefinitionArgs::parse`)
- `crates/sempai-core/src/lib.rs` — pattern to follow for new crate
  scaffolding (module structure, re-exports, lint compliance)
- `crates/weaverd/src/dispatch/observe/get_definition.rs` — pattern to
  follow for handler module structure (parse args, serialize response, return
  `DispatchResult`)
- `crates/weaverd/src/dispatch/router/tests.rs` — existing router test
  structure to extend

[^1]: `docs/roadmap.md`, lines 788–795
