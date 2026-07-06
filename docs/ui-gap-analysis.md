# Weaver command-interface gap analysis

This document audits the gap between Weaver's current pre-0.1.0 command-line
interface and the target human-friendly, agent-native command contract defined
by [ADR 007](adr-007-agent-native-command-surface.md), the
[design document](weaver-design.md), and the [roadmap](roadmap.md).

The previous version of this document focused on discoverability defects in the
prototype `observe` / `act` / `verify` grammar. Those findings remain useful
evidence of why the reset is necessary, but the future target is no longer to
polish that prototype grammar. The target is one generated command contract
with two first-class renderers:

- a localized, accessible, human-friendly default renderer; and
- a stable `--json` renderer for agents, scripts, and UNIX pipelines.

The audit below therefore treats legacy command names, root `--output`,
operation-local `--format`, root `--capabilities`, provider-first mutation
commands, and hand-maintained catalogues as current-state evidence rather than
future contract details.

## Current evidence

The prototype interface still exposes several concrete gaps that motivated the
reset:

- `crates/weaver-cli/src/cli.rs` still models the public command as positional
  `DOMAIN`, `OPERATION`, and trailing `ARG` tokens. That keeps operation-level
  help and validation out of the schema that clap can render.
- The top-level command still has `disable_help_subcommand = true`, so
  `weaver help` is not a normal help entrypoint.
- The top-level machine-output control is `--output auto|human|json`, while
  the target contract requires one canonical `--json` switch.
- Some operation documentation still references operation-local formatting
  flags or provider-first workflows, both of which are superseded by ADR 007.
- Unknown-domain, unknown-operation, missing-argument, and missing-provider
  paths have improved in recent roadmap work, but the current surface does not
  yet provide universal enum enumeration, stable exit classes, or structured
  failure JSON for every command.
- Plugin and capability information exists in the implementation and ADRs, but
  ordinary users still encounter provider concepts too early in some current
  documentation and workflows.

These are not separate defects to fix one by one under the old grammar. They
are acceptance evidence for the command-surface reset in roadmap phase 13.

## Principle gap matrix

### Non-interactive by default

Current status: most implemented commands are scriptable, and the safety
harness avoids implicit commits.

Shortfall: future interactive review work could accidentally add prompts, and
destructive and mutating semantics are not yet declared in one command schema.

Target contract: no command prompts unless `--interactive` or a review command
is used; non-TTY paths fail fast; destructive operations require `--force`;
mutating commands declare `--dry-run` and idempotency policy.

Roadmap owner: `roadmap.md` 13.2, 16.1, 16.2, and 19.2.

### Structured, parseable output

Current status: Weaver uses JSONL internally and exposes root `--output json`.

Shortfall: `--output json` is not the target community convention; JSON mode is
not universal, operation-local `--format` exists in older docs, and
stdout/stderr/error schema rules are not yet universal.

Target contract: every data-returning command accepts `--json`; success JSON
goes to stdout; structured error JSON goes to stderr; protocol identifiers are
non-localized.

Roadmap owner: `roadmap.md` 13.2.2.

### Errors that teach and enumerate

Current status: ADR 004 and prior roadmap work define stable routing refusals
and better alternatives for some errors.

Shortfall: enumeration is not universal across enums, registries, providers,
profiles, jobs, delivery schemes, selector forms, and capability IDs.

Target contract: every enum-shaped rejection includes the invalid value, valid
values, source registry, stable error code, exit class, and a working next
command.

Roadmap owner: `roadmap.md` 13.2.3 and 18.1.3.

### Safe retries and mutation boundaries

Current status: Double-Lock, syntactic verification, atomic edits, and
capability-routed mutation work are substantial foundations.

Shortfall: mutations do not yet share one target contract for idempotency keys,
transaction IDs, retry matching, dry-run coverage, and selector-stream
provenance.

Target contract: actuator output is always planned, verified, committed
atomically, and reported with transaction metadata; retries reuse idempotency
keys or safe natural keys.

Roadmap owner: `roadmap.md` 16.1 and 16.2.

### Bounded responses

Current status: graph-slice work already has explicit budgets such as card and
edge limits.

Shortfall: bounded output is not yet a command-surface invariant for every
collection command or future tool description.

