# 2.2.4 Provide contextual guidance when a domain is supplied without an operation

This ExecPlan (execution plan) is a living document. The sections
`Constraints`, `Tolerances`, `Risks`, `Progress`, `Surprises & Discoveries`,
`Decision Log`, and `Outcomes & Retrospective` must be kept up to date as work
proceeds.

Status: COMPLETE

This document must be maintained in accordance with `AGENTS.md` at the
repository root.

## Purpose / big picture

When an operator runs `weaver observe`, `weaver act`, or `weaver verify` today,
the CLI exits 1 with the generic message
`the command operation must be provided`. That confirms the invocation is
incomplete, but it does not help the operator discover the valid operations for
the chosen domain. The operator must inspect source, remember prior usage, or
consult external documentation. This is the gap described in
[Level 2](../ui-gap-analysis.md#level-2--domain-without-operation-weaver-observe)
 and [Level 10e](../ui-gap-analysis.md#level-10--error-messages-and-exit-codes).

After this change, invoking a known domain without an operation will fail
client-side with contextual guidance written to standard error. The output must
list every operation registered for that domain and include one concrete follow
up help command. The guidance must be available without daemon startup, socket
access, or configuration discovery.

Observable outcome:

```plaintext
$ weaver observe
error: operation required for domain 'observe'

Available operations:
  get-definition
  find-references
  grep
  diagnostics
  call-hierarchy
  get-card

Run 'weaver observe get-definition --help' for operation details.
```

The command exits non-zero. Equivalent guidance appears for `act` and `verify`,
with the operation list and hint adapted to the supplied domain.

## Constraints

- `make check-fmt`, `make lint`, and `make test` must pass before the change is
  complete.
- Because implementation will touch Markdown, also run `make fmt`,
  `make markdownlint`, and `make nixie` before finishing.
- No single code file may exceed 400 lines. This is an active constraint:
  `crates/weaver-cli/src/lib.rs` is 399 lines and
  `crates/weaver-cli/src/tests/unit.rs` is 398 lines before this work starts.
- `crates/weaver-cli/build.rs` includes `src/cli.rs` via
  `#[path = "src/cli.rs"]` for manpage generation. Do not add helper methods to
  `cli.rs` that are only used at runtime unless they also compile cleanly in
  the build-script context.
- The current command model deliberately forwards operation arguments verbatim
  after `<domain> <operation>`. Do not restructure the CLI into nested clap
  subcommands as part of this task; operation-specific help belongs to roadmap
  item 3.2.4.
- New behaviour requires unit coverage and behavioural coverage using
  `rstest-bdd` v0.5.0. Keep the scenario fixture parameter named `world`.
- Keep the discoverability path client-side. `weaver <domain>` must not depend
  on a daemon round trip.
- Comments and documentation use en-GB-oxendict spelling.
- Do not add new external dependencies.

## Tolerances (exception triggers)

- Scope: if implementation requires changes to more than 12 files or more than
  250 net lines, stop and escalate.
- Interface: if satisfying the acceptance criteria requires a public API change
  outside `weaver-cli`, stop and escalate.
- Help model: if a truthful implementation of the concrete
  `weaver <domain> <operation> --help` hint requires nested clap subcommands or
  another roadmap 3.2.4-scale restructure, stop and escalate.
- Line budget: if `src/lib.rs` or `src/tests/unit.rs` would exceed 400 lines,
  extract a helper module before adding further logic. Do not waive the limit.
- Catalogue drift: if the daemon router's known-operation lists differ from the
  client-side catalogue in additional ways beyond `observe get-card`, reconcile
  the drift first and document it before proceeding.

## Risks

- Risk: the current CLI-side catalogue is already stale. `DOMAIN_OPERATIONS` in
  `crates/weaver-cli/src/localizer.rs` omits `observe get-card`, but
  `crates/weaverd/src/dispatch/router.rs` includes it in
  `DomainRoutingContext::OBSERVE.known_operations`. Mitigation: reconcile the
  catalogue before using it for contextual guidance, then add a regression test
  tying the client-side list to the router contract.

- Risk: `lib.rs` has one line of headroom. A naive preflight branch for
  domain-only guidance will violate the repository-wide 400-line rule.
  Mitigation: move new runtime guidance logic into a dedicated helper module
  and keep the `lib.rs` call site minimal.

- Risk: the acceptance criteria requires a concrete
  `weaver <domain> <operation> --help` hint, but operation-specific help is not
  scheduled until roadmap 3.2.4. Today clap treats that form as top-level help,
  not operation-level help. Mitigation: use a deterministic concrete hint now,
  record the limitation in the design document and user's guide if it remains
  true after implementation, and avoid promising operation-specific argument
  help in this task.

- Risk: the existing integration test
  `crates/weaver-cli/tests/main_entry.rs::missing_operation_exits_with_failure`
  currently locks in the old generic message. Mitigation: update the test as
  part of this task so the binary-level contract matches the new guidance.

## Progress

- [x] (2026-03-10) Read the roadmap entry, UI gap analysis, design/testing
      references, and the current `weaver-cli` implementation.
- [x] (2026-03-10) Confirm the current call path:
      `Cli::try_parse_from(...)` -> bare-invocation shortcut -> config load ->
      `CommandInvocation::try_from(cli)`, which means missing-operation
      guidance is not currently emitted before configuration loading.
- [x] (2026-03-10) Identify catalogue drift: CLI help omits `observe get-card`
      while the daemon router advertises it.
- [x] (2026-03-10) Write the initial ExecPlan to
      `docs/execplans/2-2-4-provide-contextual-domain-guidance.md`.
- [x] (2026-03-12) Stage A: established
      `crates/weaver-cli/src/discoverability.rs` as the canonical client-side
      domain-operation catalogue and reconciled `observe get-card`.
- [x] (2026-03-12) Stage B: added a preflight guidance branch in
      `CliRunner::run_with_handler` that emits contextual guidance before
      config loading, daemon connection, or auto-start.
- [x] (2026-03-12) Stage C: added unit, behavioural, and integration coverage
      for the new guidance and preserved the complete-command config-failure
      path.
- [x] (2026-03-12) Stage D: updated `docs/weaver-design.md`,
      `docs/users-guide.md`, and marked roadmap item 2.2.4 complete.
- [x] (2026-03-12) Stage E: passed `make fmt`, `make markdownlint`,
      `make nixie`, `make check-fmt`, `make lint`, and `make test`.

## Surprises & Discoveries

- The client-side help catalogue introduced for roadmap 2.2.2 is not currently
  authoritative. It already drifted from `weaverd` by missing
  `observe get-card`. This task should not build new runtime behaviour on top
  of stale data.

- The current CLI architecture does not validate domain names when the
  operation is missing. `weaver bogus` still collapses into the generic
  missing-operation path because `CommandInvocation::try_from` only checks for
  presence and non-blank values. Unknown-domain validation is intentionally a
  later roadmap item (2.3.1), so this plan must preserve that boundary.

- The hint command required by the roadmap is ahead of the current help model.
  `weaver observe get-definition --help` is still handled by clap's top-level
  help path, as documented in `docs/ui-gap-analysis.md`.

- The 400-line file cap became a live implementation constraint rather than a
  planning note. `lib.rs` and `src/tests/unit.rs` both crossed the limit once
  the first draft landed, so shared helpers had to move into dedicated modules
  before validation could proceed.

## Decision Log

- Decision: add a dedicated client-side guidance module in `weaver-cli`
  instead of extending `CommandInvocation::try_from`. Rationale:
  `CommandInvocation` should remain a small data conversion layer. The new
  behaviour is a preflight UX path that belongs near the runtime boundary, and
  `lib.rs` does not have room for substantial new logic. Date: 2026-03-10.

- Decision: use a single canonical client-side domain catalogue for both
  top-level help assertions and domain-only guidance, and reconcile it with the
  daemon router before anything else. Rationale: this task already uncovered
  catalogue drift. Reusing stale or duplicated lists would compound the
  problem. Date: 2026-03-10.

- Decision: only known domains receive contextual guidance in this step.
  Unknown domains continue to follow the generic missing-operation path until
  roadmap 2.3.1 adds client-side unknown-domain validation. Rationale: this
  keeps the scope aligned with the roadmap and avoids mixing two error-policy
  changes in one feature. Date: 2026-03-10.

- Decision: choose the first operation in the canonical per-domain ordering as
  the concrete help hint. Rationale: the catalogue already defines a stable
  presentation order, and the acceptance criteria only requires one concrete
  hint, not heuristics. Date: 2026-03-10.

- Decision: list operation names only in the new contextual guidance block.
  Do not add per-operation summaries unless they emerge naturally from an
  existing authoritative catalogue during implementation. Rationale: the
  acceptance criteria requires complete operation enumeration and one concrete
  hint. Adding descriptive copy would broaden the catalogue surface and
  increase drift risk without being required for acceptance. Date: 2026-03-10.

- Decision: keep the top-level clap `after_help` text static in `cli.rs`, but
  drive all assertions from the canonical discoverability catalogue and keep
  the static text synchronized via tests. Rationale: `build.rs` includes
  `cli.rs` directly for manpage generation, so wiring runtime-only helpers into
  clap attributes would couple the build-script context unnecessarily. Date:
  2026-03-12.

## Outcomes & Retrospective

Implemented on 2026-03-12.

Behavioural outcome:

- `weaver <known-domain>` now exits non-zero with a contextual guidance block
  that lists that domain's registered operations and prints a deterministic
  `weaver <domain> <first-operation> --help` hint.
- The guidance is emitted before configuration discovery, daemon startup, or
  socket connection. Unknown domains still follow the generic missing-operation
  path until roadmap item 2.3.1.
- The canonical client-side catalogue now lives in
  `crates/weaver-cli/src/discoverability.rs`, and the `observe` list now
  includes `get-card` to match `weaverd`.

Implementation lesson:

- The file-size guardrails were correct. Extracting `discoverability`,
  `prepare_cli_arguments`, and capability-mode helpers kept the runtime change
  small enough to stay within the repository-wide 400-line cap while still
  making the new preflight branch explicit and testable.

## Context and orientation

The relevant runtime lives in `crates/weaver-cli/src/lib.rs`. The current flow
parses clap arguments, handles bare invocation, loads configuration, handles
`--capabilities` and lifecycle subcommands, and only then converts the parsed
CLI into a `CommandInvocation`. Missing-operation errors are therefore surfaced
late, after configuration loading, even though no daemon request is needed.

The relevant pieces today are:

- `crates/weaver-cli/src/lib.rs`
  Orchestrates argument parsing, early exits, config loading, daemon startup,
  and error-to-exit-code mapping.
- `crates/weaver-cli/src/command.rs`
  Converts `Cli` into `CommandInvocation`. It currently returns the generic
  `AppError::MissingOperation` whenever `operation` is absent or blank.
- `crates/weaver-cli/src/errors.rs`
  Owns the user-visible error strings and the sentinel `BareInvocation` path.
- `crates/weaver-cli/src/discoverability.rs`
  Holds the canonical client-side domain catalogue and renders the
  missing-operation guidance block.
- `crates/weaver-cli/src/tests/unit/bare_invocation.rs`
  Shows the established pattern for asserting a local guidance path that skips
  configuration loading by using a panicking loader.
- `crates/weaver-cli/src/tests/behaviour.rs` and
  `crates/weaver-cli/tests/features/weaver_cli.feature` Provide the
  `rstest-bdd` harness for user-visible CLI flows.
- `crates/weaver-cli/tests/main_entry.rs`
  Verifies binary-level behaviour with `assert_cmd`.
- `crates/weaverd/src/dispatch/router.rs`
  Contains the daemon-side `DomainRoutingContext::*::known_operations`
  constants that the CLI catalogue must mirror.

## Plan of work

### Stage A: reconcile and relocate the client-side catalogue

Create a small helper module under `crates/weaver-cli/src/` dedicated to
discoverability guidance. Move the client-side domain catalogue there so it is
no longer hidden inside `localizer.rs` test scaffolding. The module should hold:

- the canonical per-domain operation lists used by the CLI;
- a case-insensitive lookup function for known domains;
- a renderer or writer for the missing-operation guidance block.

Before introducing new behaviour, reconcile the `observe` list with
`crates/weaverd/src/dispatch/router.rs` by adding `get-card` and auditing the
other domains for exact ordering and membership. Re-export the canonical
catalogue from `lib.rs` if integration tests still need it.

Keep the module data-driven. A novice should be able to inspect one file and
see every domain and operation the CLI claims to support.

### Stage B: add a preflight guidance path before config loading

Insert a small preflight branch in `CliRunner::run_with_handler` immediately
after clap parsing and before `self.loader.load(...)`.

The branch must:

1. Preserve the existing bare-invocation behaviour.
2. Detect the new case:
   - no lifecycle subcommand;
   - no `--capabilities` probe;
   - `domain` is present and non-blank;
   - `operation` is absent or blank;
   - the supplied domain is recognised by the canonical client-side catalogue.
3. Write the contextual guidance block to standard error.
4. Exit with failure without loading configuration, attempting daemon
   connection, or triggering auto-start.

Do not change the low-level `CommandInvocation::try_from` behaviour for unknown
domains in this step. The preflight path should simply decline to handle
unrecognised domains and let the existing generic error path continue.

Because `AppError::BareInvocation` already exists as a sentinel for "guidance
already written", either introduce a second sentinel dedicated to
domain-guidance emission or rename the sentinel to something more general if
that keeps the code clearer. Keep the printed output single-source and avoid a
double-write in `map_result_to_exit_code`.

### Stage C: cover the behaviour at three levels

Add or update tests at three levels.

Unit tests:

- Add `crates/weaver-cli/src/tests/unit/domain_guidance.rs`.
- Use the `bare_invocation.rs` pattern with a panicking config loader to prove
  that `weaver observe`, `weaver act`, and `weaver verify` all short-circuit
  before configuration loading.
- Assert failure exit code, stderr-only output, full operation enumeration, and
  one concrete help hint per domain.
- Add one boundary test showing that an unknown domain such as `weaver bogus`
  does not use the contextual guidance block yet.
- Add one regression test proving the canonical client-side catalogue matches
  the daemon router's `known_operations` lists, so `get-card` cannot drift out
  again unnoticed.

Behavioural tests:

- Extend `crates/weaver-cli/tests/features/weaver_cli.feature` with a
  `Scenario Outline` covering `observe`, `act`, and `verify`.
- Reuse existing steps where possible:
  `When the operator runs`, `Then the CLI fails`, `And stderr contains`, and
  `And no daemon command was sent`.
- Include an unhappy-path scenario for `bogus` that confirms this task only
  changes known domains.
- Prefer reusing existing steps over adding new Rust step definitions. This
  keeps `src/tests/behaviour.rs` under the 400-line limit.

Integration tests:

- Update
  `crates/weaver-cli/tests/main_entry.rs::missing_operation_exits_with_failure`
  to assert the new output for `observe`.
- Add a small shared helper if needed so the test can assert every operation in
  the `observe` list without copy-pasting strings.

### Stage D: update operator and design documentation

Update `docs/weaver-design.md` in the CLI architecture section to record the
design decision that a known domain without an operation is handled entirely in
the client using a built-in catalogue, exits non-zero, and avoids daemon
startup.

Update `docs/users-guide.md` near the existing "Bare invocation" and "Top-level
help" sections with a new subsection describing `weaver <domain>` behaviour.
Include one sample output block and make clear that the command is a
discoverability aid, not a daemon request.

When the implementation and validation are complete, mark roadmap item 2.2.4
done in `docs/roadmap.md`.

### Stage E: validate end to end

Run the required gates with log capture:

```sh
set -o pipefail; make fmt 2>&1 | tee /tmp/2-2-4-fmt.log
set -o pipefail; make markdownlint 2>&1 | tee /tmp/2-2-4-markdownlint.log
set -o pipefail; make nixie 2>&1 | tee /tmp/2-2-4-nixie.log
set -o pipefail; make check-fmt 2>&1 | tee /tmp/2-2-4-check-fmt.log
set -o pipefail; make lint 2>&1 | tee /tmp/2-2-4-lint.log
set -o pipefail; make test 2>&1 | tee /tmp/2-2-4-test.log
```

Success means:

1. The new unit tests pass.
2. The new `rstest-bdd` scenarios pass.
3. The updated integration test passes.
4. No existing help, version, bare-invocation, or auto-start regression fails.
5. The formatted docs and source files remain within repository policy.

## Acceptance checklist

The implementation is complete only when all of the following are true:

1. `weaver <known-domain>` without an operation exits non-zero.
2. The output lists every operation registered for that domain.
3. The output includes one concrete
   `weaver <domain> <operation> --help` hint.
4. The path does not load configuration, connect to the daemon, or trigger
   auto-start.
5. `weaver bogus` is unchanged by this task and remains reserved for roadmap
   2.3.1.
6. `docs/weaver-design.md`, `docs/users-guide.md`, and `docs/roadmap.md` are
   updated.
7. `make fmt`, `make markdownlint`, `make nixie`, `make check-fmt`,
   `make lint`, and `make test` all succeed.
