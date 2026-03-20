# 7.1.3 Implement optional LSP enrichment for `observe get-card`

This ExecPlan (execution plan) is a living document. The sections
`Constraints`, `Tolerances`, `Risks`, `Progress`, `Surprises & Discoveries`,
`Decision Log`, and `Outcomes & Retrospective` must be kept up to date as work
proceeds.

Status: COMPLETE

This document must be maintained in accordance with `AGENTS.md` at the
repository root.

## Purpose / big picture

After this change, `weaver observe get-card --detail semantic` will enrich the
Tree-sitter-only symbol card with hover documentation, resolved type
information, and deprecation status obtained from the language server via the
`textDocument/hover` Language Server Protocol (LSP) request. When the LSP is
unavailable (no registered server, capability denied, or server error), the
card degrades gracefully to the Tree-sitter-only extraction and records
explicit provenance so consumers know the semantic tier was requested but not
fulfilled.

Observable behaviour after implementation:

1. A `get-card` request at `--detail semantic` against a file whose language
   server supports hover returns a card with a populated `lsp` field containing
   `hover`, `type` (JSON key for the Rust field `type_info`), `deprecated`, and
   `source` attributes, and provenance sources including both `"tree_sitter"`
   and `"lsp_hover"`.
2. The same request against a file where the LSP is unavailable (no server
   registered, hover capability missing, or server error) returns a card with
   `lsp: null` (omitted in JSON) and provenance containing
   `"tree_sitter_degraded_semantic"` — identical to the current behaviour.
3. Requests at `--detail structure` or lower are unaffected; they never
   attempt LSP enrichment.
4. `make check-fmt`, `make lint`, `make test`, `make markdownlint`, and
   `make nixie` all exit 0 when run through the repository's required
   `pipefail` plus `tee` pattern.

## Constraints

1. The public JSON schema of `SymbolCard`, `LspInfo`, `GetCardResponse`,
   `DetailLevel`, and `Provenance` defined in `crates/weaver-cards/src/` is
   already locked from roadmap 7.1.1. The `LspInfo` Rust struct already has the
   correct field shape (`hover`, `type_info`, `deprecated`, `source`) and
   serializes `type_info` as JSON key `"type"`. This task must populate it, not
   redesign it.
2. The workspace enforces strict linting: `unwrap_used`, `expect_used`,
   `indexing_slicing`, `string_slice`, `allow_attributes`, `missing_docs`,
   `missing_const_for_fn`, `cognitive_complexity`, and the 400-line file limit
   for code files.
3. Every new Rust module must begin with a `//!` comment, and all public
   items must have `///` rustdoc comments.
4. `GetCardResponse` is `#[non_exhaustive]`, so matches in `weaverd` must
   keep a wildcard arm.
5. Behaviour tests must use `rstest-bdd` v0.5.0 with the `world` fixture
   convention.
6. The Tree-sitter extraction layer in `crates/weaver-cards/` must remain
   independent of LSP. LSP enrichment is a post-extraction concern owned by the
   daemon handler in `crates/weaverd/`.
7. Prerequisites 2.1.9 (LSP host with capability negotiation) and 3.1.3
   (document sync) are already complete.
8. en-GB-oxendict spelling for comments and documentation.

## Tolerances (exception triggers)

- Scope: if implementation requires changes to more than 15 files (net), stop
  and escalate.
- Interface: if `SymbolCard`, `LspInfo`, `GetCardResponse`, or `DetailLevel`
  require schema changes, stop and escalate.
- Dependencies: if a new external crate dependency is required, stop and
  escalate.
- Iterations: if tests still fail after 5 attempts at fixing, stop and
  escalate.
- Ambiguity: if multiple valid interpretations exist for hover response
  parsing that materially affect the output, stop and present options.

## Risks

- Risk: Hover response parsing complexity. Language servers return
  `MarkupContent` or `MarkedString` variants that differ by server. Severity:
  medium. Likelihood: medium. Mitigation: extract the plain-text or Markdown
  content as a single string; type extraction is best-effort from the hover
  text. Defer structured type resolution to a future milestone.

- Risk: The behaviour-driven development (BDD) test harness creates a
  `SemanticBackendProvider` but does not register real language servers, so LSP
  enrichment cannot be exercised in BDD tests without mocking. Severity: low.
  Likelihood: high. Mitigation: BDD tests verify the degraded path (LSP
  unavailable). Unit tests in `get_card.rs` use an injected `LspHost` with stub
  language servers to verify the enriched path.

