# Implement the first actuator plugin for `rope`

This Execution Plan (ExecPlan) is a living document. The sections
`Constraints`, `Tolerances`, `Risks`, `Progress`, `Surprises & Discoveries`,
`Decision Log`, and `Outcomes & Retrospective` must be kept up to date as work
proceeds.

Status: COMPLETE (2026-02-13)

This document must be maintained in accordance with `AGENTS.md` at the
repository root.

`PLANS.md` is not present in this repository, so no additional plan governance
applies beyond `AGENTS.md` and this ExecPlan.

## Purpose / big picture

Deliver the first real specialist actuator plugin by integrating `rope` for
Python refactoring. After this work, `weaver act refactor --provider rope`
executes a sandboxed rope-backed plugin, receives a unified diff, and applies
it through the existing Double-Lock safety harness so no filesystem changes are
committed on syntactic or semantic failure.

Observable success:

- `act refactor` with `--provider rope` succeeds for supported operations and
  modifies files only after lock verification.
- Unsupported operations, missing arguments, timeout, plugin protocol errors,
  and lock failures return structured failures and leave files unchanged.
- Unit, behavioural, and end-to-end tests cover happy paths, unhappy paths,
  and edge cases.
- `docs/weaver-design.md`, `docs/users-guide.md`, and `docs/roadmap.md`
  reflect the shipped behaviour.
- `make check-fmt`, `make lint`, and `make test` succeed.

## Constraints

- Keep all execution synchronous (no async runtime introduction).
- Keep plugin execution sandboxed via `weaver-sandbox` and
  `weaver-plugins::process::SandboxExecutor`.
- Do not bypass the Double-Lock harness for plugin-produced edits.
- Continue using `rstest-bdd` v0.5.0 and write new behavioural tests with
  mutable world fixtures (`&mut World`) for new scenarios.
- Add end-to-end (e2e) command ergonomics tests in `crates/weaver-e2e/` using
  `assert_cmd` and `insta` snapshots for CLI usage flows.
- Keep module-level `//!` comments and rustdoc for public items; follow
  guidance in `docs/rust-doctest-dry-guide.md`.
- Keep files under 400 lines by splitting modules where needed.
- Prefer dependency injection and trait boundaries for non-deterministic
  concerns (process spawning, rope adapter behaviour), per
  `docs/reliable-testing-in-rust-via-dependency-injection.md`.
- Run quality gates with `tee` and `set -o pipefail` before finishing.
- Update user-facing docs and roadmap status in the same change.

## Tolerances (exception triggers)

- Scope: if delivery requires touching more than 24 files or ~2200 net lines,
  stop and escalate.
- Interface: if public protocol schema in
  `crates/weaver-plugins/src/protocol/mod.rs` must break compatibility, stop
  and escalate.
- Dependencies: if adding new external crates beyond those already in the
  workspace becomes necessary, stop and escalate with justification.
- Iterations: if the same failing test loop repeats 5 times without progress,
  stop and escalate.
- Tooling: if rope cannot be executed in CI-compatible tests without unstable
  harness workarounds, stop and escalate with options.

## Risks

- Risk: `rope` runtime dependency may be missing in some environments.
  Severity: high. Likelihood: medium. Mitigation: keep runtime behaviour
  explicit when rope is unavailable and use DI-based unit/BDD tests that do not
  require a global rope installation.

- Risk: plugin diff output may not match `apply-patch` parser expectations.
  Severity: high. Likelihood: medium. Mitigation: reuse `apply_patch`
  parser/executor path for plugin diffs and add contract tests around diff
  format compatibility.

- Risk: sandbox profile may be too restrictive for rope adapter execution.
  Severity: medium. Likelihood: medium. Mitigation: explicitly model required
  executable/path allowances in manifest bootstrap and add failure tests for
  denied execution.

- Risk: adding runtime plugin state to dispatch may increase coupling.
  Severity: medium. Likelihood: medium. Mitigation: introduce a small runtime
  abstraction and keep routing logic thin, with unit tests around dependency
  boundaries.

## Progress

- [x] (2026-02-12 00:00Z) Drafted ExecPlan at
      `docs/execplans/3-1-1a-plugin-for-rope.md`.
- [x] (2026-02-13 01:30Z) Validated plugin bootstrap assumptions and added
      executable override via `WEAVER_ROPE_PLUGIN_PATH`.
- [x] (2026-02-13 02:00Z) Implemented `crates/weaver-plugin-rope/` with
      protocol handling, rope adapter boundary, and error mapping.
