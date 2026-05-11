# Plan the agent-native Weaver documentation reset

This ExecPlan (execution plan) is a living document. The sections
`Constraints`, `Tolerances`, `Risks`, `Progress`, `Surprises & Discoveries`,
`Decision Log`, and `Outcomes & Retrospective` must be kept up to date as work
proceeds.

Status: DRAFT

This document must be maintained in accordance with `AGENTS.md` at the
repository root. It plans a documentation and roadmap overhaul only. It does
not authorise the product implementation until the plan is approved.

## Purpose / big picture

Weaver is pre-0.1.0, so the project can burn down prototype command grammar
without preserving backwards compatibility. The goal of the planned overhaul is
to update the design documents and roadmap so they give uncompromised,
buildable instructions for a human-friendly, agent-native Weaver.

After the overhaul, a reader should be able to open `docs/weaver-design.md`,
the new agent-native architectural decision record (ADR), and `docs/roadmap.md`
and see one coherent product direction:

- Weaver has one generated command contract with two first-class renderers.
- The default renderer remains friendly, localized, accessible, and pleasant
  for humans at a terminal.
- `--json` gives agents and scripts a stable, non-localized, parseable
  protocol surface.
- Weaver consumes reusable command-contract, documentation IR, agent-context,
  policy, profile, delivery, feedback, and execution-ledger contracts from
  OrthoConfig where that work exists or is already planned. Weaver only owns
  the application-specific semantic editing, capability routing, provider
  orchestration, safety, and user-facing resource grammar.
- Public commands are resource-first and community-consistent, not prototype
  `observe` / `act` / `verify` domains.
- Sempai one-liner queries are a first-class selector syntax for referencing
  one symbol or a collection of symbols anywhere a command accepts a symbol
  reference.
- Observe and act commands are composable: an observe command can emit bounded
  structured selector results, those results can flow directly into an act
  command or be filtered with ordinary UNIX tools first, and an act command can
  consume either the direct stream, the filtered stream, or a direct Sempai
  one-liner.
- Capability is the public abstraction. Rope, rust-analyzer, Language Server
  Protocol (LSP) servers, Tree-sitter, Sempai, and future helpers are provider
  implementations behind stable perceptor and actuator capabilities.
- The roadmap puts the command-surface reset before later command growth, so
  Sempai, plugins, graph work, and advanced workflows do not multiply the old
  patterns.

The observable outcome of this plan, when executed later, is a documentation
set in which every public-facing page, ADR, roadmap section, and gap analysis
uses the same language for the 0.1.0 command contract and gives implementation
teams a sequenced build plan.

## Constraints

- Do not remove the human interface. The design must keep localized help,
  manpages, shell completions, readable default output, accessible plain
  output, terminal-width-aware layouts, colour control, and explicit
  interactive review workflows.
- Do not preserve backwards compatibility for prototype public grammar.
  Pre-0.1.0 freedom means the docs should replace `observe` / `act` / `verify`,
  root `--output`, operation-local `--format`, root `--capabilities`, and
  provider-first `act refactor` with the final 0.1.0 contract.
- Do not expose provider names as the normal user workflow. Users ask Weaver
  for semantic work through resources and capabilities; providers appear in
  provenance, diagnostics, `--verbose` output, policy, and expert overrides.
- Do not duplicate work already planned in OrthoConfig. The revised Weaver
  design and roadmap must cite OrthoConfig roadmap tasks as dependencies for
  reusable command-contract machinery, and must keep Weaver's build tasks
  scoped to application-specific behaviour.
- Do not delete existing planned or historically completed Weaver roadmap work
  just because the UI redesign changes the public grammar. Preserve completed
  entries as historical foundation, re-home still-relevant planned work under
  the new command surface, and only remove or supersede items when the redesign
  makes them irrelevant to future functionality. Any removal must include a
  short rationale in the roadmap.
- Keep protocol identifiers stable and non-localized. Localize prose, help,
  human headings, examples, and recovery guidance, but not JSON field names,
  schema versions, error codes, enum values, capability IDs, or exit classes.
- Preserve the existing safety principles. All actuator output still flows
  through Weaver-owned transaction, sandbox, Double-Lock, atomic-write,
  idempotency, and rollback semantics.
- Preserve UNIX composability as a product requirement. The redesigned public
  grammar must support direct selectors, streamed selector input, filtering,
  paging, and JSON mode without forcing agents or humans through hidden session
  state.
- Keep the docs self-consistent. Any new ADR, roadmap phase, crate/module
  name, command name, or storage path must be reflected in `docs/contents.md`,
  `docs/repository-layout.md`, `docs/users-guide.md`, `README.md`, and related
  ADRs where appropriate.
- Use en-GB Oxford spelling in documentation, except for external API names
  and literal identifiers.
- Validate Markdown with `make fmt`, `make markdownlint`, and `make nixie`.
  Run `make check-fmt`, `make lint`, and `make test` before committing the
  final documentation overhaul, because repository instructions require the
  full gates before commits.
- Do not run formatting, linting, or tests in parallel. Use `tee` and write
  logs under `/tmp` with branch-specific names.

## Tolerances (exception triggers)

- Scope: if the documentation overhaul needs changes to more than 14
  documentation files, or more than 2,500 net documentation lines, stop and
  confirm the expanded scope.
- Interface: if a proposed command shape conflicts with an already completed
  implementation in a way that would require immediate code changes to keep the
  docs truthful, stop and present options.
- Architecture: if capability-first plugins cannot be described without
  exposing provider-specific public commands, stop and escalate with the failed
  abstraction boundary.
