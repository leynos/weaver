# Architectural decision record (ADR) 007: Agent-native command surface

## Status

Proposed.

## Date

2026-05-11.

## Context and problem statement

Weaver is still pre-0.1.0. That gives the project room to replace prototype
public command grammar before users or agents depend on it as a stable
contract. The current documentation and code contain valuable foundations:
daemon JSON Lines (JSONL), localized prose, capability-routed plugins,
deterministic provider refusal, broker-owned plugin execution, and the
safety-harness commit path. They also contain prototype surfaces that are not
the right 0.1.0 product contract: public `observe` / `act` / `verify` domains,
root `--output auto|human|json`, operation-local `--format`, root
`--capabilities`, provider-required `act refactor`, hand-maintained command
catalogues, and inconsistent future naming.

The design goal is not to turn Weaver into an agent-only JSON appliance. Weaver
must remain friendly for humans at a terminal: localized help, readable default
output, shell completions, manpages, accessible plain output,
terminal-width-aware layouts, and explicit interactive review workflows remain
product requirements. The redesign instead makes agent usability a first-class
constraint at every layer by giving humans and agents one command truth with
two generated renderers.

The broader reusable machinery for that command truth belongs in OrthoConfig
where the OrthoConfig roadmap already assigns it. Weaver should consume those
contracts and provide only Weaver-specific semantic editing metadata,
capability routing, provider orchestration, safety integration, selectors, and
resource grammar.

## Decision drivers

- Use pre-0.1.0 freedom to remove accidental prototype grammar.
- Preserve a high-quality human terminal interface.
- Make `--json` the one canonical machine output switch.
- Keep community command-line conventions and common verbs visible.
- Keep Sempai one-liner queries as first-class symbol selectors.
- Preserve UNIX composition between observe-style resource commands, filters,
  pagers, and act-style mutation commands.
- Keep capability as the public abstraction and provider names as
  implementation provenance.
- Avoid duplicating reusable OrthoConfig command-contract work.
- Preserve existing completed Weaver roadmap work as future foundation.
- Enforce command-surface consistency mechanically rather than by review alone.

## Decision outcome

Adopt an agent-native, human-friendly 0.1.0 command-surface reset.

Weaver's public command surface will be generated or validated from one
schema-backed command contract. That contract has two first-class renderers: a
localized human renderer used by default, and a stable machine renderer
selected with `--json`. Internal daemon and plugin transports may continue to
use JSONL envelopes. Public CLI JSON mode must expose operation result schemas
on stdout for success and structured error schemas on stderr for failure.

Weaver will depend on OrthoConfig for reusable command-contract,
documentation-intermediate-representation, agent-context, policy, renderer,
profile, delivery, feedback, and execution-ledger contracts wherever the
OrthoConfig roadmap provides them. Weaver owns the application adapter:
resource paths, verbs, capability IDs, mutability classes, async classes,
selector forms, stream input support, provider selection policy, safety class,
transaction behaviour, examples, output schemas, error schemas, and skill
references.

