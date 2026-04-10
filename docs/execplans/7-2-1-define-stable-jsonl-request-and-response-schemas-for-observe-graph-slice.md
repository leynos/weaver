# 7.2.1 Define stable JSON Lines (JSONL) request and response schemas for `observe graph-slice`

This ExecPlan (execution plan) is a living document. The sections
`Constraints`, `Tolerances`, `Risks`, `Progress`, `Surprises & Discoveries`,
`Decision Log`, and `Outcomes & Retrospective` must be kept up to date as work
proceeds.

Status: DRAFT

This document must be maintained in accordance with `AGENTS.md` at the
repository root.

## Purpose / big picture

After this change, Weaver will have one stable, serde-annotated contract for
`observe graph-slice` that downstream code can treat as the source of truth. A
caller will be able to request a bounded slice rooted at a symbol, specify
budget limits explicitly, and receive a deterministic JSON response that always
describes the applied constraints, the cards and edges that fit inside those
constraints, and any spillover that was excluded when the traversal truncated.

Observable behaviour after implementation:

1. `weaver observe graph-slice --uri <Uniform Resource Identifier (URI)>
   --position <LINE:COL>` parses into a typed request with explicit default
   values for `depth`, `direction`, `edge_types`, `min_confidence`,
   `budget.max_cards`, `budget.max_edges`, `budget.max_estimated_tokens`,
   `entry_detail`, and `node_detail`.
2. Stable JSON snapshots lock at least one successful slice, one
   truncated slice with spillover metadata, and one structured refusal.
3. Every serialized edge reports its resolution scope as exactly one of
   `full_symbol_table`, `partial_symbol_table`, or `lsp` (Language Server Protocol).
4. End-to-end snapshots exercise a dedicated 20-case Rust graph-slice
   battery and a dedicated 20-case Python graph-slice battery through the real
   `weaver` binary using `assert_cmd` and `insta`, requesting semantic detail
   via `--entry-detail semantic --node-detail semantic`.
5. `make check-fmt`, `make lint`, and `make test` exit `0`, and the
   documentation gates `make markdownlint` and `make nixie` also exit `0`
   because this task changes Markdown.

This plan covers roadmap item 7.2.1 in
[docs/roadmap.md](/home/user/project/docs/roadmap.md).

## Constraints

1. The design source of truth is
   [docs/jacquard-card-first-symbol-graph-design.md](/home/user/project/docs/jacquard-card-first-symbol-graph-design.md),
    especially the edge model, slice request shape, spillover semantics, and
   the `observe graph-slice` surface in sections 12.1 through 12.3. The
   implementation may clarify ambiguities, but every clarification must be
   recorded back into that design document.
2. This roadmap item is the schema milestone, not the full traversal
   milestone. Real two-pass Tree-sitter edge extraction belongs to 7.2.2, and
   LSP call-edge expansion belongs to 7.2.3. The code added here must define
   the public contract cleanly without forcing completion of those later
   milestones.
3. Reuse existing stable card types from `crates/weaver-cards/` rather
   than creating duplicate card or detail enums. The graph-slice schema should
   compose `SymbolCard` and `DetailLevel`, not fork them.
4. Follow the existing repository pattern for stable observe contracts:
   `weaver-cards` owns serde schema types, request parsing, snapshot tests,
   and schema-level behaviour-driven development (BDD) coverage, while
   `weaverd` owns transport, dispatch, and filesystem concerns.
5. `weaver-cards` already has the serde and testing infrastructure needed
   for this work. Keep `weaver-graph` focused on graph construction and
   provider logic unless a later milestone explicitly needs to expose new
   internal graph-building APIs from that crate.
6. Every Rust module must begin with a `//!` module comment, public items
   must have rustdoc comments, code files must stay under 400 lines, and Clippy
   warnings remain denied workspace-wide.
7. Behaviour tests must use `rstest-bdd` v0.5.0. End-to-end CLI coverage
   must use `assert_cmd` and `insta`.
8. Documentation updates are part of the feature:
   [docs/jacquard-card-first-symbol-graph-design.md](/home/user/project/docs/jacquard-card-first-symbol-graph-design.md),
   [docs/users-guide.md](/home/user/project/docs/users-guide.md), and
   [docs/roadmap.md](/home/user/project/docs/roadmap.md) must all be updated
   before the work is complete.
9. The roadmap checkbox for 7.2.1 must not be marked done until all code,
   tests, e2e snapshots, documentation, and validation gates pass.

