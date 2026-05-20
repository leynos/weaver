# Weaver roadmap

This roadmap translates `docs/weaver-design.md`,
`docs/adr-007-agent-native-command-surface.md`,
`docs/sempai-query-language-design.md`,
`docs/jacquard-card-first-symbol-graph-design.md`, `docs/rfcs/0001-o11y.md`,
and the existing ADR set into an outcome-oriented delivery sequence. It does
not promise dates. Phases carry testable product ideas, steps validate or
falsify those ideas, and tasks are review-sized execution units with explicit
dependencies and observable success criteria.

The current forward plan is the source of truth for future work. The historical
ledger in [`docs/archive/prototype-roadmap.md`](archive/prototype-roadmap.md)
preserves task numbers `1` through `11` as provenance. Relevant unfinished
archive work has been moved into this live roadmap under resource-first command
names; prototype `observe`, `act`, and `verify` spellings are not future public
grammar unless a live task explicitly reintroduces them.

The plan is intentionally validation-led. Each phase after the dependency
boundary is a product hypothesis that can fail: the result may graduate,
narrow, or defer a design rather than forcing every archived idea into the core
release. The archive remains useful evidence, but the live sequence favours
vertical slices such as "read a symbol", "select with Sempai and compose with
cards", "mutate from the same selectors", and "explain impact" over building
one horizontal layer at a time.

## 12. Dependency boundaries and archive assessment

Idea: if Weaver consumes reusable command-contract machinery from OrthoConfig
and treats the prototype archive as evidence rather than a second backlog, the
product can move quickly without rebuilding generic CLI infrastructure or
losing completed work.

This phase is the minimum unavoidable foundation. It does not validate a user
workflow directly; it validates the build boundary and prevents later slices
from rediscovering the same dependency and migration questions.

### 12.1. Confirm reusable contracts that Weaver must not duplicate

This step answers which generic command-contract pieces come from OrthoConfig
and which temporary Weaver adapters are allowed. Its outcome informs every
command-surface and renderer task. See ADR 007 and the OrthoConfig roadmap.

- [ ] 12.1.1. Track the downstream consumer boundary.
  - Depends on OrthoConfig 5.2.3.
  - Success: every command-contract task says whether it consumes OrthoConfig,
    wraps it temporarily, or records a deliberate divergence in ADR 007.
- [ ] 12.1.2. Consume recursive command metadata.
  - Depends on OrthoConfig 6.1.1 and 6.1.2.
  - Success: generated help, manpage, completion, and context output converge
    on the OrthoConfig recursive metadata shape.
- [ ] 12.1.3. Consume compact context and skill metadata.
  - Depends on OrthoConfig 6.2.1 through 6.3.2.
  - Success: Weaver-specific capability fields extend the reusable context and
    skill shapes instead of forking them.
- [ ] 12.1.4. Consume canonical vocabulary and behavioural metadata.
  - Depends on OrthoConfig 7.1.1 through 7.2.7 and 8.1.1 through 8.1.2.
  - Success: renderer, JSON, exit-code, bounded-list, mutation,
    non-interactive, capability, provenance, and reference-CLI checks point at
    OrthoConfig-owned contracts where those contracts exist.
- [ ] 12.1.5. Consume compounding primitive contracts.
  - Depends on OrthoConfig 9.1.1 through 9.3.3.
  - Success: profile, delivery, feedback, and execution-ledger command
    semantics are Weaver-owned, while reusable parsing, redaction, metadata,
    and ledger vocabulary come from OrthoConfig.

### 12.2. Assess the prototype ledger and migrate active scope

This step answers which archived tasks still matter to the product after the
0.1.0 command reset. Its outcome prevents archived prototype work from becoming
either hidden scope or a competing implementation plan.

- [x] 12.2.1. Classify archive tasks as shipped foundation, migrated product
      scope, or superseded prototype grammar.
  - See `docs/archive/prototype-roadmap.md`.
  - The archive relevance matrix maps each archived step to a live destination
    or a supersession reason.
  - Success: every unchecked archive step has a live roadmap destination or an
    explicit reason it is no longer active implementation guidance.
- [x] 12.2.2. Preserve completed prototype foundation as implementation
      evidence.
  - Requires 12.2.1.
  - Success: completed archive work for configuration, daemon transport, JSONL,
    sandboxing, Double-Lock safety, LSP hosting, plugin routing, cards, graph
    scaffolding, Sempai normalization, and patch application is referenced by
    the live tasks that reuse it.
- [x] 12.2.3. Mark prototype command spellings as provenance only.
  - Requires 12.2.1.
  - Success: unchecked archive tasks that mention `observe`, `act`, `verify`,
    provider-first commands, or root `--output` are not executable backlog
    items; their product intent is carried by resource-first live tasks.

## 13. Command contract proving slice

Idea: if Weaver can expose one tiny resource-first command loop through the
generated command contract, with human and machine renderers, introspection,
help, localization, and drift gates, later semantic slices can reuse the same
surface instead of rebuilding command grammar.

This phase validates the command contract with the smallest useful public
slice: a read-only semantic query. It deliberately pulls relevant archive work
from help, localization, operation guidance, manpage generation, capability
introspection, and agent-grade CLI hardening into one acceptance surface.

### 13.1. Ratify the reset boundary and command-surface adapter

This step answers whether Weaver-specific semantic metadata can sit on top of
OrthoConfig command contracts without duplicating them. See ADR 007 and
`docs/weaver-design.md` §§2.1.1-2.1.4.

- [x] 13.1.1. Record the agent-native command-surface reset as ADR 007.
  - Records the forward OrthoConfig dependency boundary that 12.1 validates.
  - Success: ADR 007 defines the dual renderer contract, capability routing,
    OrthoConfig dependencies, and lack of compatibility promise for the
    prototype grammar.
- [x] 13.1.2. Implement the Weaver command-surface adapter for one read-only
      command family.
  - Requires 13.1.1 and depends on OrthoConfig 5.2.3, 6.1, and 7.2.7.
  - Start with `definitions get` plus the metadata needed to add
    `references list` without a second path.
  - Success: resource path, verb, capability ID, selector forms, output
    schemas, error schemas, mutability, provider policy, examples, and skill
    references all flow from one adapter record.
- [x] 13.1.3. Define the temporary-adapter removal policy.
  - Requires 13.1.2.
  - Success: every local generic helper names the OrthoConfig task expected to
    replace it or records a permanent divergence in ADR 007.

### 13.2. Prove the dual renderer on a real command