- [x] (2026-02-13 02:20Z) Wired `act refactor` to execute plugins and route
      diff output through the existing Double-Lock apply-patch flow.
- [x] (2026-02-13 02:50Z) Added unit, behavioural (`rstest-bdd` 0.5.0), and
      e2e (`assert_cmd` + `insta`) tests for happy, unhappy, and edge paths.
- [x] (2026-02-13 03:00Z) Updated design and user documentation and marked the
      roadmap rope entry as done.
- [x] (2026-02-13 03:20Z) Ran full quality gates successfully:
      `make fmt`, `make check-fmt`, `make lint`, `make test`,
      `make markdownlint`, and `make nixie`.

## Surprises & Discoveries

- Observation: project memory Model Context Protocol (MCP) resources were not
  available in this session (`list_mcp_resources` returned no
  servers/resources). Evidence: tool output returned empty resource lists.
  Impact: planning relied on repository docs and code inspection only.

## Decision Log

- Decision: implement a dedicated rope plugin executable crate in this phase,
  rather than embedding rope-specific logic into `weaverd`. Rationale:
  preserves plugin architecture boundaries and keeps specialist refactoring
  logic replaceable. Date/Author: 2026-02-12 / Codex

- Decision: route plugin-produced unified diffs through the existing
  `act apply-patch` execution path rather than introducing a second
  patch-application implementation. Rationale: avoids duplicated
  safety-critical logic and guarantees lock behaviour parity. Date/Author:
  2026-02-12 / Codex

- Decision: use DI boundaries for rope-adapter invocation and plugin runtime
  integration so tests do not depend on a system-wide rope installation.
  Rationale: deterministic tests with clear unhappy-path coverage. Date/Author:
  2026-02-12 / Codex

- Decision: add e2e command-ergonomics snapshots with a fake daemon instead of
  requiring a real rope runtime in e2e tests. Rationale: validates CLI usage
  and JSONL command shapes deterministically while keeping tests hermetic.
  Date/Author: 2026-02-13 / Codex

## Outcomes & Retrospective

- `act refactor --provider rope` now executes a sandboxed plugin runtime in
  `weaverd`, requiring `PluginOutput::Diff` and forwarding diff application to
  the existing `act apply-patch` Double-Lock path.
- Added `crates/weaver-plugin-rope/` as the first concrete actuator plugin.
  The first shipped operation is `rename`, with required arguments `offset` and
  `new_name`.
- Added daemon/runtime unit tests and behavioural tests to cover success,
  runtime failures, malformed diff responses, missing arguments, and no-change
  edge cases.
- Added end-to-end ergonomics tests in `crates/weaver-e2e/` using
  `assert_cmd` and `insta` for:
  - isolated `act refactor` invocation;
  - pipeline invocation chaining `observe` output through `jq`.
- Updated `docs/weaver-design.md`, `docs/users-guide.md`, and `docs/roadmap.md`
  to reflect shipped behaviour and configuration.
- Full repository quality gates passed after implementation and documentation
  updates.

## Context and orientation

Relevant existing components:

- `crates/weaver-plugins/` provides manifest, registry, process execution, and
  runner abstractions for sandboxed plugins.
- `crates/weaverd/src/dispatch/act/refactor/mod.rs` currently parses request
  arguments and builds a `PluginRequest`, but returns a "plugin execution not
  yet available" status.
- `crates/weaverd/src/dispatch/act/apply_patch/mod.rs` contains the existing
  patch parser + transaction flow that already enforces Double-Lock semantics.
- `crates/weaverd/src/process/launch.rs` constructs the dispatch runtime.
- `docs/roadmap.md` Phase 3 still marks rope plugin as incomplete.
- `docs/users-guide.md` currently documents `act refactor` as not yet wired
  end-to-end.

Testing and style references:

- `docs/rust-testing-with-rstest-fixtures.md`
- `docs/rstest-bdd-users-guide.md`
- `docs/reliable-testing-in-rust-via-dependency-injection.md`
- `docs/complexity-antipatterns-and-refactoring-strategies.md`
- `docs/rust-doctest-dry-guide.md`

## Plan of work

### Stage A: finalize runtime contract and boundaries

Define the concrete scope for Phase 3.1.1a:

- supported rope operations for first release (`rename` only),
- required operation arguments and error envelopes,
- plugin executable discovery/bootstrap strategy for `weaverd`.

Record these decisions in `docs/weaver-design.md` before implementation.

Go/no-go check:

- A single written contract exists for request arguments, plugin output, and
  failure semantics.

### Stage B: implement the rope plugin executable crate

Add new crate `crates/weaver-plugin-rope/` (workspace member) that:

- reads one `PluginRequest` JSON Lines (JSONL) line from stdin,
- validates operation + arguments,
- performs rope-backed refactoring via an adapter boundary,
- emits one `PluginResponse` JSONL line to stdout, with `PluginOutput::Diff`
  on success or diagnostics on failure.

Implementation notes:

- Keep main orchestration small; split argument parsing, rope adapter, and diff
  rendering into focused modules to avoid high cognitive complexity.
- Add rustdoc usage examples for public APIs where non-trivial.
- Ensure error mapping is explicit and stable.

Go/no-go check:

- `cargo test -p weaver-plugin-rope` passes including unhappy-path coverage.

### Stage C: add daemon plugin runtime bootstrap for rope

Extend `weaverd` runtime construction so `DomainRouter`/`act refactor` can
execute plugins, including rope manifest registration.

Likely touchpoints:

- `crates/weaverd/src/process/launch.rs`
- `crates/weaverd/src/dispatch/handler.rs`
- `crates/weaverd/src/dispatch/router.rs`
- new small runtime wiring module under `crates/weaverd/src/dispatch/`

Requirements:

- register rope plugin manifest with absolute executable path,
- return clear configuration/runtime errors when rope plugin is unavailable,
- keep runtime state testable via trait abstraction or injected executor.

Go/no-go check:

- unit tests can execute `act refactor` path with a mock executor without
  spawning external processes.

### Stage D: wire `act refactor` through plugin execution + Double-Lock

Replace current stub behaviour in
`crates/weaverd/src/dispatch/act/refactor/mod.rs` with end-to-end flow:

- parse and validate refactor arguments,
- read target file payload,
- execute provider plugin via runtime runner,
- require diff output for actuator success,
- feed diff through shared patch-execution path used by `apply-patch`,
- return structured success/failure to caller.

Refactor/compose `apply_patch` internals as needed so this path reuses existing
patch validation and transaction logic rather than copying it.

Go/no-go check:

- successful rope response writes validated changes,
- syntactic/semantic failures leave files unchanged,
- invalid plugin responses fail safely.

### Stage E: tests (unit + behavioural + e2e)

Add or update tests in both plugin and daemon layers.

Unit tests:

- rope plugin argument validation and operation dispatch,
- rope adapter error mapping,
- diff payload construction and protocol serialization,
- refactor handler flow using mock plugin executor and lock outcomes.

Behavioural tests (`rstest-bdd` v0.5.0, `&mut` worlds):

- happy: rope rename operation produces diff and commits after locks pass,
- unhappy: missing required operation argument,
- unhappy: unsupported refactoring operation,
- unhappy: plugin timeout/non-zero execution error,
- edge: plugin returns malformed/empty diff and filesystem remains unchanged,
- edge: lock rejection rolls back changes.

End-to-end tests (`assert_cmd` + `insta`) in `crates/weaver-e2e`:

- ergonomics: actuator command in isolation, demonstrating the expected
  `act refactor --provider rope ...` usage shape and user-visible output,
- pipeline ergonomics: `observe` query output piped through `jq` and then used
  to invoke the actuator command, snapshotting the full command transcript and
  resulting output for discoverable usage examples.

Likely files:

- `crates/weaver-plugin-rope/tests/features/rope_plugin.feature`
- `crates/weaver-plugin-rope/src/tests/behaviour.rs`
- `crates/weaverd/tests/features/refactor_rope.feature`
- `crates/weaverd/src/tests/refactor_rope_behaviour.rs`
- `crates/weaver-e2e/Cargo.toml` (add `assert_cmd` dev-dependency)
- `crates/weaver-e2e/tests/refactor_rope_cli_snapshots.rs`
- `crates/weaver-e2e/tests/snapshots/refactor_rope_cli_snapshots__*.snap`

### Stage F: documentation and roadmap updates

Update docs to match shipped behaviour:

- `docs/weaver-design.md`: add Phase 3.1.1a design decisions for rope adapter,
  runtime bootstrap, and diff-to-Double-Lock path.
- `docs/users-guide.md`: remove "not yet available" note for `act refactor`
  and document rope-supported operations, arguments, and failure modes.
- `docs/roadmap.md`: mark the rope plugin checklist item as done.

### Stage G: full quality gates

Run formatting, lint, tests, and markdown checks with `tee` and
`set -o pipefail`, review logs, and only then finalize.