## Tolerances

- If satisfying the required e2e coverage would force real multi-file
  traversal or call-hierarchy expansion that materially overlaps 7.2.2 or
  7.2.3, stop and re-confirm scope before proceeding. The acceptable overlap
  for 7.2.1 is only the minimum needed to exercise the public schema
  deterministically.
- If the public JSON field names in the Jacquard design cannot be made
  internally consistent without changing the roadmap acceptance contract, stop
  and present the mismatch explicitly. The current design text uses both
  `resolution` and the roadmap phrase `resolution scope`.
- If the implementation grows beyond roughly 18 touched files before the
  e2e fixtures are added, split the work into smaller modules before continuing.
- If any single planned Rust file exceeds 350 lines, split it immediately.
- If `make test` hangs or blocks on the Cargo build lock, inspect the log
  and background processes before changing code unrelated to the feature.

## Risks

- Risk: the schema-only milestone may accidentally leak traversal policy
  into the public contract. Mitigation: lock only budget inputs, response
  ordering, spillover metadata, edge provenance, and refusal shapes here; keep
  extraction and ranking heuristics for later milestones.
- Risk: `weaver-cards` already carries `get-card` responsibilities, so
  graph-slice types could make it sprawl. Mitigation: add a dedicated
  `graph_slice/` module tree instead of overloading the existing `card`,
  `request`, and `response` modules.
- Risk: end-to-end fixture coverage for graph slices needs richer
  workspaces than the existing single-file get-card fixtures. Mitigation: build
  a dedicated graph-slice fixture catalogue and reuse only the existing harness
  and any graph-shaped seeds.
- Risk: snapshot churn from timestamps, URIs, and generated IDs can make
  e2e output noisy. Mitigation: extend the existing
  `crates/weaver-e2e/tests/test_support/mod.rs` normalization helpers for
  slice-specific fields before recording snapshots.
- Risk: `cargo-insta` may not be installed locally. Mitigation: use
  `INSTA_UPDATE=always` for the first snapshot-acceptance run, matching the
  existing project practice.

## Progress

- [x] (2026-04-02 00:00Z) Reviewed the roadmap entry, Jacquard design,
      existing `get-card` ExecPlans, the `execplans` skill, and the
      project testing/documentation guidance.
- [x] (2026-04-02 00:00Z) Inspected the current codebase state in
      `crates/weaverd`, `crates/weaver-cards`, `crates/weaver-graph`,
      `crates/weaver-e2e`, and `docs/users-guide.md`.
- [x] (2026-04-02 00:00Z) Drafted this ExecPlan.
- [ ] Stage A: add schema modules and exports in `crates/weaver-cards/`.
- [ ] Stage B: lock request defaults, budget semantics, spillover metadata,
      and edge resolution scope with unit tests and snapshots.
- [ ] Stage C: expose `observe graph-slice` through `weaverd` and CLI
      discoverability with typed request parsing and structured responses.
- [ ] Stage D: add `rstest-bdd` happy-path, unhappy-path, and edge-case
      coverage.
- [ ] Stage E: add `assert_cmd` plus `insta` end-to-end coverage using the
      dedicated 20 Rust and 20 Python graph-slice fixture batteries.
- [ ] Stage F: update design and user documentation, mark the roadmap
      entry done, and run all required gates.

## Surprises & discoveries

- There is no `observe graph-slice` implementation yet anywhere in the
  codebase. The current observe router in
  [crates/weaverd/src/dispatch/router.rs](/home/user/project/crates/weaverd/src/dispatch/router.rs)
   only recognizes `get-definition`, `find-references`, `grep`, `diagnostics`,
  `call-hierarchy`, and `get-card`.
- `crates/weaver-cards/` already matches the stable-contract pattern used
  for `observe get-card`: request parsing, serde schema types, snapshot tests,
  and schema-focused `rstest-bdd` coverage already live there.
- `crates/weaver-graph/` is currently an internal graph/provider crate,
  not a public JSONL contract crate. Moving the stable observe schema into it
  would couple graph-engine internals to daemon-facing transport types.
- `crates/weaver-e2e/` already contains a reusable battery of 20 Rust
  fixtures and 20 Python fixtures for symbol-centric `get-card` testing under
  `src/card_fixtures/`, plus a test daemon harness that already drives the real
  `weaver` binary through `assert_cmd`.
