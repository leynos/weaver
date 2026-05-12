# Weaver roadmap

This roadmap translates `docs/weaver-design.md`,
`docs/adr-007-agent-native-command-surface.md`, and the existing ADR set into
an outcome-oriented delivery sequence. It does not promise dates. Phases carry
testable product ideas, steps answer sequencing questions, and tasks are
review-sized execution units with explicit dependencies and observable success
criteria.

The current forward plan is the source of truth for future work. The historical
ledger in [`docs/archive/prototype-roadmap.md`](archive/prototype-roadmap.md)
preserves completed foundation work and prior planned work so that it is not
lost during the 0.1.0 command-surface reset. Historical entries that mention
`observe`, `act`, or `verify` are retained as provenance, not as the future
public grammar.

## 12. External reusable CLI-contract dependencies

Idea: if Weaver consumes generic command-contract machinery from OrthoConfig
instead of rebuilding it locally, Weaver can focus on semantic code
capabilities while still inheriting consistent human and agent interfaces.

The dependency source is the OrthoConfig roadmap at
<https://raw.githubusercontent.com/leynos/ortho-config/refs/heads/main/docs/roadmap.md>.
 Weaver may build temporary adapters where needed, but any generic
command-contract implementation must either depend on the relevant OrthoConfig
task or record a deliberate divergence in ADR 007.

### 12.1. Track reusable dependency contracts

This step answers which command-contract pieces Weaver must consume from
OrthoConfig and which ones Weaver may temporarily adapt while the shared
contracts mature.

- [ ] 12.1.1. Track the downstream consumer boundary.
  - Depends on OrthoConfig 5.2.3.
  - Weaver dependency mode: blocked for final generic ownership decisions;
    local adapters may proceed when ADR 007 names the removal path.
  - Success: every Weaver command-contract task says whether it consumes
    OrthoConfig, wraps it, or intentionally diverges.
- [ ] 12.1.2. Consume recursive command metadata.
  - Depends on OrthoConfig 6.1.1 and 6.1.2.
  - Weaver dependency mode: local schema adapters may proceed, but generated
    help, manpage, completion, and context output must converge on the
    OrthoConfig recursive metadata shape.
- [ ] 12.1.3. Consume compact agent-context output and naming.
  - Depends on OrthoConfig 6.2.1, 6.2.2, and 6.2.3.
  - Weaver dependency mode: `weaver context --json` is blocked on the naming
    convention for final acceptance; a local payload fixture may proceed for
    Weaver-specific capability fields.
- [ ] 12.1.4. Consume skill manifest metadata and validation.
  - Depends on OrthoConfig 6.3.1 and 6.3.2.
  - Weaver dependency mode: skill prose may be drafted locally, but validation
    must use OrthoConfig metadata when available.
- [ ] 12.1.5. Consume canonical vocabulary policy.
  - Depends on OrthoConfig 7.1.1 through 7.1.3.
  - Weaver dependency mode: vocabulary linting is blocked for final CI gates;
    Weaver may maintain a temporary banned-name list for early migration.
- [ ] 12.1.6. Consume behavioural metadata for agent-native commands.
  - Depends on OrthoConfig 7.2.1 through 7.2.7.
  - Weaver dependency mode: renderer, JSON, exit-code, bounded-list, mutation,
    non-interactive, capability, and provenance metadata should be configured
    or extended in Weaver, not reimplemented as another generic framework.
- [ ] 12.1.7. Use `cargo-orthohelp` as the reference CLI for command contracts.
  - Depends on OrthoConfig 8.1.1 and 8.1.2.
  - Weaver dependency mode: this is a validation dependency; Weaver can build
    its own commands earlier, but must compare `--json` and enumerating-error
    behaviour against the reference CLI before 0.1.0.
- [ ] 12.1.8. Consume compounding primitive contracts.
  - Depends on OrthoConfig 9.1.1 through 9.3.3.
  - Weaver dependency mode: profile, delivery, feedback, and execution-ledger
    command semantics are Weaver-owned; reusable parsing, redaction, metadata,
    and ledger vocabulary come from OrthoConfig where available.