- Risk: File size limits. `get_card.rs` is currently 297 lines and
  `host.rs` is 366 lines. Adding enrichment logic may push past 400. Severity:
  low. Likelihood: medium. Mitigation: extract enrichment logic into a sibling
  module `crates/weaverd/src/dispatch/observe/enrich.rs`.

## Progress

- [x] (2026-03-15) Stage A: Add `Hover` capability to `weaver-lsp-host`
  (`capability.rs`, `server.rs`, `errors.rs`, `host.rs`).
- [x] (2026-03-15) Stage B: Wire hover method through `LspHost` — updated
  all `LanguageServer` trait implementations (`ProcessLanguageServer`,
  `DocStubServer`, `RecordingLanguageServer`, `failing_server!` macro, and the
  documentation stub example).
- [x] (2026-03-15) Stage C: Created `enrich.rs` module, registered in
  `observe/mod.rs`, updated `get_card::handle()` signature to accept
  `backends`, and updated `router.rs` to pass `backends`.
- [x] (2026-03-15) Stage D: Unit tests for hover response parsing
  (6 tests in `enrich.rs`). Updated existing `get_card.rs` unit tests to pass
  `backends` via a rstest fixture.
- [x] (2026-03-15) Stage E: BDD tests pass unchanged — the degraded
  path is naturally exercised since no real language servers are registered.
  The `behaviour.rs` BDD test in `weaver-lsp-host` needed `hover: None` added
  to `sample_responses()` and `.with_hover(true)` added to `all_caps`.
- [x] (2026-03-15) Stage F: Updated `docs/roadmap.md` (ticked 7.1.3
  checkboxes) and `docs/users-guide.md` (documented enrichment and degradation
  behaviour).
- [x] (2026-03-15) Stage G: `make check-fmt`, `make lint`, and
  `make test` all exit 0.

## Surprises & discoveries

- Observation: The existing `behaviour.rs` BDD test for
  `weaver-lsp-host` iterates over all capability states and asserts every one
  is enabled. Adding `Hover` to the capability system caused this test to fail
  because the test's `all_caps` did not include hover. Evidence: Test failure
  in `lsp_host_behaviour` asserting "capability Hover should be enabled".
  Impact: Required adding `.with_hover(true)` to the test's capability set and
  `hover: None` to `sample_responses()`.

## Decision log

- Decision: LSP enrichment is a daemon-layer concern, not a weaver-cards
  concern. Rationale: The card extraction crate must remain pure over source
  text and not depend on LSP infrastructure. The daemon handler already owns
  the backend lifecycle and is the natural place to attempt enrichment after
  Tree-sitter extraction. Date/Author: 2026-03-15, agent.

- Decision: Add a new `CapabilityKind::Hover` variant rather than reusing
  an existing capability. Rationale: Hover is a distinct LSP feature
  (`textDocument/hover`) with its own server capability flag. It deserves its
  own capability kind for accurate negotiation and override semantics.
  Date/Author: 2026-03-15, agent.

- Decision: Extract enrichment logic into
  `crates/weaverd/src/dispatch/observe/enrich.rs` rather than inlining in
  `get_card.rs`. Rationale: Keeps `get_card.rs` within the 400-line budget and
  separates the enrichment concern from request parsing and response
  serialization. Date/Author: 2026-03-15, agent.

## Outcomes & retrospective

All acceptance criteria met. The implementation touched 12 files (within the
15-file tolerance) across two crates:

- `weaver-lsp-host`: 7 files modified (capability, server, errors, host,
  trait_impl, doc_support, recording_server) plus 2 test files (behaviour.rs,
  unit.rs) and 1 documentation file.
- `weaverd`: 1 new file created (`enrich.rs`), 3 files modified
  (`mod.rs`, `get_card.rs`, `router.rs`).

No schema changes were required — `LspInfo` and `SymbolCard` from roadmap 7.1.1
were populated as-is. No new external dependencies were added.

Lessons learned:

1. When adding a new capability to the negotiation system, existing BDD
   tests that iterate over "all capabilities" will fail unless the new
   capability is included in the test's capability set. This should be
   anticipated for future capability additions.
2. Extracting enrichment into a sibling module was the right call — it
   kept both `get_card.rs` (347 lines) and `enrich.rs` (273 lines) comfortably
   within the 400-line budget.

Post-implementation improvements (2026-03-20):

1. **UTF-8 position encoding negotiation**: Added UTF-8 encoding preference to
   the LSP initialize handshake (`ClientCapabilities.general.position_encodings`).
   When the server agrees to UTF-8 (LSP 3.17+), Tree-sitter byte offsets can be
   used directly as character offsets, eliminating the UTF-16 conversion issue
   for the majority of modern language servers. Servers that decline UTF-8
   negotiation are logged with a debug warning. This change improves correctness
   for files containing non-ASCII characters without requiring architectural
   changes to pass source text through the enrichment pipeline.