This step answers whether the same command contract can serve humans and agents
without forking command behaviour. It migrates prototype archive work 3.2.2
through 3.3.4 and 11.3.1 through 11.3.4. See `docs/weaver-design.md` §§2.1.3,
2.1.7, 2.1.10, and 2.1.11.

- [ ] 13.2.1. Implement the localized human renderer for `definitions get`.
  - Requires 13.1.2 and depends on OrthoConfig 7.2.2.
  - Include `--plain`, `--color`, `--no-pager`, `--width`, TTY-sensitive
    progress, table headings, narrow-width labelled blocks, and ASCII
    fallbacks.
  - Success: human output does not rely on colour alone and never emits pager
    or spinner control flow outside terminal contexts.
- [ ] 13.2.2. Implement universal `--json` and structured errors for
      `definitions get`.
  - Requires 13.2.1 and depends on OrthoConfig 7.2.3 through 7.2.5 and 8.1.
  - Success: success JSON is parseable on stdout, failure JSON is parseable on
    stderr, field names and error codes are non-localized, and exit classes are
    stable.
- [ ] 13.2.3. Implement enumerating errors and bounded responses for the
      command-family pilot.
  - Requires 13.2.2 and depends on OrthoConfig 7.2.6.
  - Success: invalid selectors, languages, providers, detail levels, and
    capability IDs enumerate valid values; collection outputs expose bounds,
    cursors, truncation markers, and narrowing hints.

### 13.3. Validate discoverability and drift prevention

This step answers whether humans and agents can discover the command contract
from generated surfaces instead of hard-coded catalogues. It migrates prototype
archive work 3.2.3 through 3.2.6, 5.7.1 through 5.7.5, and 11.3.2. See
`docs/weaver-design.md` §§2.1.4 and 6.1.

- [ ] 13.3.1. Implement `weaver context --json` for the pilot command family.
  - Requires 13.2.3 and depends on OrthoConfig 6.2.1 through 6.2.3.
  - Success: context output includes schema version, commands, flags, enum
    values, output schemas, error taxonomy, capabilities, selected-provider
    summaries, and skill paths.
- [ ] 13.3.2. Implement `weaver capabilities list --json` from runtime
      provider state.
  - Requires 13.3.1.
  - Success: capability availability is separated from full command context
    and includes deterministic provider selection rationale.
- [ ] 13.3.3. Implement `weaver help`, command help, manpage input, shell
      completions, and `weaver skill-path` from the same metadata.
  - Requires 13.3.1 and depends on OrthoConfig 6.3 and 8.1.
  - Success: generated references, localized help, skills, and completion
    fixtures fail CI when they mention unknown commands or flags.
- [ ] 13.3.4. Add command-surface drift gates for the pilot.
  - Requires steps 13.1-13.3.
  - Success: CI rejects schema, router, help, docs, localization, context,
    skill, vocabulary, stdout/stderr, and JSON-schema drift for the pilot
    command family.

### 13.4. Prove the local daemon observability contract

This step answers whether local users and automation can diagnose daemon
startup, request, transport, and lifecycle failures without adding a metrics
endpoint or distributed tracing backend. It converts RFC 0001 into bounded
local signals that support later read, mutation, and workflow slices. See
`docs/rfcs/0001-o11y.md` §§"Observability primitives", "Failure modes that
warrant actionable signals", "Delivery mechanisms", and "Acceptance criteria".

- [ ] 13.4.1. Define canonical daemon event names and structured fields.
  - Requires 13.2.2 and 13.2.3.
  - Success: lifecycle, request, dispatch, and listener events have stable
    names and the minimum diagnostic fields required by RFC 0001.
- [ ] 13.4.2. Emit bounded structured events for daemon lifecycle and
      transport paths.
  - Requires 13.4.1.
  - Success: lifecycle, dispatch, request rejection, socket, and listener
    paths emit deterministic `tracing` fields without sensitive payload data.
- [ ] 13.4.3. Unify CLI guidance for pre-daemon and transport failures.
  - Requires 13.4.2 and 13.3.2.
  - Success: pre-daemon and transport failures report what failed, the relevant
    evidence path or endpoint, and the next useful operator action.
- [ ] 13.4.4. Make request-size rejections diagnosable on both sides of the
      daemon boundary.
  - Requires 13.4.2.
  - See `docs/rfcs/0001-o11y.md` §"RequestTooLarge rejection".
  - Success: CLI-side and daemon-side size failures include the observed
    request size, `JSONL_REQUEST_MAX_LINE_BYTES`, the affected command where
    known, and user guidance to reduce or split the payload.
- [ ] 13.4.5. Bound `weaverd.health` retention and stale-state handling.
  - Requires 13.4.2.
  - See `docs/rfcs/0001-o11y.md` §"Health snapshot".
  - Success: health persistence is bounded, rotates deterministically, and
    treats out-of-window data as stale.
- [ ] 13.4.6. Cover RFC 0001 failure modes in documentation and regression
      tests.
  - Requires 13.4.2 through 13.4.5.
  - Success: tests and docs cover the RFC 0001 failure taxonomy and foreground
    debug recipe.

## 14. Code-reading loop slice

Idea: if Weaver can answer common code-reading questions through the new
command surface, using existing LSP, Tree-sitter, cards, and bounded graph
foundations, the redesign proves immediate value before advanced query or
mutation work lands.

This phase validates the first real product workflow: ask where a symbol is,
where it is used, what diagnostics surround it, and what compact context an
agent should read next. It migrates archive work from LSP command parity,
cards-first context, same-file graph slices, and agent-grade compact output.

### 14.1. Prove LSP-backed resource commands

This step answers whether existing semantic backends fit the resource-first
surface without provider-specific commands. It migrates prototype archive work
10.1.1 through 10.3.2. See `docs/weaver-design.md` §§2.2, 3.1, and 6.1.

- [ ] 14.1.1. Implement `weaver definitions get`.
  - Requires phase 13.
  - Success: position references return localized human output by default and
    stable JSON under `--json`, with provider provenance in machine output.
- [ ] 14.1.2. Implement `weaver references list`.
  - Requires 14.1.1.
  - Success: reference lists are bounded, cursor-aware, provider-provenanced,
    and suitable for downstream selector processing.
- [ ] 14.1.3. Implement `weaver diagnostics list`.
  - Requires 14.1.1.
  - Success: diagnostics preserve source ranges, severity, provider
    provenance, and actionable error classes in both renderer modes.
- [ ] 14.1.4. Add combinatorial read-command E2E coverage.
  - Requires 14.1.1 through 14.1.3.
  - Success: one suite covers human output, `--json`, `--plain`, bounded
    output, invalid selectors, invalid enum values, missing capabilities, and
    provider-unavailable cases.