- Roadmap: if the new roadmap cannot be sequenced so the command-surface reset
  lands before further Sempai, plugin, or graph command growth, stop and
  present the dependency conflict.
- OrthoConfig: if a planned Weaver task appears to implement generic
  functionality already assigned to OrthoConfig, stop and convert it into a
  dependency, temporary adapter, or explicit divergence decision.
- Roadmap history: if the revised roadmap would delete completed Weaver work,
  or planned work that remains relevant after the UI redesign, stop and record
  a preservation, migration, or supersession plan before editing.
- Dependencies: if executing this documentation plan appears to require a new
  crate or external dependency immediately, stop and separate documentation
  planning from product implementation.
- Validation: if Markdown or full repository gates still fail after three
  repair loops, stop and record the failing log paths in `Decision Log`.
- Ambiguity: if later review changes the product direction between
  "agent-native human-friendly CLI" and "agent-only API appliance", stop and
  ask for an explicit product decision.

## Risks

- Risk: the current docs already describe shipped `observe`, `act`, and
  `verify` commands, so replacing them in design language may temporarily make
  the docs aspirational rather than descriptive. Severity: high. Likelihood:
  high. Mitigation: label the reset as the 0.1.0 target, update the roadmap
  before user-facing examples, and keep the current-state notes explicit until
  implementation catches up.

- Risk: "agent-native" may be misread as "human-hostile." Severity: high.
  Likelihood: medium. Mitigation: put the dual-renderer contract and
  accessibility rules in the ADR and design document before any machine-only
  details.

- Risk: capability, provider, perceptor, and actuator terminology can become
  confusing if the docs use them inconsistently. Severity: medium. Likelihood:
  high. Mitigation: define the terms once in the new ADR, then use the same
  vocabulary in design, roadmap, user guide, and developer guide.

- Risk: the roadmap may become a horizontal layer cake around schema,
  renderer, router, and plugin internals. Severity: medium. Likelihood: medium.
  Mitigation: keep the first reset phase foundational, then frame later phases
  as vertical slices that deliver usable resource commands end to end.

- Risk: Weaver may duplicate OrthoConfig's planned generic CLI contract work,
  creating two divergent schema and policy systems. Severity: high. Likelihood:
  medium. Mitigation: make OrthoConfig task dependencies explicit in ADR 007,
  `docs/weaver-design.md`, and `docs/roadmap.md`; define only Weaver-owned
  adapters and semantic capability metadata locally.

- Risk: the roadmap rewrite may erase useful historical or planned Weaver work
  while replacing the UI grammar. Severity: high. Likelihood: medium.
  Mitigation: preserve completed work as historical foundation, migrate
  relevant future work into the new phase structure, and mark superseded work
  with rationale instead of silently deleting it.

- Risk: `agent-context`, skills, jobs, profiles, delivery, and feedback could
  be documented as isolated conveniences instead of parts of the agent-native
  product shape. Severity: medium. Likelihood: medium. Mitigation: describe
  each as an agent state, introspection, or two-way I/O surface in the product
  rationale and roadmap acceptance criteria.

- Risk: provider hiding may go too far and make failures opaque. Severity:
  medium. Likelihood: medium. Mitigation: keep provider names out of ordinary
  command syntax but include provider provenance in JSON, diagnostics,
  `agent-context`, and verbose human output.

## Progress

- [x] (2026-05-09) Read repository `AGENTS.md`, branch name, and skill
      instructions for ExecPlans, roadmaps, and commits.
- [x] (2026-05-09) Inspected `docs/weaver-design.md`, `docs/roadmap.md`,
      `docs/users-guide.md`, `docs/ui-gap-analysis.md`,
      `docs/repository-layout.md`, `docs/contents.md`, and `README.md`.
- [x] (2026-05-09) Inspected ADRs 001, 004, and 006 to verify that the
      capability-first plugin model already supports a provider-hidden public
      command surface.
- [x] (2026-05-09) Used a read-only wyvern agent to inventory current
      documentation contradictions and supporting design material.
- [x] (2026-05-09) Drafted this ExecPlan.
- [x] (2026-05-09) Ran the plan-only gates: `make fmt`,
      `make markdownlint`, `make nixie`, `make check-fmt`, `make lint`, and
      `make test`.
- [x] (2026-05-11) Fetched the OrthoConfig roadmap and amended this plan so the
      Weaver design and roadmap rewrite depends on OrthoConfig tasks rather
      than duplicating reusable command-contract work.
- [x] (2026-05-11) Ran the amendment gates: `make fmt`,
      `make markdownlint`, `make nixie`, `make check-fmt`, `make lint`, and
      `make test`.
- [x] (2026-05-11) Added first-class Sempai one-liner selector and
      observe/filter/act pipeline requirements, then reran `make fmt`,
      `make markdownlint`, `make nixie`, `make check-fmt`, `make lint`, and
      `make test`.
- [ ] Obtain explicit approval before executing the planned documentation
      overhaul.
- [ ] Execute the documentation overhaul milestone by milestone, updating this
      plan as discoveries occur.
- [ ] Run documentation and repository gates.
- [ ] Commit the completed overhaul after all required gates pass.

## Surprises & Discoveries

- Discovery: the current design already says the command catalogue should be
  shared by the router, contextual help, `weaver help`, and tests. The reset
  should promote that from help hygiene into the central schema-backed command
  contract.

- Discovery: `crates/weaver-cli/src/cli.rs` still has
  `disable_help_subcommand = true` and a root `--output auto|human|json` model.
  Those are useful evidence of prototype grammar that the design reset must
  replace, not behaviours to carry forward.