## 13. Human-friendly, agent-native 0.1.0 command-surface reset

Idea: if Weaver settles the generated command contract before more capabilities
land, later Sempai, plugin, graph, and workflow slices can converge on one
surface instead of repeatedly redesigning command grammar.

This foundational phase retires the prototype public grammar for the 0.1.0
target while preserving human usability, localization, accessibility, and the
UNIX pipeline model.

### 13.1. Ratify the reset boundary and adapter policy

This step answers which contracts Weaver owns and which ones OrthoConfig owns.
The outcome informs every command, renderer, and drift gate that follows. See
`docs/adr-007-agent-native-command-surface.md` and `docs/weaver-design.md` §2.1.

- [x] 13.1.1. Record the agent-native command-surface reset as ADR 007.
  - Requires 12.1.
  - Success: ADR 007 defines the dual renderer contract, capability routing,
    OrthoConfig dependencies, and the lack of compatibility promise for the
    prototype grammar.
- [ ] 13.1.2. Implement the Weaver command-surface adapter design.
  - Requires 13.1.1 and depends on OrthoConfig 5.2.3, 6.1, and 7.2.7.
  - Model resource path, verb, capability ID, mutability class, async class,
    selector forms, stream input support, provider policy, safety class,
    transaction behaviour, examples, output schemas, error schemas, and skill
    references.
  - Success: adding or renaming one Weaver command requires one adapter change
    and exposes enough metadata for router, help, docs, tests, and context
    fixtures.
- [ ] 13.1.3. Define the temporary-adapter removal policy.
  - Requires 13.1.2.
  - Success: every local generic command-contract helper names the OrthoConfig
    task expected to replace it or records a permanent divergence in ADR 007.

### 13.2. Enforce community vocabulary and resource-first commands

This step answers whether humans and agents can infer commands from common CLI
knowledge rather than Weaver-only vocabulary. See ADR 007 and
`docs/weaver-design.md` §§1.1 and 2.1.1.

- [ ] 13.2.1. Map prototype domains to resource-first command paths.
  - Requires 13.1.2.
  - Map definitions, references, diagnostics, cards, graph slices, symbols,
    patches, capabilities, context, jobs, profiles, and feedback.
  - Success: no current forward task requires adding a new public `observe`,
    `act`, or `verify` command.
- [ ] 13.2.2. Configure vocabulary linting.
  - Requires 13.2.1 and depends on OrthoConfig 7.1.1 through 7.1.3.
  - Include canonical verbs such as `get`, `list`, `create`, `update`,
    `delete`, `apply`, `run`, `prune`, `save`, `show`, `rename`, `move`, and
    `send`.
  - Success: CI rejects off-policy verbs and flags unless ADR 007 explicitly
    grandfathers them as current-state compatibility.
- [ ] 13.2.3. Migrate command examples in design and user-facing docs.
  - Requires 13.2.1.
  - Success: examples prefer `weaver definitions get`, `weaver references
    list`, `weaver diagnostics list`, `weaver symbols list`, `weaver symbols
    rename`, `weaver patches apply`, and `weaver context --json`.

### 13.3. Deliver dual renderers and bounded machine contracts

This step answers whether one command contract can serve accessible humans and
reliable agents without forking command behaviour. See `docs/weaver-design.md`
§§2.1.3 and 2.1.4.

- [ ] 13.3.1. Implement the human renderer contract.
  - Requires 13.1.2 and depends on OrthoConfig 7.2.2.
  - Include localized default output, `--plain`, `--color`, `--no-pager`,
    `--width`, TTY-sensitive progress, table headings, narrow-width labelled
    blocks, and ASCII fallbacks.
  - Success: human output does not rely on colour alone and never emits pager
    or spinner control flow in non-terminal contexts.