The 0.1.0 public grammar will be resource-first. Target examples include:

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
weaver patches apply --file changes.patch --dry-run
weaver context --json
weaver capabilities list --json
weaver jobs list --json
weaver profiles list --json
weaver feedback create "the enum error did not list valid values"
```

Canonical verbs include `get`, `list`, `create`, `update`, `delete`, `apply`,
`run`, `prune`, `save`, `show`, `rename`, `move`, and `send`. The vocabulary
policy must include every verb used by target examples and planned resource
commands before vocabulary linting is enabled.

In this public grammar, `weaver symbols move` is the resource-first form of the
internal `extricate-symbol` capability: it moves a selected symbol to another
module or file while preserving meaning. It is not an alias for
`extract-method`, which extracts a selected code region into a new callable and
must remain a separate capability and roadmap slice if it graduates later.

## OrthoConfig dependencies

Weaver will not implement a second generic command-contract framework unless a
future ADR records a temporary adapter with a removal path. The known external
dependencies are:

| OrthoConfig task    | Weaver dependency                                                                                                           |
| ------------------- | --------------------------------------------------------------------------------------------------------------------------- |
| 5.2.3               | Downstream boundary: OrthoConfig owns reusable command-contract machinery; Weaver owns semantic code editing and execution. |
| 6.1                 | Recursive command and subcommand metadata.                                                                                  |
| 6.2.1 and 6.2.2     | Compact agent-context generation and schema stability.                                                                      |
| 6.2.3               | Public downstream context command naming, including `weaver context --json`.                                                |
| 6.3                 | Skill manifest metadata and validation against real command paths and flags.                                                |
| 7.1                 | Canonical vocabulary policy and linting.                                                                                    |
| 7.2.1 through 7.2.7 | Non-interactive, mutation, renderer, JSON stream, exit-code, bounded-list, and generic capability/provenance metadata.      |
| 8.1                 | Reference CLI proving `--json` and enumerating error behaviour.                                                             |
| 9.1                 | Profile contracts and redaction metadata.                                                                                   |
| 9.2                 | Delivery and feedback contracts.                                                                                            |
| 9.3                 | Execution-ledger contracts.                                                                                                 |

If an OrthoConfig dependency is not yet available when Weaver needs a vertical
slice, Weaver may add a narrow local adapter. The adapter must be documented as
temporary, covered by tests, and removed or replaced once the upstream
OrthoConfig contract is available.

## Temporary adapter removal policy

Temporary Weaver adapters are allowed only as vertical-slice scaffolding. Each
adapter must name the OrthoConfig task that replaces it, keep its scope limited
to Weaver-owned semantic metadata, and have tests that prove the temporary
shape is stable enough to migrate.

The first local adapter is `crates/weaver-cli/src/command_surface.rs`. Its
removal policy is:

| Local helper             | Replacement dependency          | Removal gate                                                                                                                                                                                                                                                                                   |
| ------------------------ | ------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `CommandSurfaceRecord`   | OrthoConfig 6.1 and 7.2.7       | Replace with OrthoConfig recursive command metadata plus generic capability/provenance metadata once those records can express Weaver resource paths, capability IDs, selector forms, output schemas, error schemas, mutability, async class, provider policy, examples, and skill references. |
| `READ_ONLY_COMMANDS`     | OrthoConfig 6.1, 6.2.1, and 6.3 | Replace with generated command metadata once the same source can drive runtime routing, `weaver context --json`, and skill validation for the pilot read command family.                                                                                                                       |
| `find_read_only_command` | OrthoConfig 6.1                 | Replace with the generated command registry lookup once structured command parsing and daemon routing can resolve `definitions get` and `references list` without a Weaver-specific array scan.                                                                                                |

Any local helper that survives after those OrthoConfig tasks are available must
record a permanent divergence in this ADR before it can remain in the
implementation.

## Boundary classification

Every live roadmap task that touches the command contract must be classified in
the OrthoConfig consumer boundary matrix at
[`docs/orthoconfig-consumer-boundary.md`](orthoconfig-consumer-boundary.md).
The machine-readable source of truth is
[`docs/orthoconfig-consumer-boundary.toml`](orthoconfig-consumer-boundary.toml).
The matrix complements the dependency table and removal policy above; it does
not replace either one.

The matrix uses these columns:

- `Roadmap task` links to the Weaver roadmap task being classified.
- `Gist` summarizes the task in one reviewable sentence.
- `State` is one of `consumes`, `wraps`, `pending`, or `divergent`.
- `Upstream OrthoConfig task` names the upstream task IDs the Weaver task
  depends on.
- `Shipped in` names the OrthoConfig release tag or pinned commit SHA that
  landed the contract for `consumes` rows.
- `Removal gate or divergence` records either the replacement condition for a
  temporary wrapper or the ADR 007 section that owns a deliberate divergence.
- `Next review by` records the next review date for unresolved upstream
  contracts.
- `Last reviewed` records the last date the row was checked.

The four boundary states have fixed evidence requirements:

- `consumes` means Weaver follows an OrthoConfig contract that has already
  shipped. The row must name the upstream task and the OrthoConfig release tag
  or pinned commit SHA in `Shipped in`.
- `wraps` means Weaver keeps a narrow temporary adapter while waiting for an
  upstream contract whose shape is already committed. The row must name the
  upstream task and the removal gate.
- `pending` means Weaver depends on an upstream contract whose shape has not
  yet been decided. The row must name the upstream task and a `Next review by`
  date. This state exists so `wraps` remains reserved for contracts whose
  upstream shape is already committed.
- `divergent` means Weaver deliberately keeps a different contract. The row
  must point to the ADR 007 section that explains the divergence.

The matrix may display the states with symbols for scanning, but the textual
state is the contract. Reviewers should treat the TOML row as the authority and
the Markdown matrix as generated documentation.

## Human renderer contract

The default renderer is for humans. It emits localized, readable output with
stable section headings, examples, and recovery guidance. It never relies on
colour alone. It disables ANSI styling when colour is unavailable, when
`NO_COLOR` is set, when `--color=never` is supplied, or when output is not a
terminal. It supports `--plain` for screen-reader-friendly and log-friendly
output.

Human rendering flags include:

```plaintext
--plain
--color auto|always|never
--no-pager
--width <columns>
--locale <tag>
```

Progress, diagnostics, warnings, and prompts go to stderr. Primary command
results go to stdout. Spinners and pagers are only allowed in terminal
contexts. Tables need headings, narrow-width fallbacks, and ASCII fallbacks for
Unicode decoration. Interactive behaviour requires `--interactive` or a
dedicated review command and must fail fast when stdin is not a terminal.

## Machine renderer contract

Every data-returning command accepts `--json`. In JSON mode, stdout contains
only the operation result on success. On failure, stderr contains a structured
error object and the process exits with a stable non-zero exit class.

JSON field names, schema versions, error codes, enum values, capability IDs,
and exit classes are protocol identifiers and are not localized. Human-readable
message fields may be localized only when the schema marks them as optional
prose. Agents and scripts must be able to decide the next action without
parsing localized text.

Root `--output auto|human|json` and operation-local `--format` are not part of
the 0.1.0 target. The canonical machine switch is `--json`.

## Selector and pipeline contract

Selectors are first-class inputs. A selector is a stable way to identify one
symbol or a collection of symbols. The initial selector forms are:

- Sempai one-liner queries with a canonical flag such as
  `--query <sempai-one-liner>`.
- Position references using `--uri` and `--position`.
- Structured selector streams from stdin when `--from-stdin` or the final
  canonical stream flag is present.

Sempai one-liners are a peer to position references, not a secondary search
feature. Any command that acts on one or more symbols must state which selector
forms it accepts and how it handles zero, one, and many matches.

Observe-style resource commands must emit selector records that act-style
mutation commands can consume directly or after ordinary UNIX filtering:

```sh
weaver symbols list --query 'fn $name(...)' --json \
  | weaver symbols rename --from-stdin --suffix _renamed