### 14.2. Prove card-first context is useful before full graph traversal

This step answers whether compact symbol cards are enough to guide a user or
agent to the next useful read. It migrates prototype archive work 7.1.1 through
7.1.4, 9.2.1 through 9.2.3, and 10.2.1 through 10.2.2. See
`docs/jacquard-card-first-symbol-graph-design.md` §§5-11.

- [ ] 14.2.1. Implement `weaver cards get` for position references.
  - Requires 14.1.1 and reuses prototype archive work 7.1.1 through 7.1.4.
  - Success: cards expose stable JSON, bounded enrichment, detail levels,
    cache provenance, and useful human summaries.
- [ ] 14.2.2. Add qualified symbol selectors to `cards get`.
  - Requires 14.2.1.
  - Success: symbol, path, and container qualifiers disambiguate common cases;
    ambiguous selectors fail with enumerating alternatives.
- [ ] 14.2.3. Add one-hop relation summaries to cards.
  - Requires 14.2.2 and reuses prototype archive work 3.1.4 and 9.3.1.
  - Success: cards can include bounded callers, callees, imports, dependencies,
    or dependents without invoking a full graph-slice command.

### 14.3. Decide whether static search belongs in the first code-reading loop

This step answers whether structural grep should graduate now or wait for the
Sempai selector slice. It migrates the product intent of prototype archive work
10.4.1 through 10.4.2 and 5.5.1 without forcing provider-specific commands.

- [ ] 14.3.1. Prototype `weaver symbols list --pattern` over `weaver-syntax`
      and optional `srgn`.
  - Requires 13.3.2 and 14.1.1.
  - Success: the prototype either returns bounded selector records across Rust,
    Python, and TypeScript, or records why pattern search must wait for the
    Sempai slice.
- [ ] 14.3.2. Decide whether the static-search pilot graduates into the live
      command contract.
  - Requires 14.3.1.
  - Success: ADR 007 or the roadmap records one of three outcomes: graduate as
    `symbols list --pattern`, fold into `symbols list --query`, or defer.

## 15. Sempai selector-to-context slice

Idea: if Sempai one-liners can select symbols and immediately feed cards plus
one-hop graph context, Weaver proves that query language work pays off as a
composable product loop rather than as a standalone parser layer.

This phase validates Sempai as a selector engine without reducing the original
backend and integration scope to a vague parser task. It migrates archive work
from Sempai DSL parsing, Tree-sitter execution, query routing, symbol-first
cards, and the Sempai-to-Jacquard vertical slice.

### 15.1. Prove a minimal one-liner selector with honest diagnostics

This step answers whether the target one-liner grammar can select real symbols
without overbuilding the full query engine. It migrates prototype archive work
4.1.6 through 4.1.7, 4.3.3, and 9.1.1. See
`docs/sempai-query-language-design.md` §§3-6.

- [ ] 15.1.1. Implement one-liner tokenization and Pratt parsing for positive
      symbol patterns.
  - Requires 13.2.2.
  - Success: valid one-liners compile to canonical formula form, malformed
    input produces stable `E_SEMPAI_*` diagnostics, and recovery preserves
    partial anchors where safe.
- [ ] 15.1.2. Define selector record schemas for one-liner matches.
  - Requires 15.1.1.
  - Success: selector records carry identity, range, language, query, capture,
    confidence, and provider provenance needed for safe downstream context or
    mutation.

### 15.2. Prove the Sempai Tree-sitter backend

This step answers whether Semgrep-style matching can run accurately enough on
real syntax with bounded execution semantics. It preserves prototype archive
work 4.2.1 through 4.2.12 instead of compressing the backend into a single
"matching" task. See `docs/sempai-query-language-design.md` §§7-12.

- [ ] 15.2.1. Implement language profiles and wrapper registry for Rust,
      Python, TypeScript, and Go, with optional HashiCorp Configuration
      Language (HCL) support.
  - Requires 15.1.2.
  - Success: Rust, Python, TypeScript, and Go profiles each define wrapper
    templates, list-shape mappings, and rewrite boundaries; optional HCL loads
    only behind its feature flag; profile selection failures return
    deterministic diagnostics; and fixtures validate all profile
    registrations.
- [ ] 15.2.2. Implement Semgrep-token rewrite logic with language-safe
      boundaries for metavariables, ellipsis, and deep ellipsis.
  - Requires 15.2.1.
  - Success: rewrite logic avoids substitutions in unsafe lexical regions and
    produces deterministic placeholder mappings.
- [ ] 15.2.3. Compile rewritten snippets into `PatNode`-based pattern
      intermediate representation (IR) with span traceability.
  - Requires 15.2.2.
  - Success: compiled IR snapshots are stable, and wrapper/root extraction
    metadata is preserved for diagnostics.
- [ ] 15.2.4. Implement node-kind matching and metavariable unification over
      Tree-sitter syntax trees.
  - Requires 15.2.3.
  - Success: repeated metavariables unify across compatible nodes, mismatches
    fail deterministically, and the first-language vertical fixtures from
    archive work 9.1.2 through 9.1.3 pass through the same matcher.
- [ ] 15.2.5. Implement list-context ellipsis and ellipsis-variable matching
      using bounded dynamic programming.
  - Requires 15.2.4.
  - Success: list-context fixtures pass across supported languages, and runtime
    avoids exponential backtracking.
- [ ] 15.2.6. Implement deep-ellipsis matching with bounded traversal controls.
  - Requires 15.2.5.
  - Success: deep matching respects configured node limits and returns bounded,
    deterministic results.
- [ ] 15.2.7. Compile normalized formulas into plan nodes with explicit anchor
      and constraint separation.
  - Requires 15.1.2, 15.2.4, and completed archive work 4.1.5.
  - Success: conjunction plans enforce positive-term requirements, and
    compiled plan shapes remain snapshot-stable.
- [ ] 15.2.8. Implement conjunction, disjunction, and negative-constraint
      execution semantics.
  - Requires 15.2.7.
  - Success: `not`, `inside`, and `anywhere` semantics align with documented
    behaviour and pass regression fixtures.
- [ ] 15.2.9. Implement metavariable `where`-clause constraint evaluation with
      supported and unsupported outcomes.
  - Requires 15.2.8.
  - Success: supported constraints execute deterministically, and unsupported
    constraints return stable diagnostic codes.
- [ ] 15.2.10. Implement focus selection plus `as` and `fix` projection
      behaviour in emitted matches.
  - Requires 15.2.9.
  - Success: focus and capture projection follow documented precedence, and
    `fix` is surfaced as metadata without direct application.