- [ ] 13.3.2. Implement universal `--json` and structured error output.
  - Requires 13.3.1 and depends on OrthoConfig 7.2.3 through 7.2.5 and 8.1.
  - Remove root `--output auto|human|json` and operation-local `--format` from
    the 0.1.0 target.
  - Success: success JSON is parseable on stdout, failure JSON is parseable on
    stderr, field names and error codes are non-localized, and exit classes are
    stable.
- [ ] 13.3.3. Implement enumerating errors and bounded list responses.
  - Requires 13.3.2 and depends on OrthoConfig 7.2.6 and 8.1.2.
  - Success: enum, registry, capability, profile, provider, delivery, and job
    validation errors list valid values; list-style commands expose bounded
    defaults, `--limit`, cursors, truncation markers, and narrowing hints.

### 13.4. Generate introspection, references, and drift gates

This step answers whether the command contract can stay synchronized as the
surface grows. See `docs/weaver-design.md` §2.1.4 and ADR 007.

- [ ] 13.4.1. Implement `weaver context --json`.
  - Requires 13.1.2 and depends on OrthoConfig 6.2.1 through 6.2.3.
  - Success: context output includes schema version, commands, flags, enum
    values, output schemas, error taxonomy, capabilities, provider summaries,
    profiles, jobs, delivery schemes, feedback state, and skill paths.
- [ ] 13.4.2. Implement `weaver capabilities list --json`.
  - Requires 13.4.1.
  - Success: runtime capability availability is separated from full command
    context and includes deterministic provider selection rationale.
- [ ] 13.4.3. Implement `weaver skill-path` and initial skill manifests.
  - Requires 13.4.1 and depends on OrthoConfig 6.3.
  - Success: skills teach workflows rather than command catalogues, and
    validation fails when a skill mentions unknown commands or flags.
- [ ] 13.4.4. Add generated artefact and drift gates.
  - Requires steps 13.1-13.4.
  - Generate or validate clap definitions, daemon router metadata, localized
    help, manpages, shell completions, docs snippets, JSON schema fixtures,
    vocabulary linting, and tests.
  - Success: CI fails when schema, router, help, docs, localization, context,
    skill manifests, or test fixtures drift.

## 14. Resource command slice: definitions, references, diagnostics, and cards

Idea: if existing LSP, Tree-sitter, and card foundations can be re-exposed
through the new generated surface, Weaver proves the reset without waiting for
new semantic engines.

This slice migrates useful read-only commands first. It gives humans and agents
immediate value while validating selectors, renderers, and capability
introspection end to end.

### 14.1. Re-expose LSP perceptors through resource commands

This step answers whether existing semantic backends fit the resource-first
surface without provider-specific commands. See `docs/weaver-design.md` §§2.2,
3.1, and 6.1.

- [ ] 14.1.1. Implement `weaver definitions get`.
  - Requires phase 13.
  - Success: position references return localized human output by default and
    stable JSON under `--json`, with provider provenance in machine output.
- [ ] 14.1.2. Implement `weaver references list`.
  - Requires 14.1.1.
  - Success: list output is bounded, cursor-aware, and suitable for downstream
    selector processing.
- [ ] 14.1.3. Implement `weaver diagnostics list`.
  - Requires 14.1.1.
  - Success: diagnostics preserve source ranges, severity, provider
    provenance, and actionable error classes in both renderer modes.

### 14.2. Re-expose card and graph-slice context

This step answers whether Jacquard-style cards and graph slices can become
first-class resource commands while preserving existing completed work. See
`docs/jacquard-card-first-symbol-graph-design.md` and `docs/weaver-design.md`
§3.3.

- [ ] 14.2.1. Implement `weaver cards get`.
  - Requires 14.1.1 and historical archive work 7.1.1 through 7.1.4.
  - Success: the command accepts position references and Sempai selectors where
    unambiguous, and returns stable card JSON with bounded enrichment.
- [ ] 14.2.2. Implement `weaver graph-slices get`.
  - Requires 14.2.1 and historical archive work 7.2.1.
  - Success: graph traversal exposes explicit budgets, truncation markers,
    provenance, and guidance for narrowing.
