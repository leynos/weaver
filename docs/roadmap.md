# Weaver roadmap

This roadmap translates `docs/weaver-design.md`,
`docs/adr-007-agent-native-command-surface.md`,
`docs/sempai-query-language-design.md`,
`docs/jacquard-card-first-symbol-graph-design.md`, and the existing ADR set
into an outcome-oriented delivery sequence. It does not promise dates. Phases
carry testable product ideas, steps validate or falsify those ideas, and tasks
are review-sized execution units with explicit dependencies and observable
success criteria.

The current forward plan is the source of truth for future work. The historical
ledger in [`docs/archive/prototype-roadmap.md`](archive/prototype-roadmap.md)
preserves task numbers `1` through `11` as provenance. Relevant unfinished
archive work has been moved into this live roadmap under resource-first command
names; prototype `observe`, `act`, and `verify` spellings are not future public
grammar unless a live task explicitly reintroduces them.

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
- [ ] 13.1.2. Implement the Weaver command-surface adapter for one read-only
      command family.
  - Requires 13.1.1 and depends on OrthoConfig 5.2.3, 6.1, and 7.2.7.
  - Start with `definitions get` plus the metadata needed to add
    `references list` without a second path.
  - Success: resource path, verb, capability ID, selector forms, output
    schemas, error schemas, mutability, provider policy, examples, and skill
    references all flow from one adapter record.
- [ ] 13.1.3. Define the temporary-adapter removal policy.
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

This phase deliberately validates the narrowest useful Sempai workflow before
building the full Semgrep-compatible backend. It migrates archive work from
Sempai DSL parsing, Tree-sitter execution, query routing, symbol-first cards,
and the Sempai-to-Jacquard vertical slice.

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

### 15.2. Prove Tree-sitter execution on the smallest useful language set

This step answers whether Semgrep-style matching can run accurately enough on
real syntax before the full operator matrix lands. It migrates prototype
archive work 4.2.1 through 4.2.12 and 9.1.2 through 9.1.3. See
`docs/sempai-query-language-design.md` §§7-12.

- [ ] 15.2.1. Implement language profiles and snippet wrapping for Rust and
      Python.
  - Requires 15.1.2.
  - Success: the profiles support safe snippet parsing, wrapper provenance,
    and clear refusal for unsupported languages.
- [ ] 15.2.2. Implement node-kind matching, metavariable unification, and
      bounded ellipsis for the pilot profiles.
  - Requires 15.2.1.
  - Success: positive-pattern fixtures match expected symbols, execution caps
    prevent runaway matches, and unsupported operators enumerate alternatives.
- [ ] 15.2.3. Decide whether TypeScript joins the first Sempai slice.
  - Requires 15.2.2.
  - Success: TypeScript either passes the same selector fixtures or is deferred
    with a documented blocker and fallback behaviour.

### 15.3. Prove query output composes with context commands

This step answers whether one-liner selectors can feed product commands rather
than remaining search output. It migrates prototype archive work 4.3.1 through
4.3.9, 9.1.4 through 9.3.2, and 11.2.1. See `docs/weaver-design.md` §2.1.2.

- [ ] 15.3.1. Implement `weaver symbols list --query`.
  - Requires 15.2.2 and 14.2.1.
  - Success: `symbols list --query 'fn $name(...)' --json` emits bounded
    selector records that ordinary UNIX filters can preserve.
- [ ] 15.3.2. Allow `cards get` and one-hop relation summaries to consume
      Sempai selectors.
  - Requires 15.3.1 and 14.2.3.
  - Success: one query-to-card-to-relation workflow works in one command and
    in a pipeline, with deterministic zero-, one-, and many-match behaviour.
- [ ] 15.3.3. Add Sempai selector conformance and pipeline E2E coverage.
  - Requires 15.3.2.
  - Success: suites cover YAML-backed and one-liner selectors, malformed query
    recovery, `jq` filtering, pager consumption, bounded output, and invalid
    selector-stream rejection.

## 16. Safe change loop slice

Idea: if the same selector records can drive safe patches, renames, and
extract-or-move refactors through capability-routed actuators, Weaver proves
that agent-native composition does not weaken the Double-Lock safety model.

This phase validates the first end-to-end observe-to-act loop. It migrates
archive work from apply-patch, rename-symbol, extrication, selector handoff,
mutation metadata, and visible safety-harness reporting.

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
  - Requires 15.3.1 and 16.2.1.
  - Success: `symbols rename --query ...` handles zero, one, and many matches
    deterministically and requires explicit policy for ambiguous mutation.
- [ ] 16.2.3. Add `--from-stdin` selector stream consumption to mutation
      commands.
  - Requires 15.3.3 and 16.2.2.
  - Success: `weaver symbols list --query … --json | jq … | weaver symbols
    rename --from-stdin …` works without hidden state.
- [ ] 16.2.4. Add rename and selector-mutation combinatorial E2E coverage.
  - Requires 16.2.3.
  - Success: coverage combines selector forms, `--json`, `--dry-run`,
    idempotency, provider failures, syntactic failures, semantic failures, and
    rollback assertions.

### 16.3. Decide whether move/extract is viable for the core product

This step answers whether extrication is a dependable product differentiator or
an attractive but unstable refactor. It migrates prototype archive work 5.3.1
through 5.4.7 under canonical public verbs. See
`docs/rust-extricate-actuator-plugin-technical-design.md`.

- [ ] 16.3.1. Implement the `symbol.move` or `symbol.extract` capability
      contract and refusal vocabulary.
  - Requires 16.2.1.
  - Success: the public command uses canonical verbs while the internal
    capability captures transaction, import-repair, and provider constraints.