## Concrete steps

1. Create the new crate and module skeletons.
2. Implement protocol handler, rope adapter boundary, and error mapping.
3. Add rope plugin unit tests and BDD scenarios.
4. Implement plugin runtime bootstrap in `weaverd` and inject into dispatch.
5. Replace refactor stub with plugin execution and shared patch application.
6. Add `weaverd` unit tests and BDD scenarios for refactor behaviour.
7. Add `assert_cmd` + `insta` e2e tests for isolated and pipeline ergonomics.
8. Update design docs, user docs, and roadmap status.
9. Run quality gates and inspect logs.

Commands (run from repository root):

    set -o pipefail && make fmt 2>&1 | tee /tmp/3-1-1a-make-fmt.log
    set -o pipefail && make check-fmt 2>&1 | tee /tmp/3-1-1a-check-fmt.log
    set -o pipefail && make lint 2>&1 | tee /tmp/3-1-1a-make-lint.log
    set -o pipefail && make test 2>&1 | tee /tmp/3-1-1a-make-test.log
    set -o pipefail && make markdownlint 2>&1 | tee /tmp/3-1-1a-markdownlint.log
    set -o pipefail && make nixie 2>&1 | tee /tmp/3-1-1a-nixie.log

Targeted test loops while implementing:

    set -o pipefail && cargo test -p weaver-plugin-rope 2>&1 | tee /tmp/3-1-1a-rope-plugin-test.log
    set -o pipefail && cargo test -p weaverd refactor 2>&1 | tee /tmp/3-1-1a-weaverd-refactor-test.log
    set -o pipefail && cargo test -p weaver-e2e refactor_rope_cli 2>&1 | tee /tmp/3-1-1a-weaver-e2e-refactor.log

## Validation and acceptance

The feature is accepted when all items below are true:

- `weaver act refactor --provider rope ...` executes the rope plugin path,
  returns status 0 on success, and writes verified edits.
- `act refactor` failures from plugin/runtime/lock validation are surfaced with
  clear errors and no partial filesystem writes.
- Unit tests cover request validation, runtime error mapping, and diff handoff
  to the Double-Lock path.
- BDD tests cover happy/unhappy/edge scenarios using `rstest-bdd` v0.5.0.
- E2E tests in `crates/weaver-e2e` use `assert_cmd` and `insta` to document
  and verify:
  - actuator command ergonomics in isolation,
  - a pipeline flow chaining `observe` output through `jq` into actuator
    invocation.
- `docs/weaver-design.md` records the decisions made by this implementation.
- `docs/users-guide.md` documents user-visible behaviour and configuration.
- `docs/roadmap.md` marks rope plugin entry as done.
- `make check-fmt`, `make lint`, and `make test` pass.

## Idempotence and recovery

- Implementation steps are re-runnable.
- If plugin execution fails, no edits are committed because commit only happens
  after Double-Lock success.
- If sandbox/plugin bootstrap fails, fix configuration or executable path and
  re-run tests.
- If markdown checks fail, run `make fmt` and re-run markdown validation before
  repeating Rust gates.

## Artifacts and notes

Expected artifacts:

- New crate: `crates/weaver-plugin-rope/`
- New/updated refactor runtime wiring in `crates/weaverd/src/dispatch/`
- New behavioural feature files and step definitions for rope plugin and
  `act refactor`
- Updated docs:
  `docs/weaver-design.md`, `docs/users-guide.md`, `docs/roadmap.md`

## Interfaces and dependencies

Planned interfaces (final names may vary but intent must hold):

- In `crates/weaver-plugin-rope/src/adapter.rs`:

      pub trait RopeAdapter {
          fn execute(&self, request: &PluginRequest) -> Result<PluginOutput, RopeAdapterError>;
      }

- In `crates/weaverd/src/dispatch/act/refactor/...`:

      pub trait RefactorPluginRuntime {
          fn execute(
              &self,
              provider: &str,
              request: &PluginRequest,
          ) -> Result<PluginResponse, PluginError>;
      }

- In `crates/weaverd/src/dispatch/act/apply_patch/...` (shared path): expose a
  crate-visible helper that executes a patch string through the existing parser
  and `ContentTransaction` flow, so refactor and apply-patch share the same
  safety-critical implementation.

Dependency expectations:

- Prefer existing workspace dependencies; avoid adding new third-party crates
  unless justified by a documented escalation.

## Revision note

Initial draft created for roadmap item: "Phase 3 -> first actuator plugin ->
rope".