- [ ] 14.2.3. Add combinatorial read-command E2E coverage.
  - Requires steps 14.1-14.2.
  - Success: the suite covers human output, `--json`, `--plain`, bounded
    output, invalid enum errors, missing capabilities, and provider
    unavailable cases across the resource read commands.

### 14.3. Make Sempai one-liners first-class selectors

This step answers whether the future Sempai DSL can select one symbol or a
collection of symbols before full Sempai execution is complete. See
`docs/sempai-query-language-design.md` and `docs/weaver-design.md` §2.1.2.

- [ ] 14.3.1. Add selector record schemas for Sempai one-liners.
  - Requires 13.4.4 and historical archive work 4.1.1 through 4.1.5.
  - Success: selector records carry identity, range, language, query, capture,
    confidence, and provider provenance needed for safe downstream mutation.
- [ ] 14.3.2. Implement `weaver symbols list --query`.
  - Requires 14.3.1.
  - Success: `weaver symbols list --query 'fn $name(...)' --json` emits a
    bounded selector stream that ordinary UNIX filters can process.
- [ ] 14.3.3. Add selector stream compatibility checks.
  - Requires 14.3.2.
  - Success: commands consuming selector streams reject incompatible records
    with enumerating, structured errors rather than guessing.

## 15. Capability-routed mutation slice: symbols and patches

Idea: if symbol and patch mutations can run through one resource-first,
capability-routed transaction path, Weaver proves that agent-native commands do
not weaken its safety model.

This slice migrates the implemented patch and rename foundations under the new
grammar, then adds direct selector-based mutation and observe-to-act
composition.

### 15.1. Migrate patches and rename under the new grammar

This step answers whether existing safety-harness and actuator work can be
reused without exposing provider-first commands. See `docs/weaver-design.md`
§§4.1-4.3 and ADR 001.

- [ ] 15.1.1. Implement `weaver patches apply`.
  - Requires phase 13 and historical archive work 6.1.1 through 6.1.4.
  - Success: `patches apply` preserves Double-Lock verification, atomic
    transactions, `--dry-run`, structured safety results, and universal
    `--json`.
- [ ] 15.1.2. Implement `weaver symbols rename` for position references.
  - Requires 15.1.1 and historical archive work 5.2.1 through 5.2.5.
  - Success: provider routing remains capability-first, mutation results
    include transaction ID, affected paths, provider provenance, and safety
    outcome.
- [ ] 15.1.3. Implement `weaver symbols move` or `weaver symbols extract`.
  - Requires 15.1.2 and historical archive work 5.3 and 5.4.
  - Success: the public verb is canonical, the internal capability captures
    the richer operation, and no provider-specific command is required.

### 15.2. Prove selector-driven mutation and pipeline composition

This step answers whether observe-style resource commands and act-style
mutation commands compose through structured streams. See
`docs/weaver-design.md` §2.1.2.

- [ ] 15.2.1. Add direct Sempai selector support to symbol mutations.
  - Requires 14.3.2 and 15.1.2.
  - Success: `weaver symbols rename --query ...` handles zero, one, and many
    matches deterministically and requires explicit policy for ambiguous
    mutation.
- [ ] 15.2.2. Add `--from-stdin` selector stream consumption.
  - Requires 14.3.3 and 15.1.2.
  - Success: `weaver symbols list --query … --json | weaver symbols rename
    --from-stdin …` works without hidden state.
- [ ] 15.2.3. Add filtered pipeline E2E coverage.
  - Requires 15.2.2.
  - Success: at least one scenario pipes selector records through `jq` before
    mutation, and one scenario pipes observe-style output into a pager or other
    UNIX consumer without mutation.

### 15.3. Harden mutation boundaries for retries and destructive operations

This step answers whether agents can retry safely and humans can preview
consequential changes. See `docs/weaver-design.md` §§2.1.5 and 4.2.

- [ ] 15.3.1. Add idempotency keys and mutation transaction IDs.
  - Requires 15.1.1 and depends on OrthoConfig 7.2.1.
  - Success: repeated equivalent mutation submissions return the existing
    transaction or refusal rather than duplicating work.