- [ ] 16.3.2. Prototype Python symbol extraction through Rope.
  - Requires 16.3.1.
  - Success: a narrow class of Python moves either commits safely through the
    Double-Lock path or records refusal reasons that invalidate the approach.
- [ ] 16.3.3. Prototype Rust symbol movement through rust-analyzer.
  - Requires 16.3.1.
  - Success: a narrow class of Rust moves either commits safely with
    import/module repair or records compatibility limits that defer the
    feature.
- [ ] 16.3.4. Decide whether move/extract graduates from the core roadmap.
  - Requires 16.3.2 and 16.3.3.
  - Success: the roadmap records one of three outcomes: graduate both
    languages, graduate one language with clear limits, or defer the capability
    out of the 0.1.0 core.

## 17. Impact and history slice

Idea: if graph slices, history reconstruction, and probabilistic matching can
explain the blast radius of a proposed change with bounded output, Weaver moves
beyond point queries into decision support without overwhelming agent context.

This phase validates richer graph work only after the read and mutation loops
exist. It migrates archive work from graph-slice traversal, graph-history,
probabilistic matching, static-analysis provider integration, optional ledger
cache, and card-driven traversal.

### 17.1. Prove graph slices add useful context beyond cards

This step answers whether graph traversal gives enough extra value to justify
its complexity. It migrates prototype archive work 7.2.2 through 7.2.5, 11.1.1,
and 11.2.2. See `docs/jacquard-card-first-symbol-graph-design.md` §§12-13.

- [ ] 17.1.1. Implement `weaver graph-slices get` with Tree-sitter inventory
      and LSP call edges.
  - Requires 14.2.3.
  - Success: graph slices include bounded cards, typed edges, provenance,
    truncation markers, and useful human summaries.
- [ ] 17.1.2. Add import, config, dependency, and dependent edge extraction.
  - Requires 17.1.1 and reuses prototype archive work 5.9.1.
  - Success: static-analysis and graph providers enrich slices without adding
    provider-specific public commands.
- [ ] 17.1.3. Implement budgeted priority traversal and graph-slice E2E
      coverage.
  - Requires 17.1.2.
  - Success: tests prove edge, card, depth, and token budgets across mixed
    edge types; truncated responses include narrowing hints.

### 17.2. Prove history mode before investing in probabilistic matching

This step answers whether historical graph reconstruction is stable enough to
support change-risk narratives. It migrates prototype archive work 7.3.1
through 7.3.5. See `docs/jacquard-card-first-symbol-graph-design.md` §§16-17.

- [ ] 17.2.1. Implement git-backed graph-slice reconstruction per commit.
  - Requires 17.1.3.
  - Success: history mode loads blobs without mutating the worktree, reports
    explicit data quality, and keeps defaults safe for large repositories.
- [ ] 17.2.2. Implement normalized graph delta computation and risk warnings.
  - Requires 17.2.1.
  - Success: added, removed, changed, moved, and uncertain nodes or edges have
    stable reason codes and useful human summaries.
- [ ] 17.2.3. Decide whether a versioned graph ledger belongs in core.
  - Requires 17.2.2 and migrates prototype archive work 7.5.1 through 7.5.2.
  - Success: the decision either adds cache population and invalidation to the
    core plan or defers it with measured performance evidence.

### 17.3. Prove probabilistic matching only where history needs it

This step answers whether fuzzy matching improves history explanations enough
to carry its complexity. It migrates prototype archive work 7.4.1 through 7.4.9
and 8.6.2. See `docs/jacquard-card-first-symbol-graph-design.md` §§14-15.

- [ ] 17.3.1. Implement stable-identity, body-hash, and structural-hash
      matching phases.
  - Requires 17.2.2.
  - Success: exact and structural matches produce reason codes and reject
    duplicate-name ambiguity by default.
- [ ] 17.3.2. Prototype fuzzy similarity and graph-refinement matching.
  - Requires 17.3.1.
  - Success: fuzzy matches improve fixture accuracy without exceeding bounded
    candidate or token budgets; otherwise the fuzzy phase is disabled by
    default.
- [ ] 17.3.3. Add assignment guardrails and matching E2E coverage.
  - Requires 17.3.2.
  - Success: assignments are injective by default, split/merge modes are
    explicit, and confidence scores are calibrated against fixture evidence.

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
      move/extract decisions.
  - Requires 16.3.4.
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
  - Requires 16.1.2 and 17.2.1 and depends on OrthoConfig 9.3.
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
  - Requires 17.1.2 and 19.2.1.
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
  - Requires 19.3.2 and 17.1.3.
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

- [ ] 20.2.1. Reassess deferred move/extract language support.
  - Requires 16.3.4.
  - Success: unsupported languages or symbol kinds either gain new provider
    evidence or remain explicitly out of scope.
- [ ] 20.2.2. Reassess deferred dynamic-analysis ingestion.
  - Requires 19.2.3.
  - Success: runtime trace ingestion either has bounded trust semantics and a
    graph integration point or remains out of the core product.
- [ ] 20.2.3. Reassess optional graph ledger caching.
  - Requires 17.2.3.
  - Success: cache work resumes only if measured repository-scale performance
    makes it necessary for the core workflows.

## Archive

Historical prototype roadmap entries live in
[`docs/archive/prototype-roadmap.md`](archive/prototype-roadmap.md). Those
entries keep numbers `1` through `11`; this live roadmap reserves numbers `12`
through `20` for the forward ADR 007 build sequence. The archive is not an
active implementation backlog.