Target contract: lists expose `--limit`, cursor or continuation state,
truncation markers, budget metadata, and narrowing hints.

Roadmap owner: `roadmap.md` 13.2.3, 14.1, 14.2, and 17.1.

### Cross-CLI vocabulary consistency

Current status: current docs and code still mix prototype domains, provider
terms, and older flags.

Shortfall: canonical verbs and banned names are not yet enforced mechanically.

Target contract: resource-first commands use canonical verbs including `get`,
`list`, `create`, `update`, `delete`, `apply`, `run`, `prune`, `save`, `show`,
`rename`, `move`, and `send`; CI rejects off-policy names.

Roadmap owner: `roadmap.md` 13.1 and 13.3.4.

### Three-layer introspection

Current status: help and generated manpage work exist, and prior roadmap
entries improved help.

Shortfall: the target `weaver context --json`, capability availability command,
and workflow skills are missing.

Target contract: human help, structured `context --json`,
`capabilities list --json`, and `skill-path` all derive from or validate
against the same command contract.

Roadmap owner: `roadmap.md` 13.3.

### Async-aware execution

Current status: ADR 006 defines broker-owned plugin execution, but it
deliberately keeps one-shot JSONL execution narrow.

Shortfall: no public `--wait` contract or durable jobs ledger exists for
recoverable long-running workflows.

Target contract: async-submit commands support `--wait`; `weaver jobs list`,
`weaver jobs get`, and `weaver jobs prune` expose a durable ledger and retry
recovery.

Roadmap owner: `roadmap.md` 19.1.2 and 19.1.3.

### Persistent identity through profiles

Current status: Weaver has layered configuration, including config files,
environment, and flags.

Shortfall: named profiles are not yet a first-class command-surface state
primitive or exposed through context metadata.

Target contract: `weaver profiles save`, `weaver profiles list`,
`weaver profiles show`, `weaver profiles delete`, and root `--profile` provide
named agent and human identities with redaction and explicit precedence.

Roadmap owner: `roadmap.md` 19.1.1.

### Two-way I/O

Current status: Weaver has stdout/stderr discipline and internal JSONL
transport.

Shortfall: artefact delivery sinks and agent feedback are not yet first-class,
discoverable, or schema-backed.

Target contract: `--deliver stdout`, `--deliver file:<path>`, and
`--deliver webhook:<url>` handle artefact routing; `weaver feedback create`,
`weaver feedback list`, and `weaver feedback send` record local and optional
upstream friction reports.

Roadmap owner: `roadmap.md` 19.1.3 and 19.1.4.

## Human-interface gaps

The reset must not turn Weaver into an agent-only appliance. The current
prototype still lacks several human-facing guarantees that the target command
contract must supply:

- Default output must remain readable and localized. Humans should not need
  `jq` for ordinary success or failure paths.
- Help must be available through `weaver --help`, `weaver help`, command-level
  help, generated manpages, shell completions, and copy-pasteable examples.
- `--plain`, `--color auto|always|never`, `--no-pager`, `--width`, and
  `--locale` must be stable human-renderer controls rather than alternate
  protocol shapes.
- Human output must not rely on colour alone. Tables need headings, narrow
  terminals need labelled-block fallbacks, Unicode decoration needs ASCII
  fallbacks, and progress belongs on stderr only when stderr is a terminal.
- Interactive review remains valuable, but it must be explicit through
  `--interactive` or a dedicated review command and must fail fast without a
  terminal.

These gaps are primarily owned by `roadmap.md` 13.2.1 and 19.2.2. They depend
on OrthoConfig behavioural metadata, but the human layouts and recovery text
are Weaver-owned because they must explain semantic code work.

## Agent-interface gaps

The current interface is agent-friendly in important ways, especially its JSONL
foundations, daemon model, safety harness, and capability ADRs. It is not yet
agent-native by construction. The missing pieces are:

- one canonical `--json` switch instead of root `--output json`;
- stable success and failure schemas for every public command;
- stable exit-code classes;
- bounded collection defaults and continuation metadata;
- structured selector records that can flow from observe-style commands into
  act-style commands;