- [ ] 15.3.2. Standardize `--dry-run` and `--force`.
  - Requires 15.3.1 and depends on OrthoConfig 7.2.1.
  - Success: all mutating commands declare preview and destructive-operation
    policy in the command-surface metadata.
- [ ] 15.3.3. Add mutation combinatorial E2E coverage.
  - Requires steps 15.1-15.3.
  - Success: coverage combines selector forms, `--json`, `--dry-run`,
    idempotency, provider failures, syntactic failures, semantic failures, and
    rollback assertions.

## 16. Async execution, profiles, delivery, and feedback

Idea: if Weaver gives agents durable identity, recoverable execution, and
structured artefact routing, long-running workflows collapse into fewer
reliable turns without becoming hostile to humans.

This phase adds compounding primitives after the core read and mutation loops
are stable.

### 16.1. Add durable jobs and `--wait`

This step answers whether async work can survive process loss and retry without
duplicate submissions. See `docs/weaver-design.md` §2.1.5.

- [ ] 16.1.1. Implement the Weaver job ledger.
  - Requires 15.3.1 and depends on OrthoConfig 9.3.
  - Success: XDG state stores job ID, command path, idempotency key, workspace,
    request hash, status, progress, timestamps, result pointer, and exit class.
- [ ] 16.1.2. Implement `weaver jobs list|get|prune`.
  - Requires 16.1.1.
  - Success: job lists are bounded, job lookup is structured, and prune is
    explicit and safe.
- [ ] 16.1.3. Add `--wait` to async-submitting commands.
  - Requires 16.1.2.
  - Success: submit-poll-collect workflows support backoff, jitter, timeout,
    cancellation, and ledger recovery.

### 16.2. Add profiles and persistent identity

This step answers whether repeated agent and human workflows can share durable
configuration without leaking secrets. See `docs/weaver-design.md` §2.1.5.

- [ ] 16.2.1. Implement profile storage and redaction.
  - Requires 13.4.1 and depends on OrthoConfig 9.1.
  - Success: profile names and metadata appear in `context --json`, while
    secret values remain redacted or represented as references.
- [ ] 16.2.2. Implement `weaver profiles save|list|show|delete`.
  - Requires 16.2.1.
  - Success: profile precedence is
    `built-in defaults < config files < selected profile < environment < flags`.
- [ ] 16.2.3. Add root `--profile <name>`.
  - Requires 16.2.2.
  - Success: explicit flags override profile values and invalid profile names
    enumerate available profiles.

### 16.3. Add two-way I/O

This step answers whether generated artefacts and friction reports can land
where users and agents need them. See `docs/weaver-design.md` §2.1.6.

- [ ] 16.3.1. Implement `--deliver stdout|file:<path>|webhook:<url>`.
  - Requires 13.3.2 and depends on OrthoConfig 9.2.1.
  - Success: file delivery is atomic, webhook delivery reports HTTP status,
    and unknown schemes enumerate valid schemes.
- [ ] 16.3.2. Implement `weaver feedback create|list|send`.
  - Requires 16.3.1 and depends on OrthoConfig 9.2.2.
  - Success: local feedback writes JSONL by default, upstream send is optional
    and configured, and feedback availability appears in `context --json`.
- [ ] 16.3.3. Add delivery and feedback E2E coverage.
  - Requires 16.3.1 and 16.3.2.
  - Success: tests cover stdout, atomic file, webhook success, webhook failure,
    unknown delivery schemes, local feedback, and configured upstream send.

## 17. Sempai and graph intelligence under the new grammar

Idea: if Sempai and graph expansion land after selectors and resource commands
are stable, they strengthen the same user workflows instead of creating a
parallel query subsystem.

This phase migrates the existing Sempai and Jacquard plans under `symbols`,
`cards`, and `graph-slices`.

### 17.1. Finish the Sempai execution engine

This step answers whether the query language can execute with stable
diagnostics and bounded behaviour. See `docs/sempai-query-language-design.md`.