- [ ] 15.2.11. Implement Tree-sitter query escape hatch with capture-name
      mapping into Semgrep-style capture keys.
  - Requires 15.2.10.
  - Success: raw Tree-sitter queries emit normalized captures and focus
    behaviour consistent with Sempai match output contracts.
- [ ] 15.2.12. Add execution safety controls for match caps, capture text caps,
      deep-search bounds, and bounded alternation.
  - Requires 15.2.11.
  - Success: safety limits are configurable, deterministic, and enforced
    across execution paths.

### 15.3. Prove Sempai Weaver integration and readiness

This step answers whether the backend can be exposed through Weaver's
resource-first command surface with stable schemas, cache behaviour,
diagnostics, quality gates, and default-enablement rules. It preserves
prototype archive work 4.3.1 through 4.3.9 under the new public command shape.
See `docs/weaver-design.md` §2.1.2.

- [ ] 15.3.1. Add Sempai execution routing in `weaverd` for selector-backed
      `symbols list`.
  - Requires 15.2.12.
  - Success: daemon execution paths compile and execute Sempai plans for
    supported languages and return structured match streams.
- [ ] 15.3.2. Add `weaver symbols list --query` with `--lang`, `--uri`, and
      `--rule-file|--rule|--query` inputs.
  - Requires 15.3.1 and 14.2.1.
  - Success: the CLI validates input combinations and supports YAML rule files,
    inline YAML rules, and one-liner query workflows with stable error
    messaging. `weaver observe query` remains archive provenance only.
- [ ] 15.3.3. Define stable JSONL request and response schemas for Sempai query
      operations, with snapshot coverage.
  - Requires 15.3.2.
  - Success: schema fixtures lock field names and payload shapes, and streaming
    output remains deterministic.
- [ ] 15.3.4. Integrate parse-cache adapter keyed by URI, language, and
      revision, aligned with daemon document lifecycle.
  - Requires 15.3.3.
  - Success: repeated queries against unchanged revisions hit cache in
    integration tests, revision changes invalidate cached parses
    deterministically, and cache misses or invalidations preserve semantic
    correctness.
- [ ] 15.3.5. Implement actuation handoff contract using focus-first selection
      with span fallback and optional capture targeting.
  - Requires 15.3.3.
  - Success: downstream mutation commands can consume Sempai output
    deterministically for target selection without hidden state.
- [ ] 15.3.6. Add diagnostics conformance suites for YAML, domain-specific
      language (DSL), semantic, compilation, and execution error categories.
  - Requires 15.3.3.
  - Success: each diagnostic category is covered by deterministic snapshots and
    stable `E_SEMPAI_*` error codes.
- [ ] 15.3.7. Add layered quality suites for parser and execution behaviour.
  - Requires 15.3.6.
  - Success: unit, snapshot, corpus, property, and fuzz suites run under
    repository gates and include representative language corpora plus
    malformed-input coverage.
- [ ] 15.3.8. Publish compatibility boundaries for supported operators, modes,
      constraints, and escape-hatch behaviour.
  - Requires 15.3.7.
  - Success: user-facing docs distinguish supported, unsupported, and
    parse-only behaviours with stable terminology.
- [ ] 15.3.9. Define release gates for enabling Sempai by default.
  - Requires 15.3.8.
  - Success: crash-free requirements, diagnostics parity, and documentation
    parity are codified in CI policy and block default enablement when
    thresholds are not met.

### 15.4. Prove query output composes with context commands

This step answers whether one-liner selectors can feed product commands rather
than remaining search output. It migrates prototype archive work 9.1.4 through
9.3.2 and 11.2.1. See `docs/weaver-design.md` §2.1.2.

- [ ] 15.4.1. Allow `cards get` and one-hop relation summaries to consume
      Sempai selectors.
  - Requires 15.3.5 and 14.2.3.
  - Success: one query-to-card-to-relation workflow works in one command and
    in a pipeline, with deterministic zero-, one-, and many-match behaviour.
- [ ] 15.4.2. Add Sempai selector conformance and pipeline E2E coverage.
  - Requires 15.4.1.
  - Success: suites cover YAML-backed and one-liner selectors, malformed query
    recovery, `jq` filtering, pager consumption, bounded output, and invalid
    selector-stream rejection.

## 16. Safe change loop slice

Idea: if the same selector records can drive safe patches, renames, and symbol
relocation through capability-routed actuators, Weaver proves that agent-native
composition does not weaken the Double-Lock safety model.

This phase validates the first end-to-end observe-to-act loop. It migrates
archive work from apply-patch, rename-symbol, `extricate-symbol`, selector
handoff, mutation metadata, and visible safety-harness reporting. In this
roadmap, `extricate-symbol` means moving a selected symbol to another module or
file while preserving meaning. It is distinct from `extract-method`, which
extracts a selected code region into a new callable and is not compressed into
the symbol-relocation tasks below.

### 16.1. Prove patch application under the new command contract

This step answers whether the completed patch and Double-Lock foundations fit
the resource-first grammar. It migrates prototype archive work 6.1.1 through
6.1.4 and 11.4.1 through 11.4.2. See `docs/weaver-design.md` §§4.2-4.3.

- [ ] 16.1.1. Implement `weaver patches apply`.
  - Requires phase 13 and reuses prototype archive work 6.1.1 through 6.1.4.
  - Success: `patches apply` preserves Double-Lock verification, atomic
    transactions, `--dry-run`, structured safety results, universal `--json`,
    and human-readable safety summaries.
- [ ] 16.1.2. Add idempotency keys, transaction IDs, and retry matching for
      patch application.
  - Requires 16.1.1 and depends on OrthoConfig 7.2.1.
  - Success: repeated equivalent patch submissions return the existing
    transaction or refusal instead of duplicating work.
- [ ] 16.1.3. Standardize mutation `--dry-run`, `--force`, and structured
      safety metadata.
  - Requires 16.1.2 and depends on OrthoConfig 7.2.1.
  - Success: mutating commands declare preview, destructive-operation, and
    safety-harness policy in command-surface metadata.

### 16.2. Prove capability-routed rename from positions and selectors

This step answers whether provider-hidden actuators can mutate safely from both
direct references and Sempai streams. It migrates prototype archive work 5.2.1
through 5.2.6, 10.5.1 through 10.5.2, and 4.3.5. See ADR 001 and ADR 004.

- [ ] 16.2.1. Implement `weaver symbols rename` for position references.
  - Requires 16.1.3 and reuses prototype archive work 5.2.1 through 5.2.5.
  - Success: provider routing remains capability-first, mutation results
    include transaction ID, affected paths, provider provenance, and safety
    outcome.