- Sempai one-liner selectors as peers of `--uri` plus `--position`;
- `weaver context --json` for whole-command introspection;
- `weaver capabilities list --json` for runtime capability availability;
- workflow skills discoverable through `weaver skill-path`;
- recoverable job state for long-running workflows;
- named profiles for repeated agent identity;
- delivery sinks for artefacts; and
- feedback commands for reporting friction.

These gaps are distributed across roadmap phases 13 through 19. They are not a
request to duplicate OrthoConfig. The reusable metadata, naming, renderer,
profile, delivery, feedback, and ledger contracts remain explicit dependencies
on OrthoConfig; Weaver owns the semantic editing and safety integration.

## Capability and provider gaps

Weaver's existing ADRs already point in the correct direction:

- ADR 001 says user intent is represented by stable capability IDs and provider
  selection is internal.
- ADR 004 requires deterministic routing and stable refusal diagnostics.
- ADR 006 keeps plugin execution broker-owned and prevents plugins from owning
  final commit behaviour.

The gap is that the ordinary public surface and some current documentation
still leak provider-first thinking. The target command surface must present
capabilities as the public abstraction:

```sh
weaver symbols rename --uri file:///src/lib.rs --position 42:9 --new-name run
```

Provider-specific commands such as `weaver rope rename` or workflows that
require `--provider` by default are out of contract. Provider IDs remain useful
in JSON provenance, verbose diagnostics, policy, profiles, and expert
overrides. They are not the everyday grammar.

Roadmap phase 18 owns this migration behind the command-surface adapter defined
in phase 13.

## Selector and pipeline gaps

Sempai one-liner queries must be first-class selectors, not a search-only side
feature. Position references and selector streams are peer selector forms. The
target interactions include:

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

The optional filter in an observe-to-act pipeline may be any ordinary UNIX
filter that preserves compatible selector records. Act commands that consume
selectors must state zero-match, one-match, and many-match behaviour. Selector
records must preserve enough provenance for safe mutation and auditability.

Roadmap phases 15 and 16 own the core selector and mutation slices that make
these examples real. Phase 14 supplies the read-command outputs, and phase 17
adds richer graph context once the core loop is proven.

## Historical prototype findings

The following older findings are retained as historical evidence. They should
not be implemented literally if doing so would preserve the superseded public
grammar:

- Bare `weaver` produced only `the command domain must be provided`.
- Top-level help listed `DOMAIN`, `OPERATION`, and `ARG` placeholders rather
  than a resource command tree.
- Domains and operations were not enumerated in generated help.
- `weaver help` was disabled by clap configuration.
- Configuration flags were stripped before clap and therefore invisible in
  help.
- Missing operations, unknown domains, and unknown operations did not
  universally enumerate alternatives.
- `act refactor` errors exposed provider-first workflow details.
- Plugin listing was absent.
- Root `--capabilities` reported overrides rather than the full runtime
  capability picture.

ADR 007 and `roadmap.md` supersede the specific remedies that would merely
patch those behaviours in place. The durable remedies are generated command
metadata, resource-first commands, human and machine renderers, structured
introspection, capability-routed providers, and drift gates.

## Acceptance checklist

The gap analysis is resolved when the following checks have concrete evidence:

- The command-surface adapter can generate or validate public commands, help,
  docs snippets, router metadata, schemas, and tests from one source.
- `weaver --help`, `weaver help`, generated manpages, and completions expose
  the same command tree.
- Every data-returning command accepts `--json`.
- JSON success and JSON failure outputs are parseable and non-localized where
  protocol stability requires it.
- Every enum-shaped validation error enumerates valid values.
- Every collection command has bounded defaults and narrowing hints.
- Every mutating command declares selector forms, dry-run support, idempotency,
  safety policy, and transaction metadata.
- `weaver context --json`, `weaver capabilities list --json`, and
  `weaver skill-path` exist and are validated against the command contract.
- Profiles, jobs, delivery, and feedback are discoverable through context
  metadata and have redaction rules where needed.
- Provider IDs appear in provenance and expert policy, not as required normal
  workflow syntax.
- Roadmap tasks that depend on reusable command-contract work cite the
  relevant OrthoConfig task instead of recreating generic infrastructure in
  Weaver.