- Discovery: `docs/users-guide.md` still describes `act refactor` as requiring
  `--provider`, even though the completed rename-symbol capability work makes
  provider selection optional and capability-routed. The overhaul must remove
  provider-first examples from ordinary workflows.

- Discovery: `README.md` says the workspace has five crates and that the
  Double-Lock safety harness is still under development. That conflicts with
  `docs/repository-layout.md` and the completed roadmap entries, so the public
  narrative already needs trust repair independent of the agent-native reset.

- Discovery: ADR 001, ADR 004, and ADR 006 already justify the desired plugin
  model. The planned work should refine and generalize those decisions rather
  than reversing them.

- Discovery: `make fmt` exposed a pre-existing Markdown wrapping problem in
  `docs/developers-guide.md`. The plan-only change includes a narrow wording
  cleanup there so the repository documentation gate can pass.

- Discovery: the active OrthoConfig roadmap already plans reusable
  agent-native CLI infrastructure for downstream consumers including Weaver:
  whole-CLI introspection, compact agent-context generation, downstream
  `context --json` naming, skill manifest validation, vocabulary policy, dual
  renderer metadata, JSON stream contracts, exit-code metadata, bounded-list
  metadata, generic capability and provenance metadata, profile contracts,
  delivery and feedback contracts, and execution-ledger contracts. Weaver's
  plan must consume or depend on those tasks instead of reimplementing them as
  local generic framework work.

## Decision Log

- Decision: treat "scorched earth" as removal of accidental public grammar,
  drift, and prototype compatibility, not removal of human usability.
  Rationale: the user explicitly corrected that Weaver must remain friendly,
  accessible, localizable, and grounded in community CLI conventions. Date:
  2026-05-09.

- Decision: design one generated command contract with two first-class
  renderers. Rationale: humans and agents should share one source of truth,
  while the default human renderer and `--json` machine renderer can optimize
  for different consumers without drifting. Date: 2026-05-09.

- Decision: replace public `observe` / `act` / `verify` domains with
  resource-first commands in the 0.1.0 target documentation. Rationale:
  resource-first names such as `definitions get`, `references list`,
  `diagnostics list`, `symbols rename`, and `patches apply` better match
  community CLI vocabulary and are easier for both humans and agents to guess.
  Date: 2026-05-09.

- Decision: remove root `--output` and operation-local `--format` from the
  target contract. Rationale: `--json` should be the canonical machine switch;
  default output should be human; `--plain`, `--color`, `--no-pager`, and
  `--width` should control human rendering without changing protocol shape.
  Date: 2026-05-09.

- Decision: replace root `--capabilities` with explicit introspection
  resources. Rationale: the agent-context payload should describe the full
  command and workflow surface, while `weaver capabilities list --json` can
  describe runtime capability availability. The public context command name
  should follow OrthoConfig roadmap task 6.2.3, which prefers
  `<tool> context --json` before first release. Date: 2026-05-09. Updated:
  2026-05-11.

- Decision: keep capability-based perceptor and actuator plugins as a core
  design pillar. Rationale: the user asked whether this still fits; the answer
  is yes, provided capability is public and providers remain implementation
  details behind deterministic routing, refusal diagnostics, provenance, and
  expert policy overrides. Date: 2026-05-09.

- Decision: make Sempai one-liner queries a first-class selector form.
  Rationale: the user clarified that one-liners must reference a symbol or
  collection of symbols directly, and must compose with observe/filter/act
  pipelines as well as direct act commands. Position references remain a peer
  selector form, not the only first-class path. Date: 2026-05-11.

- Decision: do not use Firecrawl for the initial plan draft. Rationale: the
  full conversation and the target repository docs are available locally, and
  the task is to encode the final decisions from that conversation into a
  repository plan rather than to perform new web research. Date: 2026-05-09.

- Decision: make OrthoConfig the dependency for reusable command-contract
  infrastructure and make Weaver the owner of semantic editing behaviour.
  Rationale: the OrthoConfig roadmap explicitly assigns reusable documentation
  IR, whole-CLI introspection, agent-context schema, policy linting, renderer
  metadata, profile, delivery, feedback, and execution ledger contracts to
  OrthoConfig, while naming Weaver as a downstream consumer. Weaver should not
  fork that generic machinery unless a dependency is unavailable and an ADR
  records the temporary adapter or divergence. Date: 2026-05-11.

- Decision: preserve existing Weaver roadmap history during the redesign.
  Rationale: the user explicitly required that existing planned work and
  historical completed work not be deleted unless no longer relevant to future
  functionality after the UI redesign. Completed safety, atomic-edit,
  capability-routing, card, graph, and help work should be retained as
  foundation or migrated into the new command grammar. Date: 2026-05-11.

## Context and orientation

The files most likely to be edited when this plan is approved are:

- `docs/adr-007-agent-native-command-surface.md`, a new ADR to record the
  reset decision.
- `docs/weaver-design.md`, the primary architecture and product rationale.
- `docs/roadmap.md`, the implementation sequence that must be reordered around
  the command-surface reset while preserving relevant completed and planned
  work.
- `docs/ui-gap-analysis.md`, the gap inventory that should become an
  agent-native and human-friendly audit rather than a historical prototype
  issue list.
- `docs/users-guide.md`, the operator-facing guide and examples.
- `docs/developers-guide.md`, the contributor-facing contract guidance.
- `docs/repository-layout.md`, the ownership map for any planned
  command-surface, agent-context, skill, profile, job, delivery, and feedback
  components, including which pieces are OrthoConfig dependencies and which
  pieces are Weaver-owned adapters.
- `docs/contents.md`, the documentation index.
- `README.md`, the public summary that must stop contradicting implemented
  safety and workspace status.