weaver symbols list --query 'fn $name(...)' --json \
  | jq 'select(.name | startswith("old_"))' \
  | weaver symbols rename --from-stdin --replace-prefix old_ --with-prefix new_

weaver symbols list --query 'class $name' --json | less
```

The pipeline contract is part of the product surface. It must remain useful to
agents, scripts, and humans composing ordinary shell tools.

## Capability, perceptor, actuator, and provider

A capability is a stable semantic contract, such as `definition.get`,
`references.list`, `diagnostics.list`, `symbol.rename`, `symbol.move`, or
`patch.apply`.

A perceptor is a read-only provider that observes the codebase and returns
facts, diagnostics, cards, graph slices, matches, selector records, or
provenance.

An actuator is a mutation-planning provider that returns edits, diffs, patches,
plans, or workspace-change proposals. It never commits directly. Every actuator
result passes through Weaver-owned transaction, safety, Double-Lock,
idempotency, rollback, and atomic-write machinery.

A provider is an implementation of one or more capabilities, such as Rope,
rust-analyzer, Tree-sitter, an LSP server, Sempai, or a Weaver built-in.

The ordinary public command path is capability-first:

```sh
weaver symbols rename --uri file:///src/lib.rs --position 42:9 --new-name run
```

It is not provider-first:

```sh
weaver act refactor --provider rust-analyzer --refactoring rename ...
```

Provider selection is deterministic:

1. explicit CLI override,
2. selected profile policy,
3. environment or configuration policy,
4. workspace policy,
5. provider priority from manifests,
6. deterministic tie-breaker,
7. structured refusal if no provider qualifies.

Provider names remain available in diagnostics, provenance, `--verbose` human
output, JSON output, `context --json`, and expert policy overrides. They are
not the normal user workflow.

## Prototype surfaces superseded for 0.1.0

The following prototype surfaces are superseded by this ADR for the 0.1.0
target:

- public `observe` / `act` / `verify` domain grammar,
- root `--output auto|human|json`,
- operation-local `--format`,
- root `--capabilities`,
- provider-required `act refactor`,
- provider-specific public commands such as `weaver rope rename`,
- hand-maintained command catalogues,
- implicit prompts,
- unbounded list output.

Existing implementations remain useful evidence and foundation. The roadmap
must preserve completed work and migrate still-relevant planned work under the
new grammar. Any removed roadmap item must be marked superseded with a short
rationale.

## Relationship to previous ADRs

ADR 001 already establishes the capability-first plugin direction: user intent
is represented by stable capability IDs, provider choice is resolved
internally, and final edits flow through the safety harness. This ADR
generalizes that model from `act extricate` into the whole 0.1.0 public surface.

ADR 004 already requires deterministic routing and structured refusal
diagnostics. This ADR makes those properties part of the generated public
command contract and the machine renderer.

ADR 006 already keeps plugin execution one-shot and broker-owned. This ADR
preserves that boundary while separating internal JSONL plugin transport from
public `--json` command output.

## Consequences

Documentation and implementation work must be sequenced so the command-surface
reset lands before further Sempai, plugin, graph, or workflow command growth.
Future commands must declare their resource path, canonical verb, capability
ID, selector forms, mutability class, async class, pagination behaviour,
renderer contracts, error schemas, provenance, and drift gates.

The design raises the consistency bar. Command names, help, manpages, shell
completions, docs snippets, `context --json`, skill manifests, router metadata,
and tests must be generated from or validated against the same contract. Manual
review is no longer sufficient as the primary consistency mechanism.

The human interface remains first-class. The reset removes accidental prototype
grammar and drift; it does not remove friendly terminal behaviour.