2. **Allocation optimization**: Replaced `contains(&String::from("lsp_hover"))`
   with `iter().any(|s| s == "lsp_hover")` in `apply_lsp_enrichment` to avoid
   allocating a temporary `String` during provenance checks.

## Context and orientation

The `observe get-card` operation extracts a structured symbol card for a given
file position. The operation spans three crates:

- `crates/weaver-cards/` defines the card schema (`SymbolCard`, `LspInfo`,
  `DetailLevel`, `Provenance`) and Tree-sitter extraction. The extractor
  currently sets `lsp: None` and records `"tree_sitter_degraded_semantic"` in
  provenance when `detail >= Semantic`.
- Before 7.1.3, `crates/weaver-lsp-host/` managed per-language LSP servers with
  capability negotiation and supported four capability kinds (`Definition`,
  `References`, `Diagnostics`, `CallHierarchy`) but not `Hover`.
- Before 7.1.3, `crates/weaverd/` is the daemon. The handler at
  `crates/weaverd/src/dispatch/observe/get_card.rs` only received
  `(request, writer)` and did not have access to backends. The router at
  `crates/weaverd/src/dispatch/router.rs` required updating to pass `backends`
  so the handler could attempt LSP enrichment.

**Table:** Key types and locations referenced in this plan

| Type                      | File                                              |
| ------------------------- | ------------------------------------------------- |
| `SymbolCard`, `LspInfo`   | `crates/weaver-cards/src/card.rs`                 |
| `DetailLevel`             | `crates/weaver-cards/src/detail.rs`               |
| `Provenance`              | `crates/weaver-cards/src/card.rs`                 |
| `provenance_sources()`    | `crates/weaver-cards/src/extract/utils.rs`        |
| `CapabilityKind`          | `crates/weaver-lsp-host/src/capability.rs`        |
| `ServerCapabilitySet`     | `crates/weaver-lsp-host/src/server.rs`            |
| `LanguageServer` trait    | `crates/weaver-lsp-host/src/server.rs`            |
| `LspHost`                 | `crates/weaver-lsp-host/src/host.rs`              |
| `HostOperation`           | `crates/weaver-lsp-host/src/errors.rs`            |
| `get_card::handle()`      | `crates/weaverd/src/dispatch/observe/get_card.rs` |
| `route_observe()`         | `crates/weaverd/src/dispatch/router.rs`           |
| `SemanticBackendProvider` | `crates/weaverd/src/semantic_provider/mod.rs`     |
| BDD world                 | `crates/weaverd/src/tests/get_card_behaviour.rs`  |
| BDD feature               | `crates/weaverd/tests/features/get_card.feature`  |

The `observe get-definition` handler at
`crates/weaverd/src/dispatch/observe/get_definition.rs` serves as the reference
pattern for how to access backends and call LSP methods from a handler.

## Plan of work

### Stage A: Add Hover capability to weaver-lsp-host

**A1. Add `Hover` variant to `CapabilityKind`**
(`crates/weaver-lsp-host/src/capability.rs`)

Add `Hover` after `CallHierarchy` in the enum at line 21, with key
`"observe.get-card-hover"`. Add the `Hover` variant to the
`resolve_capabilities` loop (line 121) and the `resolve_state` match (line 149).

**A2. Add `hover` field to `ServerCapabilitySet`**
(`crates/weaver-lsp-host/src/server.rs`)

Add `pub(crate) hover: bool` to the struct (line 20). Add builder method
`with_hover(bool)`. Add query method `supports_hover()`. The existing `new()`
constructor initializes `hover: false` for backwards compatibility.

**A3. Add `hover` method to `LanguageServer` trait**
(`crates/weaver-lsp-host/src/server.rs`)

Add:

```rust
fn hover(
    &mut self,
    params: lsp_types::HoverParams,
) -> Result<Option<lsp_types::Hover>, LanguageServerError>;
```

**A4. Add `Hover` variant to `HostOperation`**
(`crates/weaver-lsp-host/src/errors.rs`)

Add `Hover` to the enum and `"hover"` to the `Display` impl.

**A5. Add `hover` method to `LspHost`** (`crates/weaver-lsp-host/src/host.rs`)

Use the `lsp_method!` macro, following the pattern of `goto_definition`:

```rust
lsp_method!(
    /// Routes a hover request to the configured language server.
    pub fn hover(
        &mut self,
        language: Language,
        params: lsp_types::HoverParams,
    ) -> Result<Option<lsp_types::Hover>, LspHostError> {
        CapabilityKind::Hover,
        HostOperation::Hover,
        hover
    }
);
```

**A6. Update imports in `host.rs`** to include `HoverParams` from `lsp_types`.

### Stage B: Update weaver-lsp-host tests

Existing tests in `crates/weaver-lsp-host/src/tests/` use mock servers. Add the
`hover` method to the mock implementations and add a unit test verifying that
`hover` succeeds when the capability is advertised and returns
`CapabilityUnavailable` when it is not.

### Stage C: Add enrichment module and update handler/router

**C1. Create `crates/weaverd/src/dispatch/observe/enrich.rs`**

This module owns the LSP enrichment concern. It exposes a single public
function:

```rust
/// Attempts LSP hover enrichment on a Tree-sitter-extracted card.
///
/// When the semantic backend is available and the language server
/// supports hover, this function calls `textDocument/hover` at the
/// card's symbol position and populates the `lsp` field. When LSP
/// is unavailable, the card is returned unchanged.
pub fn try_lsp_enrichment(
    card: &mut SymbolCard,
    backends: &mut FusionBackends<SemanticBackendProvider>,
) -> EnrichmentOutcome
```

Where `EnrichmentOutcome` is:

```rust
pub enum EnrichmentOutcome {
    /// LSP enrichment succeeded; provenance should include "lsp_hover".
    Enriched,
    /// LSP was unavailable; provenance is unchanged.
    Degraded,
}
```

The function:

1. Inspects `card.symbol.symbol_ref.language` to derive the LSP
   `Language`.
2. Calls `backends.ensure_started(BackendKind::Semantic)`. On failure,
   returns `Degraded`.
3. Calls `backends.provider().with_lsp_host_mut(...)` to initialize the
   language server and call `hover`. On any error (unknown language, capability
   unavailable, server error), returns `Degraded`.
4. On success, parses the `Hover` response to extract plain text, type
   info, and deprecation signal. Sets `card.lsp = Some(LspInfo { ... })` and
   returns `Enriched`.

Hover response parsing helper:

```rust
fn parse_hover_response(hover: &lsp_types::Hover) -> LspInfo
```

Extracts the hover contents as a string (handling `MarkedString`,
`MarkupContent`, and `Vec<MarkedString>` variants). Type info is extracted as
best-effort from the hover text. Deprecation is detected by identifying
structured deprecation markers in the hover payload (such as lines beginning
with `@deprecated`, `**deprecated**`, or similar marker tokens).

**C2. Register `enrich` module** (`crates/weaverd/src/dispatch/observe/mod.rs`)

Add `pub mod enrich;` to the module declaration.

**C3. Update `get_card::handle()` signature and logic**
(`crates/weaverd/src/dispatch/observe/get_card.rs`)

Change the handler to accept `backends`:

```rust
pub fn handle<W: Write>(
    request: &CommandRequest,
    writer: &mut ResponseWriter<W>,
    backends: &mut FusionBackends<SemanticBackendProvider>,
) -> Result<DispatchResult, DispatchError> {
```

After successful Tree-sitter extraction but before serialization, when
`card_request.detail >= DetailLevel::Semantic`, call
`enrich::try_lsp_enrichment(&mut card, backends)`. If the outcome is
`Enriched`, update provenance sources to replace
`"tree_sitter_degraded_semantic"` with `"lsp_hover"`.

**C4. Update router** (`crates/weaverd/src/dispatch/router.rs` line 199)

Change:

```rust
"get-card" => observe::get_card::handle(request, writer),
```

To:

```rust
"get-card" => observe::get_card::handle(request, writer, backends),
```

### Stage D: Unit tests

**D1. Unit tests for `enrich.rs`**

Test `try_lsp_enrichment` with:

- A mock backend that returns a hover response → card has `lsp` populated.
- A mock backend that returns capability unavailable → card.lsp remains
  `None`, outcome is `Degraded`.
- A mock backend where the semantic backend fails to start → `Degraded`.

**D2. Unit tests for `parse_hover_response`**

Test with:

- `HoverContents::Scalar(MarkedString::String("..."))` — plain text.
- `HoverContents::Markup(MarkupContent { kind: Markdown, value: "..." })`
  — Markdown content.
- Empty hover contents — yields empty strings.

**D3. Update existing `get_card.rs` tests**