- ADR 001, ADR 004, and ADR 006 if the new ADR needs cross-links or wording
  that clarifies provider-hidden capability routing.

The current source material to preserve includes these existing decisions:

- ADR 001 says user intent should be represented by stable capability IDs,
  provider choice should be resolved internally, provider internals should stay
  out of normal workflows, and final edits should flow through the safety
  harness.
- ADR 004 says provider routing must be deterministic and refusal diagnostics
  must be stable and machine-readable.
- ADR 006 says plugins use one-shot JSONL execution with broker ownership and
  do not own final commit behaviour.
- `docs/weaver-design.md` already separates stable daemon reason codes from
  localized CLI prose.
- `docs/roadmap.md` already contains completed safety, atomic-edit,
  capability-routing, and card/graph groundwork that should be reused rather
  than redesigned.
- OrthoConfig roadmap task 5.2.3 records the consumer boundary: OrthoConfig
  owns reusable command-contract machinery, while Weaver owns semantic code
  editing and execution.
- OrthoConfig roadmap tasks 6.1 and 6.2 own recursive subcommand metadata,
  compact agent-context output, and downstream context-command naming.
- OrthoConfig roadmap task 6.3 owns reusable skill manifest metadata and
  validation against real command paths and flags.
- OrthoConfig roadmap tasks 7.1 and 7.2 own reusable vocabulary policy,
  behavioural semantics, dual-renderer metadata, JSON mode stream contracts,
  exit-code metadata, bounded-list metadata, and generic capability/provenance
  metadata.
- OrthoConfig roadmap tasks 9.1, 9.2, and 9.3 own reusable profile, delivery,
  feedback, and execution-ledger contracts. Weaver owns the semantic contents,
  storage policy choices, and safety integration for those surfaces.

## Product rationale to encode

The planned overhaul must state the product rationale before the mechanics.
Weaver should be agent-native because agents increasingly use CLIs as primary
interfaces, and every inconsistent flag, prompt, vague error, unbounded output,
or hidden backend choice costs tokens, retries, and reliability. Weaver should
also stay human-friendly because the best agent-native CLI practices are old
CLI practices made explicit: predictable names, useful help, parseable output,
good errors, bounded responses, standard streams, completions, manpages, and
recoverable workflows.

The reset should describe this as one product bet:

```plaintext
If Weaver defines one schema-backed command contract and generates both human
and machine surfaces from it, the product can remain pleasant for terminal
users while giving agents a stable, introspectable, recoverable interface.
```

The docs must make clear that this is not a Cloudflare or HeyGen clone. Weaver
keeps its UNIX, daemon, JSONL, sandbox, semantic-fusion, and Double-Lock
identity. The change is that the public command surface becomes generated,
bounded, introspectable, state-aware, and capability-routed by construction.

## Target command contract

The design documents should replace the prototype public grammar with a
resource-first command grammar. The exact command set can evolve during
implementation, but the documentation reset should use these examples as the
0.1.0 target:

```sh
weaver definitions get --uri file:///src/main.rs --position 10:5
weaver references list --uri file:///src/main.rs --position 10:5
weaver diagnostics list --workspace .
weaver cards get --uri file:///src/main.rs --position 10:5
weaver graph-slices get --uri file:///src/main.rs --position 10:5
weaver symbols list --query 'fn $name(...)'
weaver symbols rename --query 'fn process_request(...)' --new-name run_request
weaver symbols rename --uri file:///src/main.rs --position 10:5 --new-name run
weaver symbols move --uri file:///src/main.rs --position 10:5 --to src/runner.rs
weaver symbols list --query 'fn $name(...)' --json \
  | jq 'select(.name | test("_old$"))' \
  | weaver symbols rename --from-stdin --suffix ""
weaver patches apply --file changes.patch --dry-run
weaver context --json
weaver jobs list --json
weaver profiles list --json
weaver feedback create "the enum error did not list valid values"
```

Canonical verbs are `get`, `list`, `create`, `update`, `delete`, `apply`,
`run`, `prune`, `save`, `show`, and `rename`. Banned or non-canonical public
forms include `info`, `ls`, `--skip-confirmations`, operation-local
`--format=json`, root `--output json`, and provider-named commands such as
`weaver rope rename`.

The design should permit standard ancestral flags where the convention is
strong:

```plaintext
-h, --help
-V, --version
-v, --verbose
-q, --quiet
```

Global human rendering flags should include:

```plaintext
--plain
--color auto|always|never
--no-pager
--width <columns>
--locale <tag>
```

The canonical machine switch is:

```plaintext
--json
```

All data-returning commands accept `--json`. In JSON mode, success writes only
the operation result to stdout. Failure writes a structured error object to
stderr and exits with a stable non-zero exit class.

## Selector and pipeline model

The planned design must define selectors as first-class inputs. A selector is
any stable way to identify one symbol or a collection of symbols. The initial
selector forms are:

- Sempai one-liner queries, passed with a canonical flag such as
  `--query <sempai-one-liner>`.
- Position references, passed with `--uri` and `--position`.
- Structured selector streams, read from stdin when `--from-stdin` or an
  equivalent explicit stream flag is present.

Sempai one-liners are not a secondary search feature. They are a peer to
position references. Any command that acts on one or more symbols must state
which selector forms it accepts and how it handles zero, one, and many matches.

Observe commands must be able to emit selector records that act commands can
consume without bespoke glue. The roadmap and design should preserve these
composition shapes:

```sh
weaver symbols list --query 'fn $name(...)' --json \
  | weaver symbols rename --from-stdin --suffix _renamed

weaver symbols list --query 'fn $name(...)' --json \
  | jq 'select(.name | startswith("old_"))' \
  | weaver symbols rename --from-stdin --replace-prefix old_ --with-prefix new_

weaver symbols rename --query 'fn process_request(...)' --new-name run_request

weaver symbols rename \
  --uri file:///src/main.rs \
  --position 10:5 \
  --new-name run_request

weaver symbols list --query 'class $name' --json | less
```

The exact command and flag names may change during design review, but the
capabilities may not collapse back to a position-only model. This requirement
is about the interaction contract: observe results are useful downstream act
inputs, act commands can resolve direct query selectors, and ordinary UNIX
filters may sit between them when the caller wants narrowing or transformation.

## Capability and plugin model

The docs must define four terms once and use them consistently:

- A capability is a stable semantic contract, such as `definition.get`,
  `references.list`, `diagnostics.list`, `symbol.rename`, `symbol.move`, or
  `patch.apply`.
- A perceptor is a read-only provider that observes the codebase and returns
  facts, diagnostics, cards, graph slices, matches, or provenance.
- An actuator is a mutation-planning provider that returns edits, diffs,
  patches, plans, or workspace-change proposals. It never commits directly.
- A provider is an implementation of one or more capabilities, such as Rope,
  rust-analyzer, Tree-sitter, an LSP server, Sempai, or a Weaver built-in.

The ordinary command path should be capability-first:

```sh
weaver symbols rename --uri file:///src/lib.rs --position 42:9 --new-name run
```

It should not be provider-first:

```sh
weaver act refactor --provider rust-analyzer --refactoring rename ...
```

Provider selection follows deterministic precedence:

1. explicit CLI override,
2. selected profile policy,
3. environment or config policy,
4. workspace policy,
5. provider priority from manifests,
6. deterministic tie-breaker,
7. structured refusal if no provider qualifies.

Expert overrides may exist, but they are advanced policy:

```sh
weaver symbols rename \
  --uri file:///src/main.py \
  --position 10:5 \
  --new-name run \
  --provider rope
```

Human output should hide providers during routine success unless `--verbose` is
used. JSON output should include provider provenance because it matters for
debugging and reproducibility. Failures should explain provider constraints
when they affect the outcome, without requiring the caller to know provider
internals ahead of time.

## Required document changes

### Milestone 1: Record the reset as a new ADR

Create `docs/adr-007-agent-native-command-surface.md`.

The ADR must decide that Weaver's 0.1.0 command surface is generated from one
schema-backed command contract. The ADR must state that Weaver depends on
OrthoConfig for reusable command-contract, documentation IR, agent-context,
policy, profile, delivery, feedback, and execution-ledger contracts wherever
the OrthoConfig roadmap provides them. Weaver's local contract layer feeds CLI
parsing, daemon routing metadata, semantic capability mapping, provider
selection, safety policy, docs snippets, JSON schemas, and tests for
Weaver-owned behaviour.

The ADR must include an explicit dependency table:

- OrthoConfig 5.2.3 for downstream consumer boundaries.
- OrthoConfig 6.1 for recursive command metadata.
- OrthoConfig 6.2 for compact agent-context output and public
  `weaver context --json` naming.
- OrthoConfig 6.3 for skill manifest metadata and validation.
- OrthoConfig 7.1 for vocabulary policy.
- OrthoConfig 7.2 for renderer, JSON stream, exit-code, bounded-list, and
  generic capability/provenance metadata.
- OrthoConfig 9.1 for profile contracts and redaction metadata.
- OrthoConfig 9.2 for delivery and feedback contracts.
- OrthoConfig 9.3 for execution-ledger contracts.

The ADR must explicitly preserve human usability. It should say that the
default command path is human-readable and localized, while `--json` is the
stable machine path. It should state that accessibility, localization, and
community CLI conventions are product requirements, not afterthoughts.

The ADR must burn down these prototype surfaces for the 0.1.0 target:

- public `observe` / `act` / `verify` domain grammar,
- root `--output auto|human|json`,
- operation-local `--format`,
- root `--capabilities`,
- provider-required `act refactor`,
- hand-maintained command catalogues,
- implicit prompts,
- unbounded list output,
- provider-specific public commands.

The ADR must also state that the redesign preserves existing Weaver roadmap
history. Completed work remains as foundation, relevant planned work is
migrated under the new public grammar, and removed items must be marked
superseded with rationale.

Success criteria:

- ADR 007 defines the dual-renderer command contract.
- ADR 007 defines capability, perceptor, actuator, and provider.
- ADR 007 links ADR 001, ADR 004, and ADR 006 as supporting decisions.
- ADR 007 links the OrthoConfig roadmap tasks that Weaver depends on and does
  not claim ownership of those reusable contracts.
- ADR 007 contains no compatibility promise for the prototype grammar.

### Milestone 2: Rewrite the design around the new command contract

Update `docs/weaver-design.md`.

The executive summary and vision must describe Weaver as a human-friendly,
agent-native semantic CLI rather than only an agent-friendly JSONL primitive.
Keep the UNIX and JSONL rationale, but clarify that JSONL is an internal daemon
and provider transport, while public CLI JSON mode uses stable operation result
schemas.

Replace the current command examples with the resource-first grammar from
`Target command contract`. The design may mention old names only in a
current-state or migration note.

Add a section named `Agent-native command surface`. It must specify:

- OrthoConfig as the owner of reusable documentation IR, agent-context, policy,
  renderer, profile, delivery, feedback, and execution-ledger contracts.
- A Weaver-owned command-surface adapter only for application-specific
  semantics: resource path, verb, capability ID, mutability class, async class,
  selector forms, stream input support, provider selection policy, safety
  class, transaction behaviour, examples, output schemas, error schemas, and
  skill references.