- [ ] 17.1.1. Implement the one-liner lexer, parser, and recovery path.
  - Requires historical archive work 4.1.1 through 4.1.5.
  - Success: valid one-liners compile to canonical formula form, malformed
    input produces stable `E_SEMPAI_*` diagnostics, and recovery preserves
    partial anchors where safe.
- [ ] 17.1.2. Implement the Tree-sitter backend.
  - Requires 17.1.1 and historical archive work 4.2.
  - Success: Rust, Python, and TypeScript profiles support Semgrep-compatible
    pattern matching, metavariable unification, ellipsis, constraints, focus,
    and bounded execution controls.
- [ ] 17.1.3. Route Sempai execution through resource commands.
  - Requires 14.3.2 and 17.1.2.
  - Success: query execution feeds `symbols list`, cards, and graph workflows
    without adding a future public `observe query` command.

### 17.2. Complete graph-slice and history workflows

This step answers whether cards and graph slices can give agents compact,
bounded context across code structure and history. See
`docs/jacquard-card-first-symbol-graph-design.md`.

- [ ] 17.2.1. Complete graph-slice extraction and traversal.
  - Requires 14.2.2 and historical archive work 7.2.2 through 7.2.5.
  - Success: Tree-sitter inventory, LSP call edges, import/config edges, and
    budgeted traversal produce bounded graph-slice JSON and useful human
    summaries.
- [ ] 17.2.2. Implement graph history and risk deltas.
  - Requires 17.2.1 and historical archive work 7.3.
  - Success: history mode reconstructs slices per commit, reports normalized
    deltas, and keeps defaults safe for large repositories.
- [ ] 17.2.3. Implement probabilistic identity matching.
  - Requires 17.2.2 and historical archive work 7.4.
  - Success: matching emits reason codes, duplicate-name guardrails, calibrated
    confidence, and assignment decisions suitable for agents to inspect.

## 18. Plugin ecosystem behind capability contracts

Idea: if Weaver keeps providers behind capability contracts, it can add
specialist tools without forcing users or agents to learn backend-specific
commands.

This phase expands perceptors and actuators after the resource and mutation
contracts have proven the routing model.

### 18.1. Normalize existing and planned actuator capabilities

This step answers whether Rope, rust-analyzer, and later actuators can share
one public contract. See ADR 001, ADR 004, and ADR 006.

- [ ] 18.1.1. Publish capability migration notes for `symbol.rename`.
  - Requires historical archive work 5.2.1 through 5.2.5.
  - Success: docs explain provider provenance, refusal semantics, and the
    resource-first replacement for provider-required refactor commands.
- [ ] 18.1.2. Migrate extrication work to `symbol.move` or `symbol.extract`.
  - Requires 15.1.3 and historical archive work 5.3 and 5.4.
  - Success: public commands use canonical verbs while internal capabilities
    retain enough detail for Rope and rust-analyzer implementations.
- [ ] 18.1.3. Add additional actuator providers behind manifests.
  - Requires 18.1.2 and historical archive work 5.5.
  - Success: srgn or equivalent tools declare capability support without
    adding provider-specific public commands.

### 18.2. Add specialist perceptors and capability discoverability

This step answers whether read-only specialist providers improve results while
remaining transparent to ordinary users. See `docs/weaver-design.md` §4.1.

- [ ] 18.2.1. Deliver the first specialist perceptor provider.
  - Requires 14.1.3 and historical archive work 5.6.
  - Success: the provider enriches definitions, references, diagnostics,
    cards, or symbol matches through capability routing, not through a public
    provider command.
- [ ] 18.2.2. Wire provider summaries into `capabilities list` and
      `context --json`.
  - Requires 13.4.2 and depends on OrthoConfig 7.2.7.
  - Success: agents can discover availability, selected provider, and refusal
    reasons without parsing backend-specific help.
- [ ] 18.2.3. Refine graceful degradation guidance.
  - Requires 18.2.2 and historical archive work 5.8.
  - Success: unsupported capability errors suggest resource-first fallback
    workflows and enumerate valid alternatives.

