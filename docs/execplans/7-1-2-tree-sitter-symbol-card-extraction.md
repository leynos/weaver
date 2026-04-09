# 7.1.2 Implement Tree-sitter symbol card extraction for `observe get-card`

This ExecPlan (execution plan) is a living document. The sections
`Constraints`, `Tolerances`, `Risks`, `Progress`, `Surprises & Discoveries`,
`Decision Log`, and `Outcomes & Retrospective` must be kept up to date as work
proceeds.

Status: COMPLETE

This document must be maintained in accordance with `AGENTS.md` at the
repository root.

## Purpose / big picture

After this change, `weaver observe get-card` will stop returning the
placeholder refusal for supported Rust, Python, and TypeScript files. Instead,
it will parse the target file with Tree-sitter, resolve the symbol covering the
requested position, and return a deterministic `SymbolCard` payload that
includes:

- a stable `SymbolId` that does not drift under whitespace-only edits,
- bounded structural detail (`signature`, `doc`, `attachments`, `structure`,
  `metrics`) derived without a live language server,
- file/module interstitial data for imports and related top-level spans, and
- structured refusals for unsupported languages and positions that do not map
  to a symbol.

Observable behaviour after implementation:

1. Running `weaver observe get-card --uri <file://...> --position <line:col>`
   against fixture Rust, Python, and TypeScript files returns
   `"status":"success"` with a populated card.
2. Cards for decorated or documented symbols include stable bundled
   attachments even when blank lines or indentation are edited.
3. Nested locals, lambdas, and closures never appear in the entity table or as
   top-level cards.
4. Requests for unsupported extensions or symbol-less positions return a
   structured refusal rather than a daemon crash or ad hoc text.
5. `make check-fmt`, `make lint`, `make test`, `make markdownlint`, and
   `make nixie` all exit 0 when run through the repository's required
   `pipefail` plus `tee` pattern.

## Constraints

1. The public JSON schema defined in
   `crates/weaver-cards/src/{card,detail,request,response,symbol}.rs` and
   completed in roadmap item 7.1.1 is already locked. This task must implement
   extraction against that schema, not redesign it.
2. This is the Tree-sitter-first milestone only. Do not require Language
   Server Protocol (LSP) startup in 7.1.2. Later roadmap item 7.1.3 will enrich
   cards semantically.
3. Use the existing syntactic foundation in `crates/weaver-syntax/` as the
   parsing layer. Extraction logic belongs with the card domain, not in
   `weaverd`.
4. Keep filesystem and uniform resource identifier (URI) handling in
   `crates/weaverd/`; keep card extraction itself pure over provided source
   text, path, and request position so it is cheap to unit test.
5. The workspace enforces strict linting from `Cargo.toml`, including
   `unwrap_used`, `expect_used`, `indexing_slicing`, `string_slice`,
   `allow_attributes`, `missing_docs`, `missing_const_for_fn`,
   `cognitive_complexity`, and the 400-line file limit for code files.
6. Every new Rust module must begin with a `//!` comment, and all public items
   must have `///` rustdoc comments.
7. Behaviour tests must use `rstest-bdd` v0.5.0, which is already pinned in
   the workspace. Fixture matching is by parameter name; the shared BDD fixture
   must remain named `world`.
8. Documentation changes are part of scope:
   `docs/jacquard-card-first-symbol-graph-design.md`, `docs/users-guide.md`,
   and `docs/roadmap.md` must be updated before the work is considered complete.
9. `docs/users-guide.md` must explain any new user-visible behaviour,
   especially successful `get-card` responses, refusal cases, and the fact that
   `semantic` / `full` requests still degrade to Tree-sitter-only sections in
   this milestone.
10. The roadmap entry must not be marked done until all implementation,
   documentation, and validation steps in this plan have completed.

## Tolerances

- If delivering the feature requires changing the serialized shape of
  `SymbolCard`, `GetCardResponse`, `DetailLevel`, `CardLanguage`, or
  `CardSymbolKind`, stop and escalate before implementing. Those contracts were
  the whole point of 7.1.1.