- Generated or validated outputs: clap definitions, daemon router metadata,
  localized help, manpages, shell completions, docs snippets, `context --json`
  agent-context payloads, skill manifests, JSON Schema fixtures, vocabulary
  linting, and tests.
- The mechanical rule that adding or renaming a Weaver command requires one
  application schema or OrthoConfig metadata change, and CI fails on drift.
- The fallback rule that any local generic command-contract code is temporary
  unless ADR 007 records why an OrthoConfig dependency cannot satisfy it.

Add a section named `Selector and pipeline contract`. It must define:

- Sempai one-liner selectors for one symbol or a symbol collection,
- position-reference selectors for editor-style point operations,
- structured selector streams for observe-to-act composition,
- explicit stdin consumption through `--from-stdin` or the final canonical
  equivalent,
- JSON selector record schemas that preserve enough provenance for safe
  mutation,
- zero, one, and many match semantics for every selector-consuming command,
- the invariant that observe command output can feed compatible act commands
  either directly or after ordinary UNIX filtering.

Add sibling sections named `Human renderer contract` and
`Machine renderer contract`.

The human renderer contract must require:

- localized default output,
- `--plain`, `--color`, `--no-pager`, and `--width`,
- no meaning conveyed by colour alone,
- no spinner unless stderr is a terminal,
- progress and diagnostics on stderr,
- no pager in non-terminal contexts,
- table headings and narrow-width labelled-block fallbacks,
- Unicode decoration with ASCII fallbacks,
- examples that can be copied and run.

The machine renderer contract must require:

- universal `--json`,
- operation result JSON on stdout for success,
- structured error JSON on stderr for failure,
- non-localized field names, enum values, schema versions, and error codes,
- stable exit-code taxonomy,
- no localized prose required for an agent to decide its next action.

Add a section named `Structured introspection and skills`. It must define:

```sh
weaver context --json
weaver capabilities list --json
weaver skill-path
```

`context --json` returns the OrthoConfig-shaped agent-context payload: CLI
version, schema version, commands, flags, enum values, output schemas, error
taxonomy, installed capabilities, provider summaries, profiles, jobs, delivery
schemes, feedback state, and skill paths. The design must call out its
dependency on OrthoConfig 6.2.3 for public command naming and must not add a
public `agent-context` alias before 0.1.0 unless ADR 007 records the reason.
`capabilities list` returns Weaver runtime capability availability.
`skill-path` returns the directory containing workflow `SKILL.md` manifests,
with validation delegated to OrthoConfig 6.3 where possible.

Add a section named `Agent state and recoverable workflows`. It must define:

- `weaver jobs list|get|prune`,
- `--wait` on async-submitting commands,
- a durable XDG state job ledger,
- idempotency keys for mutations and async submissions,
- `weaver profiles save|list|show|delete`,
- root `--profile <name>`,
- precedence
  `built-in defaults < config files < selected profile < environment < flags`,
- profile secrecy rules that expose names and metadata but not secrets.

This section must explicitly depend on OrthoConfig 9.1 for profile contract and
redaction metadata, and OrthoConfig 9.3 for reusable execution-ledger metadata.
Weaver owns the job record contents, workspace safety integration, idempotency
semantics, storage path choice, and public `jobs` command behaviour.

Add a section named `Two-way I/O`. It must define:

- `--deliver stdout`,
- `--deliver file:<path>` with atomic writes,
- `--deliver webhook:<url>` with surfaced HTTP status,
- structured refusal for unknown schemes,
- `weaver feedback create|list|send`,
- local JSONL feedback by default,
- optional upstream feedback only when configured,
- feedback availability in the `context --json` agent-context payload.

This section must explicitly depend on OrthoConfig 9.2 for reusable delivery
target parsing and feedback storage contracts. Weaver owns the semantic
payloads, safety checks, webhook payload shape, and how delivered artefacts
relate to code-edit transactions.

Add a section named `Capability-routed perceptors and actuators`. It must
generalize ADR 001, ADR 004, and ADR 006:

- public commands map to capability IDs,
- provider manifests declare capability support,
- perceptors are read-only,
- actuators produce proposed edits and never commit directly,
- perceptors that return symbol collections produce selector records suitable
  for downstream actuator input,
- actuators may accept Sempai one-liner selectors directly when their
  capability supports query resolution,
- the broker owns routing, safety, idempotency, jobs, and final rendering,
- provider provenance is available for debugging without making provider names
  the normal command surface.

Success criteria:

- `docs/weaver-design.md` no longer presents `observe` / `act` / `verify` as
  the future public grammar.
- It clearly separates internal JSONL transport from public `--json` output.
- It documents both human and machine rendering as generated schema outputs.
- It explains why capability-first plugins still fit the design.

### Milestone 3: Rewrite the roadmap around the build sequence

Update `docs/roadmap.md` using the roadmap skill conventions. The roadmap
should keep completed historical entries where useful, but the forward plan
must be reordered around the reset. The rewrite must not delete existing
planned work or completed historical work unless the UI redesign makes that
work irrelevant to future functionality. When work is superseded, the roadmap
must say what superseded it and why.

Before adding new Weaver tasks, add a dependency section named something like:

```plaintext
External reusable CLI-contract dependencies
```

That section must cite the OrthoConfig roadmap tasks Weaver depends on:

- 5.2.3 for the Weaver/Netsuke downstream boundary.
- 6.1 for recursive subcommand metadata.
- 6.2.1 and 6.2.2 for compact agent-context generation and schema stability.
- 6.2.3 for the public `weaver context --json` naming convention.
- 6.3 for skill manifest metadata and validation.
- 7.1 for canonical vocabulary policy.
- 7.2.1 through 7.2.7 for non-interactive, mutation, renderer, JSON stream,
  exit-code, bounded-list, and capability/provenance metadata.