- [ ] 16.2.2. Add direct Sempai selector support to `symbols rename`.
  - Requires 15.3.5 and 16.2.1.
  - Success: `symbols rename --query ...` handles zero, one, and many matches
    deterministically and requires explicit policy for ambiguous mutation.
- [ ] 16.2.3. Add `--from-stdin` selector stream consumption to mutation
      commands.
  - Requires 15.3.3, 15.3.5, and 16.2.2.
  - Success: `weaver symbols list --query … --json | jq … | weaver symbols
    rename --from-stdin …` works without hidden state.
- [ ] 16.2.4. Add rename and selector-mutation combinatorial E2E coverage.
  - Requires 16.2.3.
  - Success: coverage combines selector forms, `--json`, `--dry-run`,
    idempotency, provider failures, syntactic failures, semantic failures, and
    rollback assertions.

### 16.3. Prove the `extricate-symbol` contract with Python first

This step answers whether symbol relocation can be expressed as one stable
capability and public command before Weaver invests in Rust-specific
orchestration. It migrates prototype archive work 5.3.1 through 5.3.6 under the
canonical public command `weaver symbols move`. See ADR 001 and ADR 004.
`extract-method`, `replace-body`, and `extract-predicate` may be declared as
distinct capability IDs, but this step does not implement those operations.

- [ ] 16.3.1. Add capability ID scaffolding and resolver policy for actuator
      capabilities.
  - Requires 16.2.1.
  - Success: `rename-symbol`, `extricate-symbol`, `extract-method`,
    `replace-body`, and `extract-predicate` are distinct typed capability IDs;
    resolver output includes language, selected provider, and policy rationale;
    no task treats `extract-method` as an alias for `extricate-symbol`.
- [ ] 16.3.2. Extend plugin manifests and broker loading for capability-aware
      selection.
  - Requires 16.3.1.
  - Success: manifest validation enforces capability fields, and provider
    selection respects language plus capability compatibility without exposing
    provider-specific commands.
- [ ] 16.3.3. Add the `weaver symbols move --uri --position --to` command
      contract and discovery output for `extricate-symbol`.
  - Requires 13.3.2, 16.3.2, and depends on OrthoConfig 7.2.7.
  - Success: CLI request shape is stable across providers, `context --json`
    and `capabilities list --json` report support by language, and the public
    verb remains `move` while the internal capability remains
    `extricate-symbol`.
- [ ] 16.3.4. Extend the Rope plugin with narrow Python `extricate-symbol`
      support.
  - Requires 16.3.3.
  - Success: Rope returns unified diffs through the existing patch application
    flow for supported Python symbol moves and emits structured refusals for
    unsupported symbol shapes.
- [ ] 16.3.5. Extend plugin and daemon failure schemas with deterministic
      refusal diagnostics and rollback guarantees for extrication.
  - Requires 16.1.3 and 16.3.4.
  - Success: refusal paths emit structured diagnostics, include stable reason
    codes, enumerate valid alternatives where possible, and leave the
    filesystem unchanged.
- [ ] 16.3.6. Add unit, behavioural, and end-to-end coverage for capability
      resolution and Python extrication baseline paths.
  - Requires 16.3.5.
  - Success: tests assert capability negotiation, refusal behaviour,
    incomplete payload failures, deterministic patch output, rollback, and
    `--json` plus human-renderer behaviour.

### 16.4. Prove Rust `extricate-symbol` with explicit orchestration gates

This step answers whether Rust symbol relocation is realistic without reducing
the safety bar or hiding complexity inside a single "move" task. It migrates
prototype archive work 5.4.1 through 5.4.7. See
`docs/rust-extricate-actuator-plugin-technical-design.md`.

- [ ] 16.4.1. Define Rust extrication orchestration contracts and transaction
      boundaries in `weaverd`.
  - Requires 16.3.3.
  - Success: stage boundaries, capability ownership, rollback semantics, and
    daemon/plugin responsibilities are explicit and covered by unit tests.
- [ ] 16.4.2. Implement the Rust symbol planning pipeline using
      rust-analyzer definition, references, and call-site discovery.
  - Requires 16.4.1.
  - Success: the planner identifies relocation scope deterministically, records
    required file payloads, and returns structured diagnostics for unsupported
    symbol shapes.
- [ ] 16.4.3. Implement staged Rust transformation execution through the
      rust-analyzer actuator path.
  - Requires 16.4.2.
  - Success: staged execution emits unified diffs, preserves deterministic
    operation order, reports stage-level failures, and makes the prototype
    viability of Rust extrication visible before repair hardening begins.
- [ ] 16.4.4. Implement import and module-graph repair loops.
  - Requires 16.4.3.
  - Success: common import breakages are auto-repaired, ambiguous repairs
    return deterministic refusal diagnostics, and no partial writes are
    committed.
- [ ] 16.4.5. Integrate semantic verification and rollback enforcement for
      Rust extrication transactions.
  - Requires 16.3.5 and 16.4.4.
  - Success: semantic lock failures abort the transaction, rollback is complete
    across all touched files, and diagnostics identify the failed verification
    stage.
- [ ] 16.4.6. Add Rust-specific unit, behavioural, and end-to-end coverage for
      extrication scenarios.
  - Requires 16.4.5.
  - Success: coverage includes nested module moves, trait implementation
    updates, macro-adjacent boundaries, module graph updates, rollback
    guarantees, and deterministic failure semantics.
- [ ] 16.4.7. Publish Rust `extricate-symbol` compatibility boundaries and
      operator guidance.
  - Requires 16.4.6.
  - Success: docs, human output, and capability surfaces use stable terminology
    for supported, partial, and unsupported Rust shapes.
- [ ] 16.4.8. Decide whether `extricate-symbol` graduates from the core
      roadmap.
  - Requires 16.3.6 and 16.4.7.
  - Success: the roadmap records one of three outcomes: graduate Python and
    Rust, graduate one language with clear limits, or defer the capability out
    of the 0.1.0 core. `extract-method` is assessed separately and cannot be
    marked complete by this decision.

## 17. Impact and history slice

Idea: if graph slices, history reconstruction, and probabilistic matching can
explain the blast radius of a proposed change with bounded output, Weaver moves
beyond point queries into decision support without overwhelming agent context.

This phase validates richer graph work only after the read and mutation loops
exist. It migrates archive work from graph-slice traversal, graph-history,
probabilistic matching, static-analysis provider integration, conditional
ledger caching, and card-driven traversal.