- If the work cannot be completed within roughly 20 touched files or requires
  more than one new public API surface in `weaver-cards`, stop and re-plan
  before continuing.
- If any planned Rust file grows beyond 350 lines, split it before continuing.
  Do not wait to hit the 400-line hard limit.
- If one of the three target languages cannot be covered with deterministic
  Tree-sitter queries without adding a new external dependency, stop and
  escalate.
- If `make test` still hangs after one focused retry and log inspection, stop
  and document the blocker instead of burning time blindly.

## Risks

- Risk: Tree-sitter grammar differences across Rust, Python, and TypeScript
  can make "one extractor" drift into a branch-heavy, hard-to-maintain module.
  Mitigation: keep a shared pipeline and isolate language-specific query and
  normalization logic into per-language submodules.
- Risk: attachment bundling can accidentally depend on whitespace layout rather
  than syntax, which would break the acceptance criterion around stability
  under formatting edits. Mitigation: base bundling on contiguous trivia and
  token classes, not raw line counts.
- Risk: `SymbolId` may drift if raw comments, decorator order formatting, or
  interstitial raw text leak into the fingerprint input. Mitigation: define one
  canonical normalization function and test it directly.
- Risk: file-level interstitials do not map cleanly to the current schema,
  because 7.1.1 locked the symbol kinds before extraction was implemented.
  Mitigation: represent file-level context on synthetic module cards rather
  than widening the schema in this milestone.
- Risk: the existing note about a possible `make test` hang in
  `weaver-cli::tests::unit::auto_start::auto_start_succeeds_and_proceeds` can
  obscure whether a failure comes from this work or from unrelated test
  instability. Mitigation: use `tee` logs, inspect the tail immediately, and
  isolate the failing crate before changing code.

## Progress

- [x] (2026-03-07 00:00Z) Review roadmap item 7.1.2, the Jacquard design
  document, the existing 7.1.1 schema plan, and the testing/documentation
  guides referenced by the task.
- [x] (2026-03-07 00:00Z) Inspect the current implementation state in
  `weaver-cards`, `weaver-syntax`, and `weaverd`.
- [x] (2026-03-07 00:00Z) Draft this ExecPlan.
- [x] (2026-03-07 00:00Z) Add extraction modules and deterministic
  fingerprinting in `crates/weaver-cards/`.
- [x] (2026-03-07 00:00Z) Wire `weaverd` `observe get-card` to the extractor
  and replace the placeholder refusal on supported inputs.
- [x] (2026-03-07 00:00Z) Add unit and behaviour tests covering happy paths,
  refusals, and determinism edge cases.
- [x] (2026-03-07 00:00Z) Update the Jacquard design doc, user guide, and
  roadmap.
- [x] (2026-03-07 00:00Z) Run formatting, lint, test, and Markdown validation
  gates with logged output.

## Surprises & Discoveries

- `crates/weaver-cards/` already exists and cleanly models the JSON schema, but
  it currently contains no extraction or fingerprinting logic.
- `crates/weaverd/src/dispatch/observe/get_card.rs` currently parses the
  request and always returns `GetCardResponse::not_yet_implemented(...)`.
- The workspace already pins `rstest-bdd = "0.5.0"` and
  `rstest-bdd-macros = "0.5.0"` in the root `Cargo.toml`, so no dependency
  update is needed to satisfy the task's BDD requirement.
- The current `docs/users-guide.md` explicitly states that Tree-sitter card
  extraction is not implemented yet. That text must be removed or rewritten
  once the feature lands.
- The first `SymbolId` implementation drifted under whitespace-only edits
  because it hashed the human-readable `signature.display`. The fix was to hash
  canonical parameter and return-shape data instead.
- `rstest-bdd` coverage is simplest to maintain as a dedicated `get_card`
  feature rather than by stretching the generic daemon-dispatch feature file.

## Decision Log