- Those existing `card_fixtures` are optimized for single-file symbol-card
  extraction and are not rich enough to serve as the primary graph-slice e2e
  catalogue. Graph-slice needs connected workspaces that can express `call`,
  `import`, and `config` edges plus budget truncation.
- The design document's examples use the field name `resolution`, while the
  roadmap acceptance criteria say "edges carry resolution scope". This must be
  normalised in the public schema and reflected back into the design document.
- The design document's example response flattens `max_cards` and
  `max_estimated_tokens` under `constraints`, but the roadmap item speaks about
  `budget` semantics explicitly. This plan resolves that by making the budget
  an explicit nested object in the typed request and response.

## Decision Log

- Decision: own the stable graph-slice schema in
  [crates/weaver-cards/](/home/user/project/crates/weaver-cards/), not in
  `weaverd` and not in `weaver-graph`. Rationale: this repo already uses
  `weaver-cards` as the stable observe-contract crate, while `weaver-graph`
  remains an internal engine/provider crate.
- Decision: add a dedicated module tree such as
  `crates/weaver-cards/src/graph_slice/{mod,budget,request,response}.rs`.
  Rationale: it keeps the existing `get-card` code stable, avoids oversized
  files, and makes the slice contract easy to reason about in isolation.
- Decision: use `resolution_scope` as the public JSON field name on edges.
  Rationale: it matches the roadmap acceptance wording and is clearer than the
  ambiguous design-example field name `resolution`.
- Decision: model `budget` as its own nested struct in both the request and
  the response `constraints`. Rationale: the budget is a first-class part of
  the public contract, and nesting it avoids a grab-bag of top-level integers.
- Decision: always serialize the normalized constraints explicitly in the
  success response, even when the caller omitted optional flags. Rationale:
  this makes default semantics observable and snapshot-stable.
- Decision: always include a `spillover` object in successful responses,
  with `truncated: false` when nothing was dropped. Rationale: this keeps the
  response shape consistent while still satisfying the acceptance criterion
  that spillover metadata is present when traversal truncates.
- Decision: reuse `DetailLevel` from `weaver-cards` for `entry_detail` and
  `node_detail`. Rationale: it avoids parallel detail enums and keeps the
  card/slice surfaces aligned.
- Decision: record all of these schema decisions back into
  [docs/jacquard-card-first-symbol-graph-design.md](/home/user/project/docs/jacquard-card-first-symbol-graph-design.md)
   during implementation so the design stays authoritative.

## Outcomes & retrospective

Successful completion will mean:

1. `weaver-cards` exports stable schema types for `GraphSliceRequest`,
   `GraphSliceResponse`, `GraphSlice`, `SliceBudget`, typed slice edges, edge
   provenance, spillover metadata, and refusal payloads.
2. The daemon and CLI recognize `observe graph-slice`, and the transport
   path emits schema-valid JSON instead of an ad hoc string.
3. Snapshot tests lock budget defaults, truncation metadata, and edge
   resolution scope.
4. BDD coverage demonstrates happy paths, invalid-argument refusals, and
   truncation edge cases.
5. End-to-end snapshots cover the requested 20 Rust and 20 Python
   graph-slice scenarios at semantic detail.
6. The design doc, user guide, and roadmap all match the shipped contract.

## Context and orientation

The feature touches four main areas.

1. [crates/weaver-cards/](/home/user/project/crates/weaver-cards/)

   This is the intended home for the graph-slice schema. It already owns the
   stable `observe get-card` contract, request parsing, snapshots, and
   schema-oriented behaviour tests. The graph-slice types should extend that
   existing pattern.

2. [crates/weaver-graph/](/home/user/project/crates/weaver-graph/)

   This crate remains the likely home for later graph-building engines and
   provider integrations. It should not become the daemon-facing JSONL contract
   crate in 7.2.1.

3. [crates/weaverd/](/home/user/project/crates/weaverd/)

   The daemon must parse the new request, route the new observe operation, and
   serialize typed responses through the existing JSONL transport.

4. [crates/weaver-e2e/](/home/user/project/crates/weaver-e2e/)

   This crate already contains a reusable CLI daemon harness plus the
   `get-card` fixture catalogue. Graph-slice e2e work should reuse the harness
   but add a dedicated `graph_slice_fixtures/` catalogue for connected
   workspace topologies.

The closest completed analogue is roadmap item 7.1.1 for `observe get-card`.
That work created stable schema types in `weaver-cards`, snapshot tests, BDD
coverage, a daemon route, documentation, and roadmap updates. This plan follows
the same delivery shape.

## Plan of work