### 17.1. Prove graph slices add useful context beyond cards

This step answers whether graph traversal gives enough extra value to justify
its complexity. It preserves the completed prototype archive schema work 7.2.1
and migrates prototype archive work 7.2.2 through 7.2.5, 11.1.1, and 11.2.2.
See `docs/jacquard-card-first-symbol-graph-design.md` §12.1 through §12.3.

- [ ] 17.1.1. Implement a two-pass Tree-sitter extraction pipeline for graph
      slices.
  - Requires 14.2.3 and uses completed prototype archive work 7.2.1.
  - Success: edge extraction builds a full or partial symbol table before
    resolving edges, every edge carries resolution scope
    (`full_symbol_table`, `partial_symbol_table`, or `lsp`), and unresolved
    references are preserved as external nodes with explicit confidence.
- [ ] 17.1.2. Implement call-edge slice expansion through `weaver-graph`.
  - Requires 17.1.1.
  - Success: `call` edges use the existing LSP call hierarchy provider,
    include explicit provenance, enforce depth limits, and have end-to-end
    coverage for a depth-2 traversal on a fixture repository.
- [ ] 17.1.3. Implement baseline `import` and `config` edge extraction.
  - Requires 17.1.1.
  - Success: Tree-sitter interstitial passes and per-language queries emit
    bounded `import` and `config` edges with confidence values, provenance,
    and at least one test per supported language.
- [ ] 17.1.4. Add dependency and dependent edge enrichment without leaking
      provider-specific commands.
  - Requires 17.1.3 and reuses prototype archive work 5.9.1.
  - Success: static-analysis and graph providers enrich slices with
    dependency and dependent edges through capability routing, preserve
    provenance, and degrade visibly when a provider is unavailable.
- [ ] 17.1.5. Implement budgeted priority traversal and graph-slice command
      integration.
  - Requires 17.1.2, 17.1.3, and 17.1.4.
  - Success: `weaver graph-slices get` consumes the completed schema from
    prototype archive work 7.2.1, never exceeds explicit `max_cards`,
    `max_edges`, or `max_estimated_tokens` caps, emits debug rejection reasons
    when requested, and includes behaviour-driven tests for fan-out explosions
    and budget truncation.

### 17.2. Prove history mode before investing in probabilistic matching

This step answers whether historical graph reconstruction is stable enough to
support change-risk narratives. It migrates prototype archive work 7.3.1
through 7.3.5. See `docs/jacquard-card-first-symbol-graph-design.md` §13.1,
§13.2, and §22.

- [ ] 17.2.1. Implement git-backed blob loading for historical revisions
      without checkout.
  - Requires 17.1.5.
  - Success: history queries never invoke `git checkout`, load only files
    required by the slice budget, enforce blob size, parse-time, file-count,
    and partial-parse limits, and record fallback reasons such as `timeout`,
    `blob_too_large`, `partial_parse`, and `unsupported_grammar`.
- [ ] 17.2.2. Implement slice reconstruction per commit with data-quality
      metadata.
  - Requires 17.2.1.
  - Success: `--commits 5` returns a stable set of commits and per-commit
    slice payloads with `quality.resolution_scope` and `quality.fallbacks`;
    delta payloads include added, removed, and changed nodes and edges; and
    curated git fixtures cover deterministic output.
- [ ] 17.2.3. Implement delta normalization and change taxonomy
      classification.
  - Requires 17.2.2.
  - Success: import blocks and decorators are treated as commutative sets for
    deltas, normalized representations are persisted alongside raw text,
    import or decorator reordering is classified as `text` change, taxonomy
    output includes confidence, and fixtures cover comment-only and
    signature-only edits.
- [ ] 17.2.4. Implement semantic risk warnings on history deltas.
  - Requires 17.2.3.
  - Success: `--warning-depth` widens the dependency neighbourhood scanned for
    warnings, warnings include edge paths and confidence, `text`-only deltas
    emit lower-risk warnings, and curated fixtures validate dependency and
    dependent warning types.
- [ ] 17.2.5. Implement history-mode gating and safe defaults.
  - Requires 17.2.4.
  - Success: default history mode uses Tree-sitter-only extraction for
    historical commits, LSP enrichment is disabled unless explicitly enabled
    and documented, and degraded behaviour is visible through provenance
    fields.

### 17.3. Prove probabilistic matching only where history needs it

This step answers whether fuzzy matching improves history explanations enough
to carry its complexity. It migrates prototype archive work 7.4.1 through 7.4.9
and 8.6.2. See `docs/jacquard-card-first-symbol-graph-design.md` §14.1 through
§14.8.

- [ ] 17.3.1. Implement phase 1 stable-identity matching.
  - Requires 17.2.5.
  - Success: matching uses type, name, container, and file hints; outputs
    include winning phase and confidence; non-matching candidates are rejected;
    and fixtures include rename and move cases that must not match in phase 1.
- [ ] 17.3.2. Implement phase 2 body-hash matching for rename detection.
  - Requires 17.3.1.
  - Success: unchanged-body renames match with explicit phase and confidence,
    low-confidence matches are rejected rather than forced, and fixtures cover
    rename scenarios with unchanged bodies.
- [ ] 17.3.3. Implement phase 3 structural-hash matching on normalized AST
      shapes.
  - Requires 17.3.2.
  - Success: formatting-only moves match through structural evidence, outputs
    include phase and confidence, and low-confidence matches remain rejected.
- [ ] 17.3.4. Implement phase 4 fuzzy similarity matching.
  - Requires 17.3.3.
  - Success: token-overlap and shingle matching improves rename and move
    fixtures with minor body edits without exceeding bounded candidate or token
    budgets, and low-confidence matches remain rejected.
- [ ] 17.3.5. Implement phase 5 graph refinement and global assignment
      refinement.
  - Requires 17.3.4.
  - Success: neighbourhood evidence resolves supported rename and move
    scenarios, outputs include phase and confidence, and low-confidence matches
    remain rejected.
- [ ] 17.3.6. Implement deterministic feature extraction for cross-commit
      matching.
  - Requires 17.3.5.
  - Success: feature extraction covers signature, AST-shape, docstring
    fingerprints, attachments, and neighbourhood sketches; identical inputs
    produce identical features; unit tests cover whitespace-only edits and
    alpha-renaming of locals; and failures emit structured diagnostics.
- [ ] 17.3.7. Implement candidate generation and calibrated scoring with reason
      codes.
  - Requires 17.3.6.
  - Success: response payloads always include `best_match` plus top-K
    alternates up to the requested cap, reason codes are stable enumerations,
    and debug output surfaces the top contributing features.