- Decision: implement Tree-sitter card extraction in `weaver-cards`, not in
  `weaver-syntax` or `weaverd`. Rationale: `weaver-cards` owns the stable
  schema and is the natural place for turning syntax trees into cards.
  `weaver-syntax` remains the parsing utility; `weaverd` remains the transport
  layer.
- Decision: keep the extractor API free of direct filesystem I/O.
  Rationale: this follows the repository's testability guidance and keeps unit
  tests deterministic. `weaverd` will read the file and pass the content to the
  extractor.
- Decision: represent file-level interstitials on synthetic module cards in
  7.1.2 rather than adding a new public symbol kind. Rationale: the schema is
  already locked, and `CardSymbolKind::Module` is sufficient for
  file/module-level cards carrying `interstitial` data.
- Decision: requests for `--detail semantic` or `--detail full` will still
  return a successful Tree-sitter-only card in this milestone, with the higher
  sections simply absent and provenance showing only Tree-sitter sources.
  Rationale: the schema is additive, and later milestones are explicitly about
  enrichment rather than about changing request validity.
- Decision: `SymbolId` must hash canonical structured signature data
  (`params`, `returns`) rather than the rendered signature string. Rationale:
  display strings still drift under harmless whitespace edits around
  punctuation, which violates the roadmap acceptance criteria.
- Decision: these four decisions must be copied into
  `docs/jacquard-card-first-symbol-graph-design.md` during implementation so
  the design document stays authoritative.

## Outcomes & Retrospective

Successful completion will mean:

- supported `get-card` requests succeed through the real daemon path,
- Tree-sitter extraction is deterministic across whitespace-only edits,
- nested locals remain structural data only and never become entities,
- the design doc and user guide match the shipped behaviour, and
- all required repository gates pass with saved logs.

Completed implementation notes:

- `weaver-cards` now owns a pure Tree-sitter extraction pipeline with
  language-specific modules for Rust, Python, and TypeScript.
- `weaverd` now resolves `file://` URIs, loads source, and maps unsupported
  language and no-symbol cases into structured `get-card` refusals.
- Unit coverage now exercises supported symbol kinds, interstitial module
  cards, determinism rules, attachment stability, nested-entity filtering, and
  `semantic` provenance degradation.
- `rstest-bdd` behaviour coverage now exercises daemon-level success,
  degraded-semantic success, unsupported-language refusal, and no-symbol
  refusal scenarios.

## Context and orientation

The feature touches three existing areas:

1. `crates/weaver-cards/`
   This crate currently defines the stable `observe get-card` schema from
   7.1.1. Relevant files are `src/card.rs`, `src/detail.rs`, `src/request.rs`,
   `src/response.rs`, and `src/symbol.rs`.
2. `crates/weaver-syntax/`
   This crate already wraps Tree-sitter parsing. `src/parser.rs` exposes
   `Parser` and `ParseResult`; `src/language.rs` exposes
   `SupportedLanguage::{from_extension, from_path, tree_sitter_language}`.
3. `crates/weaverd/`
   `src/dispatch/observe/get_card.rs` is the current placeholder handler.
   `src/dispatch/router.rs` already routes `"get-card"` requests correctly.

The implementation should preserve that separation:

- `weaverd` parses CLI arguments, resolves the file URI, reads the file, and
  maps extraction failures into `GetCardResponse::Refusal`.
- `weaver-cards` parses source text into one or more regions, resolves the
  target entity or synthetic module card, computes the card payload, and
  returns deterministic domain data.
- `weaver-syntax` remains the shared syntactic parsing utility rather than
  becoming another schema or transport layer.

## Plan of work

### Stage A: Add a real extraction surface to `weaver-cards`

Add a small extraction API to `crates/weaver-cards/` and keep it intentionally
pure. The implementation should depend on `weaver-syntax` for parsing and can
use `tree-sitter` query types directly if required, but the public API should
accept already-loaded source text.

Expected shape:

```rust
pub struct CardExtractionInput<'a> { ... }
pub enum CardExtractionError { ... }
pub struct TreeSitterCardExtractor;
impl TreeSitterCardExtractor {
    pub fn extract(&self, input: &CardExtractionInput<'_>) -> Result<SymbolCard, CardExtractionError>;
}
```

The exact names may differ, but the responsibilities must not:

- the input must include source text, file path, requested position, and
  requested `DetailLevel`,
- the extractor must not open files or parse URIs itself, and
- the error type must distinguish at least unsupported language, parse failure,
  and no symbol at position so `weaverd` can map them to the existing refusal
  reasons.

Likely module split:

- `crates/weaver-cards/src/extractor/mod.rs`
- `crates/weaver-cards/src/extractor/region.rs`
- `crates/weaver-cards/src/extractor/attachments.rs`
- `crates/weaver-cards/src/extractor/fingerprint.rs`
- `crates/weaver-cards/src/extractor/languages/{mod,rust,python,typescript}.rs`

Do not collapse all of this into `lib.rs` or `card.rs`.

### Stage B: Build the region pass and per-language extraction rules

Implement the deterministic two-pass extraction pipeline described in
`docs/jacquard-card-first-symbol-graph-design.md`:

1. Parse the file and build alternating entity and interstitial regions.
2. Resolve the requested position to either:
   - the smallest eligible entity region that contains it, or
   - the synthetic file/module card when the position is inside a file-level
     interstitial span that the schema attaches to the module.
3. Extract only the card requested by the position, but keep the intermediate
   entity table available for interstitial attachment and nested filtering.

Language-specific query coverage must be sufficient to satisfy the acceptance
criterion of at least three symbol kinds per language. A practical target set
is:

- Rust: function, method, type-like item (`struct`, `enum`, `trait`, or
  `type`), plus module card support.
- Python: function, class, method, with decorators and docstrings handled
  correctly.
- TypeScript: function, class or method, and interface or type alias, plus
  decorators where supported by the grammar.

The first implementation should prefer accuracy and determinism over breadth.
It is acceptable to support a narrow, explicit set of node kinds as long as the
unsupported cases degrade predictably instead of silently emitting bad cards.

### Stage C: Implement interstitial attachment and backwards-scanned bundles

This is the most design-sensitive part of the milestone.

Implement three related pieces together:

1. File/module interstitial pass.
   Capture import/use blocks, top-level headers, and similar between-entity
   spans. Normalize imports into the existing `InterstitialInfo` schema,
   preserving both the raw block and the normalized grouping.
2. Attachment bundling.
   Starting from the symbol start byte, scan backwards across trivia until a
   non-comment, non-decorator, non-annotation token is found. Attach the
   contiguous leading block to the symbol card and record
   `attachments.bundle_rule = "leading_trivia"`.
3. In-body documentation extraction where the language requires it.
   Python docstrings are inside the entity body, so `doc.docstring` and
   `doc.summary` need a language-specific rule that is separate from backwards
   scanning.

The important invariants are:

- whitespace edits inside the bundled trivia may change raw text but must not
  change the attachment boundaries,
- blank lines inside a contiguous block are handled consistently,
- decorators and annotations are normalized for `attachments.normalized`, and
- file-level interstitials are attached to the relevant module card rather than
  getting dropped.

### Stage D: Enforce nested entity filtering and stable fingerprints

Nested locals must appear only in `structure.locals`, never as top-level
entities or selectable cards, unless the nesting rule is explicitly part of the
language model. For this milestone, the default should be:

- include class members and methods as entities when the grammar models them as
  top-level members of the enclosing class or impl item,
- exclude local functions, closures, lambdas, and inner helpers from the
  entity table,
- keep branches and locals as structural metadata on the enclosing card.

Implement fingerprinting only after the filtering and attachment rules are in
place. The fingerprint input must use canonical, whitespace-stable data:

- language,
- symbol kind,
- canonical or container-qualified name,
- normalized signature representation,
- normalized AST shape features, and
- a low-weight normalized path hint.

