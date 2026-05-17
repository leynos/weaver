# Align archive `3.2.2` daemon-start help with live `13.3.3`

This ExecPlan (execution plan) is a living document. The sections
`Constraints`, `Tolerances`, `Risks`, `Progress`, `Surprises & Discoveries`,
`Decision Log`, and `Outcomes & Retrospective` must be kept up to date as work
proceeds.

Status: DRAFT; ROADMAP-FIT ANALYSIS COMPLETE; DO NOT IMPLEMENT AS A STANDALONE
`3.2.2` PATCH.

This document must be maintained in accordance with `AGENTS.md` at the
repository root. Approval from the user is required before implementation
begins.

## Purpose / big picture

This ExecPlan preserves the useful product intent from archive roadmap item
`3.2.2`, "Extend `daemon start` help with config and environment guidance",
while renumbering the active work against the current live roadmap framework.
The archive in `docs/archive/prototype-roadmap.md` keeps `3.2.2` as historical
provenance only. The live roadmap in `docs/roadmap.md` moves help, command
metadata, generated references, manpages, shell completions, skills, and drift
prevention into phase `13`, especially task `13.3.3`.

The original standalone plan would add hand-written clap `long_about` text to
the prototype `weaver daemon start --help` path so it mentions `WEAVERD_BIN`
and `WEAVER_FOREGROUND`. That remains a real discoverability gap, but it is no
longer the best next build unit under the live roadmap. The current framework
requires generated help from one command metadata source, not another local
help-text patch that will later be replaced.

The recommended outcome is therefore:

1. Treat archive `3.2.2` as provenance for the acceptance requirement that
   lifecycle command help must expose operational environment variables.
2. Fold that requirement into live roadmap task `13.3.3`, after `13.3.1`
   establishes command context metadata for the pilot command family.
3. Implement daemon lifecycle help only through the generated command-help
   path, or as an explicitly temporary adapter justified under `13.1.3`.

Observable success for the eventual live implementation:

```plaintext
$ weaver help daemon start
...
Environment variables:
  WEAVERD_BIN
  WEAVER_FOREGROUND
...
```

The exact command spelling may change if the command-surface reset rehomes
daemon lifecycle operations. The invariant is that the generated help and
reference surfaces expose both environment variables from the same metadata
source as command help, manpage input, shell completions, and skill output.

## Current numbering

The current roadmap numbering is:

- Archive provenance: `docs/archive/prototype-roadmap.md` item `3.2.2`.
- Archive step disposition: `3.2` is marked "Migrated" in the archive
  relevance matrix, with help, command metadata, and manpage work moved to live
  roadmap phase `13.2` and `13.3`.
- Live direct owner: `docs/roadmap.md` task `13.3.3`, "Implement
  `weaver help`, command help, manpage input, shell completions, and
  `weaver skill-path` from the same metadata."
- Live supporting tasks: `13.1.3` defines temporary-adapter removal policy,
  `13.2.1` proves the localized human renderer, `13.3.1` establishes
  `weaver context --json`, and `13.3.4` adds command-surface drift gates.

This ExecPlan is therefore numbered as `13.3.3` with archive `3.2.2`
provenance. It must not ask implementers to mark archive `3.2.2` complete in
`docs/roadmap.md`; that item now lives only in the archive. Completion belongs
to the relevant live phase-`13` task.

## Roadmap-fit analysis

The old standalone plan is not worth building as written in the new roadmap
framework.

It solves a valid user problem: an operator running
`weaver daemon start --help` cannot discover `WEAVERD_BIN`, which overrides the
spawned daemon binary, or `WEAVER_FOREGROUND`, which keeps the daemon attached
to the controlling terminal. That information matters during installation
failures, debugging, and CI jobs.

The problem is still real, but the proposed mechanism is misaligned with the
live roadmap. A hand-written `long_about` on `DaemonAction::Start` would add
another source of command help just as phase `13` is trying to converge help,
localization, manpage generation, completions, skills, context JSON, and drift
tests on one command contract. It would be useful only as a short-lived patch
to the prototype grammar.

Building the original patch now also creates avoidable follow-up work. The
implementation would need tests and snapshots for a legacy help path, then a
later `13.3.3` implementation would need to move the same content into the
generated metadata model and update the tests again. That churn does not prove
the new command framework; it delays it.

The requirement is worth keeping. The old implementation plan is not. The right
live-roadmap version is to add lifecycle environment-variable metadata to the
generated command-help model once `13.3.3` is underway, with tests that prove
all generated surfaces agree.

A tactical exception is acceptable only if there is an immediate release or
support need to document `WEAVERD_BIN` and `WEAVER_FOREGROUND` before `13.3.3`.
In that case, the patch must be explicitly labelled as a temporary adapter
under `13.1.3`, must name the live task that will remove it, and must not
expand into broader prototype help polishing.