### Stage A: Add the graph-slice schema surface to `weaver-cards`

Create a dedicated `graph_slice/` module tree under
[crates/weaver-cards/src/](/home/user/project/crates/weaver-cards/src/) and
re-export the public types from `lib.rs`.

The module split should stay narrow and explicit:

- `graph_slice/budget.rs`
  Defines `SliceBudget` and any helper constructors/defaults.
- `graph_slice/request.rs`
  Defines `GraphSliceRequest`, request parsing, flag validation, and the
  normalized default semantics.
- `graph_slice/response.rs`
  Defines `GraphSliceResponse`, `GraphSlice`, `SliceConstraints`,
  `SliceSpillover`, refusal types, typed edges, and provenance structs.
- `graph_slice/mod.rs`
  Re-exports the public slice schema surface.

Add the needed dependencies to
[crates/weaver-cards/Cargo.toml](/home/user/project/crates/weaver-cards/Cargo.toml):

- `weaver-graph` only if a shared graph-domain type is genuinely needed

Use the existing serde/test dependencies already present in `weaver-cards`.
Only add the graph-layer dependency that is genuinely needed for shared types
or conversions, and only if it does not create a circular boundary.

The minimum request contract should include:

- `uri`
- `line`
- `column`
- `depth`
- `direction`
- `edge_types`
- `min_confidence`
- `budget`
- `entry_detail`
- `node_detail`

The minimum success response contract should include:

- `slice_version`
- `entry`
- `constraints`
- `cards`
- `edges`
- `spillover`

The edge contract should include:

- `edge_version`
- `type`
- `from`
- `to` or `to_external`
- `confidence`
- `direction`
- `resolution_scope`
- `provenance`

Normalize all unordered request inputs into deterministic output order.
Concretely:

1. Canonicalize `edge_types` into stable enum order:
   `call`, `import`, `config`.
2. Serialize cards in stable order with the entry card first, then by
   symbol identity.
3. Serialize edges in stable order using a total ordering over type,
   source, target, direction, and provenance source.

### Stage B: Lock the contract with unit tests and snapshots first

Before wiring the daemon path, add the contract tests in `weaver-cards` that
fail red-first and prove the public shape is stable.

Add unit coverage for:

1. Request parsing with all flags present.
2. Omitted optional flags resolving to the documented defaults.
3. Invalid `--position`, `--depth`, `--direction`, `--edge-types`,
   `--min-confidence`, and budget values.
4. Duplicate or unsorted `--edge-types` input becoming canonical output.
5. Edge serialization for all three `resolution_scope` values.
6. Spillover serialization when `truncated` is `true` and when it is
   `false`.

Add snapshot coverage for at least:

1. A default-budget success response.
2. A truncated success response that includes spillover metadata.
3. A success response that shows `full_symbol_table`,
   `partial_symbol_table`, and `lsp` edge resolution scopes.
4. A refusal response.

Use the existing project snapshot pattern:

```plaintext
INSTA_UPDATE=always cargo test -p weaver-cards snapshot
```

If `assert_json_snapshot!` is unavailable in the current workspace
configuration, use `assert_snapshot!` over `serde_json::to_string_pretty(...)`
as the project already does elsewhere.

### Stage C: Expose `observe graph-slice` through the daemon path

Add the new operation to the observe router and daemon dispatch code.

Expected files:

- [crates/weaverd/src/dispatch/router.rs](/home/user/project/crates/weaverd/src/dispatch/router.rs)
- [crates/weaverd/src/dispatch/observe/mod.rs](/home/user/project/crates/weaverd/src/dispatch/observe/mod.rs)
- a new
  [crates/weaverd/src/dispatch/observe/graph_slice.rs](/home/user/project/crates/weaverd/src/dispatch/observe/graph_slice.rs)
- CLI discoverability surfaces in
  [crates/weaver-cli/src/discoverability.rs](/home/user/project/crates/weaver-cli/src/discoverability.rs)
   and any related help/localization files

This milestone should keep the runtime narrow:

1. Parse the request through `GraphSliceRequest`.
2. Produce schema-valid JSON responses through typed structs.
3. Avoid committing to the full traversal implementation before 7.2.2.

If a minimal deterministic schema exercise path is needed to satisfy the
required daemon and e2e coverage, keep it intentionally thin and explicit. It
must only exercise the public contract and must not become an accidental second
traversal engine.

### Stage D: Add behavioural tests with `rstest-bdd` v0.5.0