## 19. Formal verification and safety hardening

Idea: if formal checks attach to the safety and routing kernels after their
public contracts settle, Weaver can prove the invariants that matter without
freezing prototype interfaces.

This phase preserves the existing formal-verification plan while tying it to
the new command contract and mutation slices.

### 19.1. Establish verifier tooling and proof contracts

This step answers which invariants are proved and which external-tool
behaviours remain trusted. See `docs/formal-verification-methods-in-weaver.md`.

- [ ] 19.1.1. Add pinned Kani and Verus tooling.
  - Requires phase 15.
  - Migrates historical archive work 8.1.
  - Success: verifier installs are reproducible and normal Rust workflows are
    unaffected unless a formal target is invoked.
- [ ] 19.1.2. Publish transaction, semantic-lock, and trust-boundary contracts.
  - Requires 19.1.1.
  - Migrates historical archive work 8.2.
  - Success: docs distinguish verified orchestration from trusted providers,
    language servers, parsers, and filesystems.

### 19.2. Verify the highest-risk owned kernels

This step answers whether Weaver's own write and routing decisions satisfy the
documented invariants. See `docs/weaver-design.md` §§4.2-4.3.

- [ ] 19.2.1. Add Kani checks for transactions and patch matching.
  - Requires 19.1.2 and migrates historical archive work 8.3.
  - Success: smoke harnesses prove commit gating, rollback bookkeeping,
    bounded path guardrails, and whole-command abort on unmatched patch blocks.
- [ ] 19.2.2. Add Kani and property checks for capability routing.
  - Requires 18.2.2 and 19.1.2.
  - Migrates historical archive work 8.4.
  - Success: selected providers satisfy language and capability predicates,
    and refusal is deterministic over bounded routing tables.
- [ ] 19.2.3. Add proof-only Verus kernels where they reduce long-term risk.
  - Requires 19.2.1 and 19.1.2.
  - Migrates historical archive work 8.5 and 8.6.
  - Success: proof modules remain outside the production API and prove only
    stable abstractions that will survive the 0.1.0 command reset.

## 20. Deferred extensions after the core 0.1.0 promise

Idea: if the core CLI contract is already trustworthy and boring to operate,
the project can evaluate broader extensions on product value instead of letting
them destabilize the main release.

### 20.1. Re-evaluate onboarding and interactive workflows

This step keeps useful agent workflow ideas without letting them bypass the
non-interactive contract. See `docs/weaver-design.md` §6.2.

- [ ] 20.1.1. Recast project onboarding under resource-first commands.
  - Requires phases 14 and 17.
  - Migrates historical archive work 6.2.1.
  - Success: onboarding consumes cards, graph slices, diagnostics, and
    dependency data without adding a parallel public command grammar.
- [ ] 20.1.2. Design explicit interactive review workflows.
  - Requires phase 15.
  - Migrates historical archive work 6.2.2.
  - Success: interaction is opt-in via `--interactive` or a dedicated review
    command and fails fast when stdin is not a terminal.

### 20.2. Evaluate MCP, SDK, and runtime explorer generation

This step defers generated integrations until the CLI contract they would wrap
is stable. See ADR 007 and OrthoConfig 10.1.

- [ ] 20.2.1. Decide whether to generate MCP descriptions from
      `context --json`.
  - Requires phase 13 and depends on OrthoConfig 10.1.1.
  - Success: the decision compares generated MCP descriptions against the
    command-surface token budget and records whether the feature belongs in
    Weaver 0.1.x.
- [ ] 20.2.2. Decide whether SDK or OpenAPI-shaped runtime explorers are in
      scope.
  - Requires 20.2.1 and depends on OrthoConfig 10.1.2.
  - Success: any accepted explorer uses the same command metadata and does not
    become a second source of truth.

## Archive

Historical prototype roadmap entries live in
[`docs/archive/prototype-roadmap.md`](archive/prototype-roadmap.md). Those
entries keep numbers `1` through `11`; this live roadmap reserves numbers `12`
through `20` for the forward ADR 007 build sequence.