## Constraints

- Do not implement this as a standalone archive `3.2.2` patch unless the user
  explicitly approves a temporary-adapter exception.
- Do not mark archive `3.2.2` complete in `docs/roadmap.md`; the active
  roadmap owner is live task `13.3.3`.
- Preserve the archive item as provenance in
  `docs/archive/prototype-roadmap.md`.
  That archive intentionally keeps numbers `1` through `11`.
- Any eventual implementation must use one generated command metadata source
  for help, manpage input, shell completions, skills, and drift checks.
- Runtime semantics for `WEAVERD_BIN` and `WEAVER_FOREGROUND` must not change.
  The work is discoverability only unless a later live roadmap task explicitly
  changes daemon lifecycle behaviour.
- Documentation must use en-GB-oxendict spelling and grammar.

## Tolerances (exception triggers)

- Scope: if this work grows beyond renumbering and roadmap-fit analysis, stop
  and ask for approval before changing Rust code.
- Roadmap mismatch: if live roadmap task `13.3.3` changes ownership or is
  split before implementation begins, update this ExecPlan before writing code.
- Temporary adapter: if an immediate patch to `DaemonAction::Start` is
  requested, document the removal path in the decision log before
  implementation.
- Generated surfaces: if lifecycle help cannot be represented in the generated
  command metadata without a custom path, stop and escalate with the missing
  metadata fields.
- Validation: documentation edits require `make fmt`, `make markdownlint`, and
  `make nixie` before commit.

## Risks

- Risk: leaving the operator-facing gap open until `13.3.3` may frustrate users
  debugging daemon startup. Severity: medium. Likelihood: medium. Mitigation:
  allow a narrowly scoped temporary patch only when a release or support need
  is explicit.
- Risk: implementing the old patch now creates duplicate help sources and
  snapshot churn. Severity: medium. Likelihood: high. Mitigation: do not build
  the old patch as written; fold the requirement into generated command
  metadata.
- Risk: phase `13.3.3` initially focuses on the pilot command family and may
  not include daemon lifecycle commands. Severity: medium. Likelihood: medium.
  Mitigation: carry archive `3.2.2` as an explicit acceptance note so lifecycle
  commands are either included deliberately or deferred deliberately.
- Risk: the current user's guide uses examples such as `WEAVER_FOREGROUND=1`,
  while the daemon checks whether the variable is present. Severity: low.
  Likelihood: medium. Mitigation: generated help should say "set, for example
  `WEAVER_FOREGROUND=1`" unless runtime semantics are changed.

## Progress

- [x] Confirmed `docs/archive/prototype-roadmap.md` preserves archive item
      `3.2.2` as historical provenance.
- [x] Confirmed the archive relevance matrix migrates step `3.2` into live
      roadmap phases `13.2` and `13.3`.
- [x] Confirmed `docs/roadmap.md` no longer carries a live standalone `3.2.2`
      task and assigns generated help surfaces to `13.3.3`.
- [x] Renumbered this ExecPlan as live `13.3.3` with archive `3.2.2`
      provenance.
- [x] Added roadmap-fit analysis and a recommendation not to implement the old
      standalone patch as written.
- [ ] If implementation is later approved, rewrite the build plan around
      generated command metadata rather than clap-only `long_about` text.
- [ ] If a temporary adapter is later approved, add a removal decision tied to
      `13.3.3` or `13.3.4` before changing Rust code.

## Surprises & discoveries

- The live roadmap deliberately starts at phase `12`; archive item `3.2.2`
  remains uniquely addressable only in `docs/archive/prototype-roadmap.md`.
- The current live roadmap migrates the whole `3.2` help/manpage family to
  phase `13`, but does not preserve `daemon start` help as a standalone live
  task.
- The old ExecPlan's Stage D instructions to tick `docs/roadmap.md` item
  `3.2.2` are stale because that item no longer exists in the live roadmap.

## Decision log

- Decision: renumber the active ExecPlan to live roadmap task `13.3.3` while
  retaining archive `3.2.2` in the filename and text. Rationale:
  `docs/archive/prototype-roadmap.md` preserves `3.2.2` as provenance, while
  `docs/roadmap.md` assigns generated help and reference surfaces to `13.3.3`.
  Date/Author: 2026-05-17, planning agent.

- Decision: do not recommend building the original clap-only
  `DaemonAction::Start` `long_about` patch as the next implementation.
  Rationale: the live roadmap is validation-led and metadata-first. A
  hand-authored prototype help patch would duplicate the command contract that
  `13.3.3` is meant to generate. Date/Author: 2026-05-17, planning agent.