- 8.1 for the `cargo-orthohelp` reference CLI proving `--json` and
  enumerating errors.
- 9.1 for profile contracts and redaction metadata.
- 9.2 for delivery and feedback contracts.
- 9.3 for execution-ledger contracts.

Each dependent Weaver task must say whether it is blocked by the OrthoConfig
task, can proceed with a local adapter, or can proceed because the OrthoConfig
contract is only needed for later validation.

The first new forward phase should be foundational:

```plaintext
Human-friendly, agent-native 0.1.0 command surface reset
```

Its idea should be falsifiable:

```plaintext
If Weaver settles the generated command contract before more capabilities
land, later Sempai, plugin, graph, and workflow slices can converge on one
surface instead of repeatedly redesigning command grammar.
```

This phase must contain review-sized tasks for:

- ADR 007 acceptance,
- OrthoConfig dependency mapping and adapter policy,
- Weaver semantic command-surface adapter design,
- vocabulary linting via OrthoConfig policy where available,
- resource-first command mapping,
- dual renderers,
- universal `--json`,
- stable exit-code taxonomy,
- enumerating errors,
- bounded list responses,
- `context --json` agent-context payload,
- Sempai one-liner selector support,
- observe-to-act pipeline composition,
- capability introspection,
- skill manifests,
- manpage and completion generation,
- schema-to-router, schema-to-help, schema-to-docs, and schema-to-tests drift
  gates.

Tasks in this foundational phase must distinguish reusable OrthoConfig work
from Weaver work. For example, Weaver may configure or consume vocabulary
policy and renderer metadata, but it should not implement a second generic
policy engine unless ADR 007 records a temporary adapter with a removal path.

The next phase should deliver a useful vertical slice under the new grammar,
not another layer. Candidate phase:

```plaintext
Resource command slice: definitions, references, diagnostics, and cards
```

This phase should prove that existing LSP, Tree-sitter, and cards work can be
re-exposed through the new generated surface with both human and JSON output.
It should also prove that Sempai one-liner selectors can produce symbol
collections that flow through `symbols list` or another observe-style resource
command into ordinary UNIX filters.

The following phase should deliver safe mutation under the new grammar:

```plaintext
Capability-routed mutation slice: symbols and patches
```

This phase should include `symbols rename`, `symbols move` or
`symbols extract`, `patches apply`, `--dry-run`, `--force`, idempotency,
transaction IDs, provider provenance, Double-Lock integration, and structured
refusals. It must include direct act commands using both Sempai one-liner
selectors and position references, plus at least one observe-to-act pipeline
where structured selector records flow directly into mutation and one where
they are filtered before mutation.

Then add phases for:

- async-aware execution and the durable jobs ledger,
- profiles and persistent agent identity,
- delivery sinks and feedback,
- Sempai query and graph work under the new grammar,
- plugin ecosystem expansion behind capability contracts,
- deferred MCP and SDK generation if still out of scope for 0.1.0.

Every task should cite the design or ADR section it implements and include
observable success criteria. Unit and behavioural tests belong in the success
criteria of implementation tasks; end-to-end and combinatorial command-surface
tests are first-class tasks because this reset is primarily about interaction
contracts.

Success criteria:

- The roadmap contains an explicit OrthoConfig dependency section.
- Tasks that overlap OrthoConfig roadmap work are rewritten as dependencies,
  adapters, or deliberate divergence decisions.
- Existing completed Weaver work remains visible as historical foundation.
- Existing planned Weaver work that still matters is migrated under the new
  command grammar instead of deleted.
- Later Sempai, plugin, and graph tasks depend on the command-surface reset.
- No future task asks implementers to add new public `observe` / `act` /
  `verify` commands.
- Sempai one-liner queries are accepted as first-class symbol selectors in the
  planned command surface.
- Observe-style resource commands and act-style mutation commands have an
  explicit structured pipeline contract.
- The roadmap explains the product-forward rationale for the chosen shape,
  not only the mechanics.

### Milestone 4: Convert the UI gap analysis into an agent-native audit

Update `docs/ui-gap-analysis.md`.

Replace the historical help-only framing with a 0.1.0 audit of the ten
agent-native principles, plus human accessibility and localization
requirements. The file should record current status, target status, design
document changes, roadmap references, and acceptance tests.

Cover these audit rows:

- non-interactive by default,
- structured parseable output,
- errors that teach and enumerate,
- safe retries and explicit mutation boundaries,
- bounded responses,
- cross-CLI vocabulary consistency,
- three-layer introspection,
- async-aware execution,
- persistent identity through profiles,
- two-way I/O,
- localized human help,
- accessible human rendering,
- capability-routed provider abstraction,
- Sempai one-liner selectors,
- observe/filter/act pipeline composition.

Each row must include an `OrthoConfig dependency` column. Rows whose reusable
metadata or linting belongs to OrthoConfig should cite the task number instead
of assigning generic implementation work to Weaver. Rows whose behaviour is
Weaver-owned should say what semantic, safety, or provider orchestration work
remains local.

Success criteria:

- The file no longer treats `list-plugins` or root `--capabilities` as the
  primary target.
- It points to `context --json`, `capabilities list`, and capability-routed
  public resources instead.
- It identifies which gaps are closed by consuming OrthoConfig work and which
  gaps require Weaver implementation.
- It names prototype surfaces to remove and the acceptance tests that prevent
  drift from returning.

### Milestone 5: Update user and developer-facing docs