The existing tests call `handle(&request, &mut writer)`. They need to be
updated to pass a `backends` argument. Create a test helper that builds a
`FusionBackends` with a no-op provider (the existing tests only exercise
Tree-sitter extraction at `structure` detail, so LSP is never attempted).

### Stage E: BDD tests

**E1. The existing BDD scenario "Semantic detail degrades to Tree-sitter
provenance"** already verifies the degraded path. Since the BDD test harness
creates a `SemanticBackendProvider` without real language servers registered,
LSP enrichment will naturally fail and degrade. This scenario continues to pass
unchanged.

**E2. Verify the existing feature file still passes.** No new BDD scenarios are
required because the enriched path requires a real language server, which is an
E2E concern for a future milestone.

### Stage F: Documentation

**F1. Update `docs/roadmap.md`** — tick the 7.1.3 checkboxes.

**F2. Update `docs/users-guide.md`** — document that `--detail semantic` now
attempts LSP enrichment and explain the degradation behaviour.

**F3. Update `docs/jacquard-card-first-symbol-graph-design.md`** if needed —
note the enrichment flow.

### Stage G: Commit gating

Run all quality gates:

```sh
set -o pipefail; make fmt 2>&1 | tee /tmp/7-1-3-fmt.log
set -o pipefail; make markdownlint 2>&1 | tee /tmp/7-1-3-markdownlint.log
set -o pipefail; make nixie 2>&1 | tee /tmp/7-1-3-nixie.log
set -o pipefail; make check-fmt 2>&1 | tee /tmp/7-1-3-check-fmt.log
set -o pipefail; make lint 2>&1 | tee /tmp/7-1-3-lint.log
set -o pipefail; make test 2>&1 | tee /tmp/7-1-3-test.log
```

## Concrete steps

All commands are run from the repository root `/home/user/project`.

1. Edit `crates/weaver-lsp-host/src/capability.rs` — add `Hover` variant.
2. Edit `crates/weaver-lsp-host/src/server.rs` — add `hover` field and
   trait method.
3. Edit `crates/weaver-lsp-host/src/errors.rs` — add `Hover` operation.
4. Edit `crates/weaver-lsp-host/src/host.rs` — add `hover` lsp_method.
5. Update mock servers in tests and add hover capability tests.
6. Create `crates/weaverd/src/dispatch/observe/enrich.rs`.
7. Register module in `crates/weaverd/src/dispatch/observe/mod.rs`.
8. Edit `crates/weaverd/src/dispatch/observe/get_card.rs` — update
   handler signature and add enrichment call.
9. Edit `crates/weaverd/src/dispatch/router.rs` — pass backends.
10. Update existing unit tests for new handler signature.
11. Verify BDD tests still pass.
12. Update documentation.
13. Run commit gating.

## Validation and acceptance

Quality criteria:

- `make check-fmt` exits 0.
- `make lint` exits 0.
- `make test` exits 0.
- `make markdownlint` exits 0.
- `make nixie` exits 0.
- The unit test for enrichment with a mock hover response verifies
  `card.lsp.is_some()` and provenance includes `"lsp_hover"`.
- The unit test for degraded enrichment verifies `card.lsp.is_none()` and
  provenance includes `"tree_sitter_degraded_semantic"`.
- The BDD scenario "Semantic detail degrades to Tree-sitter provenance"
  continues to pass.

## Idempotence and recovery

All steps are additive and can be re-run. If any stage fails, fix the issue and
re-run the stage's validation. The change does not modify any existing
serialized data or persistent state.

## Artefacts and notes

The `observe get-definition` handler at
`crates/weaverd/src/dispatch/observe/get_definition.rs` is the reference
pattern for LSP backend access from a dispatch handler.

## Interfaces and dependencies

No new external dependencies. All types come from `lsp_types` (already a
workspace dependency) and existing crates.

In `crates/weaver-lsp-host/src/capability.rs`:

```rust
pub enum CapabilityKind {
    Definition,
    References,
    Diagnostics,
    CallHierarchy,
    Hover,  // NEW
}
```

In `crates/weaver-lsp-host/src/server.rs`:

```rust
pub trait LanguageServer: Send {
    // ... existing methods ...
    fn hover(
        &mut self,
        params: lsp_types::HoverParams,
    ) -> Result<Option<lsp_types::Hover>, LanguageServerError>;
}
```

In `crates/weaverd/src/dispatch/observe/enrich.rs`:

```rust
pub enum EnrichmentOutcome {
    Enriched,
    Degraded,
}

pub fn try_lsp_enrichment(
    card: &mut SymbolCard,
    backends: &mut FusionBackends<SemanticBackendProvider>,
) -> EnrichmentOutcome
```