- Decision: keep the requirement that `WEAVERD_BIN` and `WEAVER_FOREGROUND`
  appear in lifecycle command help. Rationale: the operator need remains valid
  even though the implementation mechanism changes. The requirement should
  become acceptance evidence for the generated help framework. Date/Author:
  2026-05-17, planning agent.

## Outcomes & retrospective

The roadmap-fit analysis concludes that the old standalone plan should not be
implemented as written. Its user-facing requirement should survive as an
acceptance requirement inside live `13.3.3`, or as a narrowly scoped temporary
adapter only if an immediate release need is explicitly approved.

## Context and orientation

The original plan targeted the prototype CLI path. It proposed adding a
`#[command(long_about = ...)]` attribute to `DaemonAction::Start` in
`crates/weaver-cli/src/cli.rs`, then updating unit tests, behavioural tests,
Insta snapshots, `docs/users-guide.md`, `docs/developers-guide.md`, and the
roadmap.

That implementation context remains useful evidence but is no longer the
preferred build path. The live command framework is documented in:

- `docs/roadmap.md` phase `13`, especially `13.1.3`, `13.2.1`, `13.3.1`,
  `13.3.3`, and `13.3.4`;
- `docs/archive/prototype-roadmap.md`, whose relevance matrix marks archive
  step `3.2` as migrated;
- `docs/adr-007-agent-native-command-surface.md`, which defines the reset
  toward one command contract with human and JSON renderers; and
- `docs/ui-gap-analysis.md`, which treats prototype gaps as evidence for the
  reset rather than as independent old-grammar patches.

The existing environment-variable semantics are:

- `WEAVERD_BIN` overrides the path to the `weaverd` binary that the CLI
  spawns, falling back to `weaverd` on `PATH`.
- `WEAVER_FOREGROUND` is read by the daemon to keep it in the foreground when
  the variable is present. Existing documentation commonly shows
  `WEAVER_FOREGROUND=1` as the operator-facing example.

## Revised plan of work

Do not begin implementation from this document until the user approves either
the generated-metadata path or a temporary-adapter exception.

### Stage A -- Generated-metadata path

When live task `13.3.3` begins, add lifecycle environment-variable metadata to
the same command contract that feeds generated help, manpage input, shell
completions, skills, and drift fixtures.

Acceptance criteria:

- Generated help for the daemon lifecycle start operation includes
  `WEAVERD_BIN`, `WEAVER_FOREGROUND`, and at least one startup example.
- Generated reference artefacts and drift fixtures consume the same metadata.
- No clap-only hand-authored help path is introduced for this requirement.

### Stage B -- Temporary-adapter exception

Use this path only after explicit approval. Add the smallest possible
`DaemonAction::Start` help patch, label it as temporary, and document the
removal owner as `13.3.3` or `13.3.4`.

Acceptance criteria:

- `weaver daemon start --help` includes both environment-variable names and a
  startup example.
- Tests prove help rendering does not require daemon startup or configuration
  loading.
- Documentation names the patch as temporary and points to the generated
  command-help task that will replace it.

## Validation and acceptance

For this renumbering and analysis change, run:

```sh
make fmt | tee /tmp/fmt-weaver-13-3-3-archive-3-2-2-daemon-start-help-guidance.out
make markdownlint | tee /tmp/markdownlint-weaver-13-3-3-archive-3-2-2-daemon-start-help-guidance.out
make nixie | tee /tmp/nixie-weaver-13-3-3-archive-3-2-2-daemon-start-help-guidance.out
```

For any later Rust implementation, also run the full code gates required by
`AGENTS.md`:

```sh
make check-fmt | tee /tmp/check-fmt-weaver-13-3-3-archive-3-2-2-daemon-start-help-guidance.out
make typecheck | tee /tmp/typecheck-weaver-13-3-3-archive-3-2-2-daemon-start-help-guidance.out
make test | tee /tmp/test-weaver-13-3-3-archive-3-2-2-daemon-start-help-guidance.out
make lint | tee /tmp/lint-weaver-13-3-3-archive-3-2-2-daemon-start-help-guidance.out
```

The renumbering is complete when this file is the only ExecPlan for the
daemon-start help requirement, the old `3.2.2` filename is gone, and the
document clearly states the live roadmap owner and the recommendation not to
build the standalone old patch.

## Idempotence and recovery

The documentation change is idempotent. If the rename causes confusion, restore
the deleted file from Git and reapply the content under the old name, but keep
the live numbering and roadmap-fit analysis in the text. Do not recreate a live
`3.2.2` implementation plan without updating the decision log.

## Interfaces and dependencies

This renumbering changes documentation only. It does not change Rust code,
public APIs, command-line behaviour, Cargo dependencies, generated artefacts,
or roadmap checkboxes.

Future implementation depends on the phase-`13` command metadata work. Until
that exists, this document is a roadmap alignment and feasibility analysis, not
an approved build plan.