- [ ] 17.3.8. Implement duplicate-name guardrails.
  - Requires 17.3.7.
  - Success: `--max-duplicates` forces explicit ambiguous-mapping responses or
    fallback matching when homonyms explode, observability counters capture
    guardrail triggers, and same-name fixtures avoid false renames.
- [ ] 17.3.9. Implement injective assignment across the slice.
  - Requires 17.3.8.
  - Success: the solver avoids mapping multiple sources to one target unless
    explicitly enabled, property tests prevent illegal many-to-one mappings by
    default, a feature flag gates split or merge experimentation, and
    deterministic fixtures cover rename and move scenarios.

### 17.4. Prove the optional graph ledger after on-demand history

This step answers whether persisted graph history caching is necessary after
`snapshots_on_demand` history is measured. It migrates prototype archive work
7.5.1 through 7.5.2. See `docs/jacquard-card-first-symbol-graph-design.md`
§13.2, §18.1, and §18.2.

- [ ] 17.4.1. Validate whether a persisted graph ledger belongs in the core
      product.
  - Requires 17.2.5.
  - Success: the decision compares repeated history-query performance against
    the on-demand baseline and either accepts a core ledger with measured
    evidence or defers it without weakening history-mode correctness.
- [ ] 17.4.2. Define a versioned on-disk ledger format for cards, edges, and
      deltas.
  - Requires 17.4.1 accepting a core ledger.
  - Success: the ledger is keyed by commit hash, includes explicit version
    fields, detects corruption with checksums, and gates schema changes behind
    migrations.
- [ ] 17.4.3. Implement incremental ledger population and invalidation rules.
  - Requires 17.4.2.
  - Success: ledger writes are atomic, invalidation occurs when inputs change,
    and performance benchmarks show a measurable improvement for repeated
    history queries.

## 18. Provider ecosystem slice

Idea: if specialist perceptors and actuators can improve concrete workflows
while remaining hidden behind capability contracts, Weaver can grow an
ecosystem without forcing users or agents to learn provider-specific commands.

This phase validates additional plugins only after the core command, selector,
mutation, and graph loops exist. It migrates archive work from additional
actuators, specialist sensors, plugin introspection, graceful degradation, and
capability discoverability.

### 18.1. Prove providers remain implementation details during success

This step answers whether Rope, rust-analyzer, `srgn`, `jedi`, static analysis,
and future providers can share one public capability contract. It migrates
prototype archive work 5.5.1, 5.6.1, and 5.8.1. See ADR 001, ADR 004, and ADR
006.

- [ ] 18.1.1. Add `srgn` or equivalent as an actuator or selector provider
      behind manifests.
  - Requires 16.2.1.
  - Success: the provider declares capability support without adding a
    provider-specific public command.
- [ ] 18.1.2. Add `jedi` or equivalent as the first specialist perceptor
      provider.
  - Requires 14.1.3.
  - Success: the provider enriches definitions, references, diagnostics,
    cards, or symbol matches through capability routing.
- [ ] 18.1.3. Refine graceful degradation guidance for provider failures.
  - Requires 18.1.1 and 18.1.2.
  - Success: unsupported capability errors suggest resource-first fallback
    workflows and enumerate valid alternatives.

### 18.2. Prove provider state is discoverable without leaking into workflow

This step answers whether humans and agents can debug provider selection when
needed without making provider names normal command syntax. It migrates
prototype archive work 5.7.1 through 5.7.5 and 5.2.6. See
`docs/weaver-design.md` §6.1.

- [ ] 18.2.1. Wire provider summaries into `capabilities list` and
      `context --json`.
  - Requires 13.3.2 and depends on OrthoConfig 7.2.7.
  - Success: agents can discover availability, selected provider, refusal
    reasons, and profile-driven policy without parsing backend-specific help.
- [ ] 18.2.2. Publish capability migration notes for `symbol.rename` and
      `extricate-symbol` decisions.
  - Requires 16.4.8.
  - Success: docs explain provider provenance, refusal semantics, resource
    command replacements, and any deferred provider-specific limits.
- [ ] 18.2.3. Add provider-matrix E2E coverage.
  - Requires 18.2.1 and 18.2.2.
  - Success: one suite proves selected-provider reporting, fallback refusal,
    missing-provider guidance, profile overrides, and JSON provenance across
    read and write capabilities.

## 19. Agent workflow and assurance slice

Idea: if durable execution state, profiles, delivery, feedback, onboarding, and
formal checks can wrap the established read/write loops, Weaver becomes a
dependable agent tool rather than a set of clever commands.

This phase validates compounding agent-native primitives against real Weaver
workflows instead of shipping them as abstract infrastructure. It migrates
archive work from onboarding, interactive review, dynamic analysis ingestion,
formal verification, output delivery, feedback, profiles, and jobs.

### 19.1. Prove persistent workflow state on real commands

This step answers whether profiles, jobs, and delivery reduce repeated agent
turns without making human usage worse. It migrates the product intent of ADR
007 compounding primitives and prototype archive work 6.2.1. See
`docs/weaver-design.md` §§2.1.5-2.1.6.

- [ ] 19.1.1. Implement profile storage, redaction, and root `--profile`.
  - Requires 13.3.1 and depends on OrthoConfig 9.1.
  - Success: profile names and metadata appear in `context --json`, secret
    values stay redacted, and precedence is
    `built-in defaults < config files < selected profile < environment < flags`.
- [ ] 19.1.2. Implement durable job ledger support for long-running graph,
      history, and mutation commands.
  - Requires 16.1.2 and 17.2.5 and depends on OrthoConfig 9.3.
  - Success: job records store command path, idempotency key, workspace,
    request hash, status, progress, timestamps, result pointer, and exit class.
- [ ] 19.1.3. Implement `--wait`, `jobs list|get|prune`, and delivery sinks on
      at least one read workflow and one write workflow.
  - Requires 19.1.2 and depends on OrthoConfig 9.2.1.
  - Success: submit-poll-collect workflows support recovery, atomic file
    delivery, webhook delivery, and structured refusal for unknown schemes.
- [ ] 19.1.4. Implement `feedback create|list|send`.
  - Requires 19.1.3 and depends on OrthoConfig 9.2.2.
  - Success: local feedback writes JSONL by default, upstream send is optional
    and configured, and feedback availability appears in `context --json`.

### 19.2. Prove onboarding and optional interaction do not break automation

This step answers whether richer human workflows can exist without weakening
the non-interactive agent contract. It migrates prototype archive work 6.2.1
through 6.2.3. See `docs/weaver-design.md` §§5-6.