Update `docs/users-guide.md`.

The guide should describe the 0.1.0 target command model, including:

- default human output,
- `--json`,
- `--plain`,
- resource-first commands,
- Sempai one-liner selectors,
- position-reference selectors,
- observe-to-act pipelines through JSON selector streams,
- non-interactive execution,
- `--interactive` as explicit opt-in,
- `--dry-run`,
- `--force`,
- `--wait`,
- jobs,
- profiles,
- delivery,
- feedback,
- `context --json`,
- capability introspection,
- provider provenance,
- provider override as advanced policy only.

Remove ordinary examples that require `--provider`. Replace `act refactor`
examples with `symbols rename` and `symbols move` or `symbols extract`.

Update `docs/developers-guide.md`.

The developer guide should explain how contributors add or rename commands:

1. edit the OrthoConfig-backed command metadata or Weaver semantic adapter,
2. declare or reuse a capability,
3. provide human message IDs and examples,
4. provide JSON success and error schemas,
5. declare mutability, async, pagination, delivery, and profile behaviour,
6. update or generate docs snippets,
7. run drift and vocabulary gates.

Update `README.md`.

The README should stop claiming there are only five crates and should stop
claiming the Double-Lock safety harness is still under development if the
roadmap marks it complete. It should summarize Weaver as human-friendly and
agent-native, then point readers to the design and roadmap for the 0.1.0 reset.

Update `docs/repository-layout.md`.

Add planned components for:

- OrthoConfig-backed command metadata integration,
- Weaver semantic command-surface adapter,
- renderer generation through OrthoConfig contracts where available,
- `context --json` agent-context payload,
- skills,
- job ledger,
- profiles,
- delivery,
- feedback,
- capability introspection.

Update `docs/contents.md`.

Add ADR 007 and any new design or skills documents.

Success criteria:

- Public docs no longer contradict the target command grammar.
- Provider-specific details are presented as provenance and advanced policy,
  not the ordinary workflow.
- README, users guide, design, roadmap, and repository layout agree on what is
  implemented, planned, and target 0.1.0 behaviour.
- Repository layout distinguishes OrthoConfig-owned reusable contracts from
  Weaver-owned semantic adapters, safety integration, and provider routing.

### Milestone 6: Add drift gates to the documentation plan

The planned docs must require future CI gates that make the principles
mechanical rather than advisory. The design and roadmap should call for gates
covering:

- OrthoConfig-provided policy gates for metadata, vocabulary, renderer
  contracts, JSON stream contracts, exit-code taxonomy, bounded-list metadata,
  profile fields, delivery schemes, feedback state, skill manifests, and
  generic capability/provenance metadata.
- Weaver-owned gates for semantic capability mapping, provider routing,
  safety class declarations, transaction and idempotency behaviour,
  schema-to-router consistency, generated docs snippets, runtime
  `capabilities list`, provider provenance in JSON, and command examples that
  use the resource-first public grammar.
- A temporary-adapter gate that fails if Weaver keeps local generic
  command-contract code after the corresponding OrthoConfig dependency is
  available.

Success criteria:

- `docs/roadmap.md` has review-sized implementation tasks for the gates.
- `docs/weaver-design.md` states which invariants are enforced at schema build
  time rather than by manual review.
- The planned gates do not duplicate OrthoConfig checks except as temporary
  adapters with removal criteria.

## Implementation procedure for the document overhaul

When this plan is approved, execute it in small commits.

1. Create ADR 007 and update `docs/contents.md`.
2. Rewrite the command-surface and renderer sections of
   `docs/weaver-design.md`.
3. Rewrite the capability and plugin sections of `docs/weaver-design.md`, then
   cross-link ADR 001, ADR 004, and ADR 006.
4. Rewrite `docs/roadmap.md` so the command-surface reset precedes additional
   command growth.
5. Rewrite `docs/ui-gap-analysis.md` into the agent-native audit.
6. Update `docs/users-guide.md`, `docs/developers-guide.md`,
   `docs/repository-layout.md`, and `README.md`.
7. Run validation gates.
8. Commit only after gates pass.

Each commit should leave the documentation set internally consistent. If a
commit changes command names in one public document, it must update any other
public document that would otherwise contradict it.

## Validation

Run these commands sequentially from the repository root. Use sanitized log
names because the branch name contains a slash.

```sh
set -o pipefail && make fmt 2>&1 | tee /tmp/fmt-weaver-feat-weaver-agent-roadmap.out
set -o pipefail && make markdownlint 2>&1 | tee /tmp/markdownlint-weaver-feat-weaver-agent-roadmap.out
set -o pipefail && make nixie 2>&1 | tee /tmp/nixie-weaver-feat-weaver-agent-roadmap.out
set -o pipefail && make check-fmt 2>&1 | tee /tmp/check-fmt-weaver-feat-weaver-agent-roadmap.out
set -o pipefail && make lint 2>&1 | tee /tmp/lint-weaver-feat-weaver-agent-roadmap.out
set -o pipefail && make test 2>&1 | tee /tmp/test-weaver-feat-weaver-agent-roadmap.out
```

Expected result: every command exits 0. If any command fails, inspect the
corresponding `/tmp` log, repair the documentation or code that caused the
failure, update `Surprises & Discoveries` or `Decision Log` if the failure
changes the plan, and rerun the failed gate before continuing.

For this initial plan-only commit, the same gates should be run before commit
unless an environmental problem blocks them. If a gate cannot run because of
sandboxing, missing external tooling, or unrelated pre-existing failures,
record the blocker and log path before committing.

## Outcomes & Retrospective

Not yet executed. This section must be filled in after the documentation
overhaul is approved and completed.