Add BDD coverage in `weaver-cards` and, if needed for transport semantics, in
`weaverd`.

Recommended files:

- [crates/weaver-cards/src/tests/graph_slice_behaviour.rs](/home/user/project/crates/weaver-cards/src/tests/graph_slice_behaviour.rs)
- [crates/weaver-cards/tests/features/graph_slice_schema.feature](/home/user/project/crates/weaver-cards/tests/features/graph_slice_schema.feature)
- [crates/weaverd/src/tests/graph_slice_behaviour.rs](/home/user/project/crates/weaverd/src/tests/graph_slice_behaviour.rs)
- [crates/weaverd/tests/features/graph_slice.feature](/home/user/project/crates/weaverd/tests/features/graph_slice.feature)

The behaviour matrix should cover:

1. Happy path: a valid request serializes with default budgets and
   normalized edge types.
2. Happy path: a truncated response includes `spillover.truncated = true`
   plus the expected spillover metadata.
3. Unhappy path: invalid depth or invalid confidence is rejected.
4. Unhappy path: an unknown edge type is rejected.
5. Edge case: zero or duplicate budget values are handled deterministically.
6. Edge case: responses preserve the exact `resolution_scope` enum strings.

### Stage E: Add end-to-end snapshots with the Rust and Python fixture batteries

Extend the existing end-to-end harness instead of inventing another one.

Expected files:

- a new
  [crates/weaver-e2e/tests/graph_slice_snapshots.rs](/home/user/project/crates/weaver-e2e/tests/graph_slice_snapshots.rs)
- shared helper additions in
  [crates/weaver-e2e/tests/test_support/mod.rs](/home/user/project/crates/weaver-e2e/tests/test_support/mod.rs)
- a dedicated
  [crates/weaver-e2e/src/graph_slice_fixtures/](/home/user/project/crates/weaver-e2e/src)
   module tree for graph-shaped workspaces

The e2e suite must cover:

1. 20 Rust scenarios.
2. 20 Python scenarios.
3. Requests issued via the real `weaver` binary.
4. Semantic detail requested explicitly with
   `--entry-detail semantic --node-detail semantic`.
5. Snapshot normalization for timestamps, URIs, symbol IDs, etags, and any
   future spillover frontier identifiers.

Recommended approach:

1. Build a dedicated graph-slice fixture catalogue with connected Rust and
   Python workspaces.
2. Reuse the existing daemon/snapshot harness and any useful graph-shaped
   seeds already present in `weaver-e2e`, but do not rely on the single-file
   `card_fixtures` battery as the primary graph-slice dataset.
3. Keep snapshot names aligned with the fixture identifiers so regressions are
   easy to track by scenario.

### Stage F: Update the design doc, user guide, and roadmap

Update the design doc first so the contract decisions are recorded where future
milestones will find them.

The design doc must explicitly state:

1. the public field name `resolution_scope`,
2. the nested `budget` object,
3. default values for every budget and traversal flag,
4. the rule that success responses always echo normalized constraints, and
5. the rule that `spillover` is always present and becomes populated when
   truncation occurs.

Update the user guide with:

1. syntax for `observe graph-slice`,
2. each CLI flag and its default,
3. success and refusal JSON examples,
4. the meaning of spillover and truncation, and
5. how semantic detail interacts with cards embedded in the slice.

Only after code, tests, and docs all match should the roadmap entry be checked
off in [docs/roadmap.md](/home/user/project/docs/roadmap.md).

### Stage G: Run the full validation and commit gates

Because this task changes Rust and Markdown, run all six gates with `pipefail`
and `tee`:

```plaintext
set -o pipefail; make fmt 2>&1 | tee /tmp/7-2-1-make-fmt.log
set -o pipefail; make markdownlint 2>&1 | tee /tmp/7-2-1-make-markdownlint.log
set -o pipefail; make nixie 2>&1 | tee /tmp/7-2-1-make-nixie.log
set -o pipefail; make check-fmt 2>&1 | tee /tmp/7-2-1-make-check-fmt.log
set -o pipefail; make lint 2>&1 | tee /tmp/7-2-1-make-lint.log
set -o pipefail; make test 2>&1 | tee /tmp/7-2-1-make-test.log
```

Review the tail of each log before concluding that the milestone is done.

## Approval gate

This ExecPlan is complete as a draft. Per the `execplans` skill, the next step
after drafting is explicit user approval before implementation begins. No code
changes beyond this plan document should be made until that approval is given.
