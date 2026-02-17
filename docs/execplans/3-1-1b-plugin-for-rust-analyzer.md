# Implement the rust-analyzer actuator plugin

This Execution Plan (ExecPlan) is a living document. The sections
`Constraints`, `Tolerances`, `Risks`, `Progress`, `Surprises & Discoveries`,
`Decision Log`, and `Outcomes & Retrospective` must be kept up to date as work
proceeds.

Status: COMPLETE (2026-02-17)

This document must be maintained in accordance with `AGENTS.md` at the
repository root.

`PLANS.md` is not present in this repository, so no additional plan governance
applies beyond `AGENTS.md` and this ExecPlan.

## Purpose / big picture

Deliver the second specialist actuator plugin by integrating `rust-analyzer`
for Rust refactoring. After this work, the following command executes a
sandboxed rust-analyzer-backed plugin:

```sh
weaver act refactor --provider rust-analyzer --refactoring rename --file src/main.rs offset=42 new_name=better_name
```

The command receives a unified diff and applies it through the existing
Double-Lock safety harness.

Observable success:

- `act refactor` with `--provider rust-analyzer` succeeds for supported
  operations and modifies files only after lock verification.
- Unsupported operations, missing arguments, plugin protocol errors, and lock
  failures return structured failures and leave files unchanged.
- Unit, behavioural (`rstest-bdd` v0.5.0), and end-to-end tests cover happy,
  unhappy, and edge cases.
- `docs/weaver-design.md`, `docs/users-guide.md`, and `docs/roadmap.md`
  reflect shipped behaviour.
- `make check-fmt`, `make lint`, and `make test` succeed.

## Constraints

- Keep all execution synchronous (no async runtime introduction).
- Keep plugin execution sandboxed via `weaver-sandbox` and
  `weaver-plugins::process::SandboxExecutor`.
- Do not bypass the Double-Lock harness for plugin-produced edits.
- Continue using `rstest-bdd` v0.5.0 and write behavioural tests with mutable
  world fixtures (`&mut World`).
- Add end-to-end command ergonomics tests in `crates/weaver-e2e/` using
  `assert_cmd` and `insta` snapshots.
- Keep files under 400 lines by splitting modules where needed.
- Use DI boundaries so tests do not require a system rust-analyzer install.
- Run quality gates with `tee` and `set -o pipefail` before finishing.

## Tolerances (exception triggers)

- Scope: if delivery requires touching more than 26 files or ~2400 net lines,
  stop and escalate.
- Interface: if public protocol schema in
  `crates/weaver-plugins/src/protocol/mod.rs` must break compatibility, stop
  and escalate.
- Dependencies: if adding new external crates beyond existing workspace
  dependencies becomes necessary, stop and escalate with justification.
- Iterations: if the same failing test loop repeats 5 times without progress,
  stop and escalate.

## Risks

- Risk: rust-analyzer startup and indexing can exceed default plugin timeout.
  Severity: high. Likelihood: medium. Mitigation: register a 60-second timeout
  for the rust-analyzer actuator.
- Risk: LSP `WorkspaceEdit` response shape may include unsupported resource
  operations. Severity: medium. Likelihood: medium. Mitigation: validate and
  surface clear adapter errors.
- Risk: LSP position encoding and UTF-8 offsets can mismatch. Severity: high.
  Likelihood: medium. Mitigation: convert offsets using UTF-16-aware helpers.

## Progress

- [x] (2026-02-17 00:00Z) Added `crates/weaver-plugin-rust-analyzer/` crate
      with request dispatch, adapter trait boundary, and binary entrypoint.
- [x] (2026-02-17 00:20Z) Implemented production adapter using rust-analyzer
      LSP JSON-RPC flow with workspace-edit application.
- [x] (2026-02-17 00:35Z) Wired runtime registration in `weaverd` for the
      rust-analyzer provider and added path resolution tests.
- [x] (2026-02-17 00:45Z) Added unit and behavioural tests for plugin dispatch,
      plus e2e CLI snapshots for rust-analyzer command ergonomics.
- [x] (2026-02-17 01:00Z) Updated design docs, user guide, and roadmap status.
- [x] (2026-02-17 01:15Z) Ran full quality gates successfully.

## Surprises & Discoveries

- Observation: project memory MCP resources were unavailable in this session
  (`list_mcp_resources` returned no servers/resources). Work relied on local
  docs and source inspection.
- Observation: `crates/weaverd/src/dispatch/act/refactor/mod.rs` was near the
  400-line cap. Path-resolution logic was extracted into `plugin_paths.rs`
  before adding the new provider.

## Decision Log

- Decision: mirror `weaver-plugin-rope` crate shape for rust-analyzer.
  Rationale: consistent plugin ergonomics and lower maintenance overhead.
  Date/Author: 2026-02-17 / Codex
- Decision: implement rust-analyzer integration through short-lived LSP
  JSON-RPC 2.0 stdio exchange rather than custom analysis logic. Rationale:
  semantic correctness and alignment with existing LSP strategy. Date/Author:
  2026-02-17 / Codex
- Decision: keep all unit/BDD tests mock-based at adapter boundary.
  Rationale: deterministic test suite without requiring rust-analyzer on host.
  Date/Author: 2026-02-17 / Codex

## Outcomes & Retrospective

- Added `weaver-plugin-rust-analyzer` as the second actuator plugin with
  `rename` support (`offset`, `new_name`) and structured failure diagnostics.
- Registered rust-analyzer provider in `weaverd` with language `rust`, default
  executable `/usr/bin/weaver-plugin-rust-analyzer`, override via
  `WEAVER_RUST_ANALYZER_PLUGIN_PATH`, and timeout `60s`.
- Added plugin unit tests, `rstest-bdd` behavioural tests, and e2e CLI
  snapshot coverage for actuator isolation and observe→jq→act pipelines.
- Updated `docs/weaver-design.md`, `docs/users-guide.md`, and `docs/roadmap.md`
  to capture behaviour and design decisions.
- Full quality gates passed before completion.