Do not include raw whitespace, raw comment formatting, or raw interstitial text
in the fingerprint. Attachments may contribute only through normalized forms
that are stable under whitespace-only edits.

### Stage E: Replace the placeholder `weaverd` handler

Update `crates/weaverd/src/dispatch/observe/get_card.rs` so it performs the
real flow:

1. Parse `GetCardRequest`.
2. Resolve the `file://` URI to a local path.
3. Read the file content.
4. Infer the language from the path extension.
5. Call the `weaver-cards` extractor.
6. Serialize `GetCardResponse::Success` on success.
7. Map extractor failures to the existing refusal reasons:
   - unsupported language,
   - no symbol at position,
   - backend unavailable only if a future optional backend is missing,
   - not yet implemented only for genuinely deferred cases outside the three
     supported languages.

The handler should keep returning a structured refusal instead of raw stderr
text for known failure modes.

### Stage F: Add the required unit and behaviour tests

Unit tests belong primarily in `crates/weaver-cards/`. They should cover the
deterministic core directly and use `rstest` parameterization aggressively
instead of duplicating fixtures.

Required unit coverage:

- at least three symbol kinds per language,
- deterministic extracted ranges for the same input,
- stable comment and decorator bundling under whitespace edits,
- nested locals and closures excluded from the entity table,
- stable `SymbolId` fingerprints under whitespace-only edits,
- import or interstitial normalization for at least one file/module example.

Behaviour coverage must use `rstest-bdd` v0.5.0 and exercise observable
end-to-end outcomes. Prefer a dedicated daemon-level feature file such as
`crates/weaverd/tests/features/get_card.feature` plus a matching Rust test
module in `crates/weaverd/src/tests/`.

Minimum BDD scenarios:

1. Rust symbol card happy path.
2. Python decorated symbol happy path.
3. TypeScript symbol happy path.
4. Position with no symbol returns a structured refusal.
5. Unsupported language returns a structured refusal.

If the daemon-level BDD becomes too bulky, keep the end-to-end scenarios in
`weaverd` and place finer-grained edge-case scenarios in `weaver-cards`. Do not
drop the daemon-level happy path entirely.

### Stage G: Update the design doc, user guide, and roadmap

The implementation is not complete until the documentation matches.

Update `docs/jacquard-card-first-symbol-graph-design.md` to record the
decisions from this plan, especially:

- extraction living in `weaver-cards`,
- synthetic module cards for file-level interstitials,
- Tree-sitter-only degradation for `semantic` and `full`,
- the precise fingerprint normalization boundaries.

Update `docs/users-guide.md` to replace the current "not yet implemented"
language in the `observe get-card` section with:

- supported languages,
- success behaviour and sample output,
- refusal cases,
- the meaning of detail levels in the Tree-sitter-first phase.

Only after the feature, tests, and docs are complete should `docs/roadmap.md`
mark item 7.1.2 as done.

## Validation

Run all commands from the repository root, with `set -o pipefail` and `tee` so
the exit status is preserved and the logs can be inspected afterward.

Documentation-phase validation during implementation:

```sh
set -o pipefail; make fmt 2>&1 | tee /tmp/7-1-2-make-fmt.log
set -o pipefail; make markdownlint 2>&1 | tee /tmp/7-1-2-markdownlint.log
set -o pipefail; make nixie 2>&1 | tee /tmp/7-1-2-nixie.log
```

Final delivery gates:

```sh
set -o pipefail; make check-fmt 2>&1 | tee /tmp/7-1-2-check-fmt.log
set -o pipefail; make lint 2>&1 | tee /tmp/7-1-2-lint.log
set -o pipefail; make test 2>&1 | tee /tmp/7-1-2-test.log
```

If new snapshots are introduced and `cargo-insta` is unavailable, use the
existing project convention:

```sh
set -o pipefail; INSTA_UPDATE=always cargo test -p weaver-cards -- snapshot \
  2>&1 | tee /tmp/7-1-2-insta.log
```

Implementation is complete only when all logs end in successful exit codes and
the new happy-path plus refusal-path tests are present.