- [ ] 19.2.1. Recast project onboarding as a composition of resource-first
      commands.
  - Requires phases 14, 15, and 17.
  - Success: onboarding consumes cards, graph slices, diagnostics, dependency
    data, and optional dynamic-analysis inputs without adding a parallel
    public command grammar.
- [ ] 19.2.2. Implement explicit interactive review as an opt-in workflow.
  - Requires 16.1.3.
  - Success: interaction is available through `--interactive` or a dedicated
    review command and fails fast when stdin is not a terminal.
- [ ] 19.2.3. Decide whether dynamic-analysis ingestion belongs in core.
  - Requires 17.1.4 and 19.2.1.
  - Success: runtime traces either enrich graph slices with provenance and
    bounded trust semantics or move to deferred extensions with rationale.

### 19.3. Prove the owned safety kernels, not the external tools

This step answers whether formal and property-based checks catch regressions in
Weaver-owned invariants. It migrates prototype archive work 8.1.1 through
8.6.3. See `docs/formal-verification-methods-in-weaver.md` and ADR 005.

- [ ] 19.3.1. Add pinned Kani and Verus tooling plus explicit formal targets.
  - Requires 16.1.1.
  - Success: verifier installs are reproducible, normal Rust workflows are
    unaffected, and slow proof suites stay out of default pull-request gates
    until stable.
- [ ] 19.3.2. Publish transaction, semantic-lock, and verification
      trust-boundary contracts.
  - Requires 19.3.1.
  - Success: docs distinguish verified orchestration from trusted providers,
    language servers, parsers, operating systems, and filesystems.
- [ ] 19.3.3. Add Kani checks for transaction, patch, routing, and bounded
      graph kernels.
  - Requires 19.3.2 and 17.1.5.
  - Success: smoke harnesses cover commit gating, rollback bookkeeping,
    bounded path guardrails, whole-command abort on unmatched patch blocks,
    selected-provider predicates, refusal determinism, and graph budgets.
- [ ] 19.3.4. Add proof-only Verus kernels where they reduce long-term risk.
  - Requires 19.3.3.
  - Success: proof modules remain outside the production API and prove only
    stable abstractions that survived the read, mutation, and graph slices.

## 20. Deferred extensions after the core product promise

Idea: if the core CLI contract and semantic workflows are already trustworthy
and boring to operate, the project can evaluate broader integrations on product
value instead of letting them destabilize the main release.

This phase is not a hiding place for relevant product work. Items land here
only when the preceding slices can invalidate or defer them without weakening
the core Weaver promise.

### 20.1. Evaluate generated integrations

This step answers whether external integration surfaces are wrappers over the
same command metadata or a distracting second product. See ADR 007 and
OrthoConfig 10.1.

- [ ] 20.1.1. Decide whether to generate MCP descriptions from
      `context --json`.
  - Requires phase 13 and depends on OrthoConfig 10.1.1.
  - Success: the decision compares generated MCP descriptions against the
    command-surface token budget and records whether the feature belongs in
    Weaver 0.1.x.
- [ ] 20.1.2. Decide whether SDK or OpenAPI-shaped runtime explorers are in
      scope.
  - Requires 20.1.1 and depends on OrthoConfig 10.1.2.
  - Success: any accepted explorer uses the same command metadata and does not
    become a second source of truth.

### 20.2. Evaluate deferred provider and analysis experiments

This step answers whether experiments invalidated by earlier slices deserve a
new design rather than quiet re-entry into the core roadmap.

- [ ] 20.2.1. Reassess deferred `extricate-symbol` language support and
      `extract-method` experiments.
  - Requires 16.4.8.
  - Success: unsupported languages or symbol kinds either gain new provider
    evidence, `extract-method` gains its own evidence-backed slice, or both
    remain explicitly out of scope.
- [ ] 20.2.2. Reassess deferred dynamic-analysis ingestion.
  - Requires 19.2.3.
  - Success: runtime trace ingestion either has bounded trust semantics and a
    graph integration point or remains out of the core product.
- [ ] 20.2.3. Reassess graph ledger extensions beyond the core cache.
  - Requires 17.4.3.
  - Success: further cache work resumes only if measured repository-scale
    performance shows the core ledger is insufficient for the supported
    history workflows.

### 20.3. Evaluate deferred observability expansions

This step answers whether optional observability surfaces have earned a new
design after the local-first RFC 0001 contract exists. It keeps metrics,
distributed tracing, retained diagnostics, and status expansion out of the core
promise until their privacy, retention, endpoint, and command-latency costs are
explicit. See `docs/rfcs/0001-o11y.md` §§"Open questions", "Local request
correlation", "Deferred path: status subcommand expansion", "Option D:
Dedicated diagnostics artefact", "Deferred path: optional metrics endpoint",
and "Options considered".

- [ ] 20.3.1. Decide whether CLI pre-daemon diagnostics need a minimal
      `tracing` subscriber.
  - Requires 13.4.6.
  - Success: the decision either keeps pre-daemon diagnostics as explicit
    stderr guidance or defines a local subscriber contract without adding a
    remote telemetry surface.
- [ ] 20.3.2. Decide whether to add a local `request_id` to the CLI-to-daemon
      JSONL schema.
  - Requires 13.4.6.
  - Success: the decision either accepts a bounded local correlation field with
    schema and privacy rules or records why structured event fields are enough.
- [ ] 20.3.3. Decide whether `weaver daemon status --json` belongs in the
      command contract.
  - Requires 13.4.6 and 13.3.1.
  - Success: the outcome is recorded as a bounded status-schema extension, a
    deliberate reliance on `weaverd.health` for machine-readable state, or a
    deferral with rationale.
- [ ] 20.3.4. Decide whether abnormal exits need a dedicated bounded
      diagnostics artefact.
  - Requires 13.4.6.
  - Success: the decision either specifies a separate retained diagnostics
    artefact with privacy, retention, cleanup, and rotation rules or keeps
    foreground logs as the supported debug path.
- [ ] 20.3.5. Reconfirm the metrics endpoint and distributed tracing boundary.
  - Requires 20.3.2 and 20.3.4.
  - Success: any metrics, tracing, dashboard, or aggregation surface requires a
    follow-up RFC with local-binding, feature-flag, privacy, and latency rules.

## Archive

Historical prototype roadmap entries live in
[`docs/archive/prototype-roadmap.md`](archive/prototype-roadmap.md). Those
entries keep numbers `1` through `11`; this live roadmap reserves numbers `12`
through `20` for the forward ADR 007 build sequence. The archive is not an
active implementation backlog.
