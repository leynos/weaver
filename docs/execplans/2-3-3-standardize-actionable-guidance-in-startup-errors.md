# 2.3.3 Standardize actionable guidance in startup and routing errors

This ExecPlan (execution plan) is a living document. The sections
`Constraints`, `Tolerances`, `Risks`, `Progress`, `Surprises & Discoveries`,
`Decision Log`, and `Outcomes & Retrospective` must be kept up to date as work
proceeds.

Status: DRAFT

This document must be maintained in accordance with `AGENTS.md` at the
repository root.

## Purpose / big picture

Roadmap item `2.3.3` is the unification step that sits on top of completed
items `2.2.1`, `2.2.4`, `2.3.1`, and `2.3.2`. Today the Level 10 failure paths
already exist, but they still read like separate features:

- bare invocation prints short help with no explicit error line;
- unknown domains list valid domains but do not always tell the operator what
  to run next;
- unknown operations list alternatives but also stop short of an explicit next
  command;
- daemon auto-start failures still dump raw `LifecycleError` text.

After this change, every Level 10 path (`10a` through `10e`) must render the
same three-part shape in human-readable mode:

```plaintext
error: <problem statement>

<alternatives block>

Next command:
  <exact command>
```

The middle block may stay domain-specific (`Usage: ...`, `Valid domains: ...`,
or `Available operations: ...`), but every path must expose the same observable
structure: error, alternatives, and one concrete next step.

This work is successful when the following are all true:

1. `weaver` with no arguments, `weaver <domain>`,
   `weaver <unknown-domain> ...`, `weaver ... <unknown-operation>`, and daemon
   start failures all print the same three-part layout.
2. the `WEAVERD_BIN` failure path tells operators how to check installation and
   how to override the daemon binary path;
3. stable non-zero exit behaviour does not change;
4. unit tests and `rstest-bdd` scenarios cover the happy path, unhappy paths,
   and edge cases;
5. `docs/weaver-design.md`, `docs/users-guide.md`, and `docs/roadmap.md`
   reflect the shipped behaviour.

## Constraints

- Run `make check-fmt`, `make lint`, and `make test` before considering the
  feature complete.
- Because this task also updates Markdown, run `make fmt`,
  `make markdownlint`, and `make nixie` before finishing.
- Add both unit coverage and behavioural coverage using `rstest-bdd` v0.5.0.
  Reuse the existing feature harnesses where possible rather than creating a
  second bespoke end-to-end harness.
- Preserve the existing division of responsibility:
  - unknown domains and missing operations remain client-side preflight work;
  - unknown operations remain daemon-routed and use the daemon as the source of
    truth for known operations;
  - startup failures remain lifecycle errors emitted by the CLI.
- Preserve current non-zero exit semantics:
  - bare invocation and client-side preflight failures still exit through the
    CLI failure path;
  - unknown-operation errors still emit daemon exit status `1`;
  - startup failures still exit non-zero.
- Do not widen the outer JSONL transport envelope. The `stream` and `exit`
  message contract must remain unchanged.
- Prefer additive CLI-side formatting over changing `Display` implementations
  on `AppError` or `LifecycleError`. The new operator contract is a rendering
  concern, not a reason to blur internal error types.
- Do not add a new dependency for formatting or edit-distance logic.
- Keep files under 400 lines by extracting helpers or tests into focused
  modules before crossing the limit.
- Update `docs/weaver-design.md` with the final error-template policy and any
  startup-guidance design decision.
- Update `docs/users-guide.md` with the final operator-visible outputs.
- Mark roadmap item `2.3.3` done only after implementation, documentation, and
  all validation gates pass.
- Comments and documentation must use en-GB-oxendict spelling.
- Do not broaden this task into the locale-selection roadmap (`3.3.x`).
  Preflight/localized text should continue to use the current localizer, but
  the general human output renderer may remain English-only for now.

## Tolerances (exception triggers)

- Scope: if implementation requires changes to more than 16 files or roughly
  500 net lines, stop and re-evaluate before continuing.
- Interfaces: if a public wire type in `crates/weaver-daemon-types/` must
  change to satisfy the acceptance criteria, stop and escalate before changing
  the JSON contract.
- Dependencies: if the formatter or startup guidance appears to require a new
  external crate, stop and escalate.
- Behaviour drift: if satisfying `2.3.3` would force a change to the completed
  `2.3.1` or `2.3.2` acceptance criteria beyond layout unification, stop and
  document the conflict first.
- File-size pressure: if `crates/weaver-cli/src/lib.rs`,
  `crates/weaver-cli/src/discoverability.rs`, or
  `crates/weaver-cli/src/output/mod.rs` would exceed 400 lines, extract a new
  helper module before adding more logic.
- Ambiguity: if there is no single defensible `Next command:` for a failure
  case without product input, stop and present the options with trade-offs.
- Iterations: if the same failing validator needs more than five correction
  attempts, stop and report the blocker.

## Risks

- Risk: bare invocation currently satisfies roadmap `2.2.1` by printing short
  help, but it does not have an explicit `error:` line. A careless refactor
  could break the earlier `Usage:` and single-pointer requirements while
  chasing the new template. Severity: medium. Likelihood: high. Mitigation:
  drive the change with focused tests in `crates/weaver-cli/src/tests/unit/`
  and keep the `Usage:` line plus exactly one help pointer in the final output.

- Risk: unknown-operation guidance is intentionally daemon-routed, and the CLI
  must not rebuild the operation list from `DOMAIN_OPERATIONS`. Severity: high.
  Likelihood: medium. Mitigation: derive the `Next command:` from the daemon
  payload's first `known_operations` entry and leave the JSON payload shape
  unchanged unless a test proves that insufficient.

- Risk: lifecycle errors are surfaced through two different paths today:
  explicit `weaver daemon start` failures go through `AppError::Lifecycle`,
  while auto-start failures are printed directly in
  `execute_daemon_command(...)`. Severity: medium. Likelihood: high.
  Mitigation: add one CLI-side lifecycle-guidance renderer and route both print
  sites through it.

- Risk: the Fluent catalogue already contains some guidance strings, including
  an unused unknown-domain hint entry. It is easy to add hard-coded English
  text instead of reusing the existing localization seam. Severity: low.
  Likelihood: high. Mitigation: keep preflight and bare-invocation copy inside
  `crates/weaver-cli/locales/en-US/messages.ftl` and continue using
  `strip_bidi_isolates(...)`.

- Risk: startup guidance can sprawl if every lifecycle variant gets bespoke
  prose. Severity: medium. Likelihood: medium. Mitigation: define one small
  internal guidance model with a problem line, a list of preformatted
  alternatives lines, and a single next command, then map only the relevant
  startup variants onto it.

## Progress

- [x] (2026-04-07) Read `docs/roadmap.md`, `docs/ui-gap-analysis.md`,
  `docs/weaver-design.md`, `docs/users-guide.md`, and the referenced testing
  guidance.
- [x] (2026-04-07) Confirmed the current Level 10 implementation split across
  `crates/weaver-cli/src/localizer.rs`,
  `crates/weaver-cli/src/discoverability.rs`,
  `crates/weaver-cli/src/output/mod.rs`, and `crates/weaver-cli/src/lib.rs`.
- [x] (2026-04-07) Confirmed that the workspace already pins `rstest-bdd` and
  `rstest-bdd-macros` at `0.5.0`; no dependency upgrade is needed.
- [x] (2026-04-07) Confirmed that the daemon already emits structured
  `UnknownOperation` payloads and stable exit status `1`.
- [x] (2026-04-07) Drafted this ExecPlan in
  `docs/execplans/2-3-3-standardize-actionable-guidance-in-startup-errors.md`.
- [ ] Stage A: add failing unit and behavioural tests for the unified
  three-part template.
- [ ] Stage B: introduce a small CLI-side actionable-guidance formatter and
  refactor bare invocation plus preflight domain guidance to use it.
- [ ] Stage C: route unknown-operation human rendering and startup/lifecycle
  failures through the same formatter while preserving existing exit codes and
  daemon payload semantics.
- [ ] Stage D: update `docs/weaver-design.md`, `docs/users-guide.md`, and
  `docs/roadmap.md`.
- [ ] Stage E: run the full Markdown and Rust validation gates sequentially.

## Surprises & Discoveries

- `crates/weaver-cli/locales/en-US/messages.ftl` already contains
  `weaver-domain-guidance-help-hint-unknown-domain`, but the current Rust code
  does not use it. That is a strong sign that the intended next-command surface
  was anticipated but not yet wired in.

- The daemon-side unknown-operation contract is already in a good place for
  this roadmap item. `crates/weaverd/src/dispatch/response.rs` serializes
  `known_operations` and the CLI already parses it in human mode. The missing
  piece is the next-command line, not a new daemon payload type.

- `crates/weaver-cli/src/localizer.rs::write_bare_help(...)` is the outlier.
  It writes a help block directly rather than building from the same guidance
  shape as the other preflight paths.

- Explicit lifecycle start failures and automatic daemon-start failures already
  share `LifecycleError`, but they do not share a rendering path. This is an
  opportunity to remove drift without changing the core lifecycle logic.

## Decision Log

- Decision: keep the canonical three-part template in `weaver-cli`, not in the
  daemon wire protocol. Rationale: four of the five Level 10 paths are already
  CLI-rendered, and the remaining path (`UnknownOperation`) can be completed by
  formatting the daemon's existing payload more completely. Date: 2026-04-07.

- Decision: preserve the existing daemon JSON payload for unknown operations
  unless Stage A proves the CLI cannot produce an acceptable next command from
  the existing `known_operations` array. Rationale: roadmap `2.3.3` asks for
  consistent rendered guidance and stable exit codes, not a broader protocol
  revision. Date: 2026-04-07.

- Decision: treat startup guidance as a rendering layer over `LifecycleError`
  rather than rewriting the error enum's `Display` strings. Rationale:
  `Display` remains useful for logs and internal assertions, while the new
  operator contract belongs in one dedicated formatter. Date: 2026-04-07.

- Decision: apply the startup formatter to both auto-start failures and
  explicit `weaver daemon start` failures. Rationale: both failure modes are
  surfaced to operators and should not drift just because they reach stderr
  through different call sites. Date: 2026-04-07.

## Outcomes & Retrospective

Target outcome at completion:

1. Level 10a through 10e all print the same three-part layout in
   human-readable mode.
2. The Level 10a missing-daemon-binary path names both recovery options:
   installation/PATH validation and `WEAVERD_BIN`.
3. The CLI still avoids daemon startup for unknown domains and missing
   operations.
4. Unknown operations still return the daemon's exit status `1` and preserve
   the existing JSON payload in `--output json` mode.
5. Bare invocation still prints a `Usage:` line, all three valid domains, and
   exactly one help pointer.
6. The feature is covered by unit tests and `rstest-bdd` scenarios, and the
   final repository state passes `make fmt`, `make markdownlint`, `make nixie`,
   `make check-fmt`, `make lint`, and `make test`.
7. `docs/weaver-design.md`, `docs/users-guide.md`, and `docs/roadmap.md`
   accurately describe the shipped behaviour.

Retrospective notes will be added after implementation.

## Context and orientation

The current code is already close to the target behaviour, but the formatting
logic is fragmented.

`crates/weaver-cli/src/localizer.rs` owns `write_bare_help(...)`, which writes
the current bare-invocation block directly. That block has the right
information for roadmap `2.2.1`, but it does not expose the standard error /
alternatives / next-command structure needed by `2.3.3`.

`crates/weaver-cli/src/discoverability.rs` owns
`write_missing_operation_guidance(...)` and
`write_unknown_domain_guidance(...)`. These already print a problem line plus
alternatives, and they already use the localizer, but they are not built from a
shared formatter. Unknown domains also stop after the alternatives block unless
the code is extended to use the existing unknown-domain hint message.

`crates/weaver-cli/src/output/mod.rs` owns `render_unknown_operation(...)`.
This is the Level 10c path. It already prints the daemon-provided operations,
so the source of truth is correct; it simply needs the final `Next command:`
line and shared layout.

`crates/weaver-cli/src/lib.rs` owns the print sites. Bare invocation and
preflight guidance are emitted in `handle_preflight(...)`. Auto-start failures
are printed inside `execute_daemon_command(...)` after
`try_auto_start_daemon(...)` returns a `LifecycleError`. Explicit lifecycle
command failures travel through `AppError::Lifecycle` and are printed from
`CliRunner::map_result_to_exit_code(...)`.

`crates/weaver-cli/src/lifecycle/error.rs` defines the lifecycle failure
variants that matter here, especially `LaunchDaemon`, `StartupFailed`,
`StartupAborted`, and `StartupTimeout`. The plan should not overload their
`Display` text with presentation responsibilities.

`crates/weaverd/src/dispatch/response.rs` is already the daemon-side authority
for unknown-operation payloads. Because `known_operations` is serialized there,
the CLI can derive a deterministic next command from the first returned entry
without consulting its own catalogue.

The most relevant tests already exist:

- `crates/weaver-cli/src/tests/unit/bare_invocation.rs`
- `crates/weaver-cli/src/tests/unit/missing_operation_guidance.rs`
- `crates/weaver-cli/src/tests/unit/auto_start.rs`
- `crates/weaver-cli/tests/features/weaver_cli.feature`
- `crates/weaver-cli/src/tests/behaviour.rs`
- `crates/weaverd/tests/features/daemon_dispatch.feature`

The documentation surfaces that must be updated after implementation are:

- `docs/weaver-design.md` (CLI preflight and startup guidance policy)
- `docs/users-guide.md` (operator-visible examples)
- `docs/roadmap.md` (mark `2.3.3` done)

## Plan of work

### Stage A: lock the contract with failing tests

Add or update focused tests before changing any formatter code.

In `crates/weaver-cli/src/tests/unit/bare_invocation.rs`, extend the bare
invocation assertions so they require:

- an explicit `error:` line;
- a `Usage:` line;
- the three valid domains;
- exactly one `weaver --help` pointer;
- a two-block layout that makes the `Next command:` line observable.

In `crates/weaver-cli/src/tests/unit/missing_operation_guidance.rs`, extend the
known-domain and unknown-domain cases so they both require a `Next command:`
block. Use the existing unique-suggestion and no-suggestion cases:

- when there is exactly one domain suggestion, the next command should use that
  domain and its first known operation;
- when there is no domain suggestion, the next command should fall back to
  `weaver --help`.

In `crates/weaver-cli/src/output/mod.rs` tests, extend the unknown-operation
human rendering assertions so they require a `Next command:` line derived from
the first `known_operations` entry in the daemon payload.

In `crates/weaver-cli/src/tests/unit/auto_start.rs`, add explicit assertions
for the missing-binary path so the stderr output mentions both installation
checking and `WEAVERD_BIN`, and still exits with failure. Add a second focused
unit test for `weaver daemon start` using the existing injected-daemon-binary
test seam so the explicit lifecycle path renders the same startup guidance.

In `crates/weaver-cli/tests/features/weaver_cli.feature` and
`crates/weaver-cli/src/tests/behaviour.rs`, extend the existing behavioural
scenarios for:

- bare invocation;
- unknown domain;
- unknown operation in human mode;
- auto-start spawn failure.

Keep the current happy-path scenarios (`streaming a request`, lifecycle
success, capability probe, and valid daemon interaction) untouched so they
continue to prove that successful behaviour is unchanged.

Only add daemon-side test changes if Stage C ends up requiring a payload
revision. Otherwise, the existing `weaverd` unknown-operation tests are already
the stable-exit-code guard.

### Stage B: introduce one CLI-side actionable-guidance formatter

Create a small internal helper module, likely
`crates/weaver-cli/src/actionable_guidance.rs`, with a data model similar to:

```rust
pub(crate) struct ActionableGuidance {
    pub(crate) problem: String,
    pub(crate) alternatives: Vec<String>,
    pub(crate) next_command: String,
}

pub(crate) fn write_actionable_guidance<W: Write>(
    writer: &mut W,
    guidance: &ActionableGuidance,
) -> io::Result<()>;
```

Keep the helper generic: the `alternatives` field should accept already-shaped
lines so the middle block can serve all five Level 10 cases without awkward
special cases. For example, bare invocation can include `Usage:` and domain
rows, while unknown operations can include `Available operations:` and one
operation per line.

Refactor `crates/weaver-cli/src/localizer.rs::write_bare_help(...)` so it
either delegates to the new helper or is replaced by a new
`write_missing_domain_guidance(...)` function that uses the same formatter and
still pulls copy from the localizer.

Refactor `crates/weaver-cli/src/discoverability.rs` so
`write_missing_operation_guidance(...)` and
`write_unknown_domain_guidance(...)` construct `ActionableGuidance` values
instead of writing ad hoc blocks. Reuse the existing Fluent catalogue and add
only the minimum new entries needed for `Next command:` or revised hint text.

Do not move unknown-operation rendering into this module yet. First finish the
preflight and bare-invocation paths, because they are the most sensitive to the
existing localized strings and roadmap `2.2.x` guarantees.

### Stage C: finish the routing and startup surfaces

Update `crates/weaver-cli/src/output/mod.rs::render_unknown_operation(...)` so
the returned human-readable string follows the same three-part layout and ends
with:

```plaintext
Next command:
  weaver <domain> <first-known-operation> --help
```

The operation hint must come from the daemon payload, not from
`crates/weaver-cli/src/discoverability.rs`.

Add a dedicated lifecycle-guidance rendering seam in `weaver-cli`, either in
the new `actionable_guidance` module or as a focused helper beside the
lifecycle module. The renderer should map at least these variants:

- `LifecycleError::LaunchDaemon` with missing binary: mention installation/PATH
  checking and `WEAVERD_BIN`, and use `command -v weaverd` as the deterministic
  next command;
- `LifecycleError::StartupFailed`, `LifecycleError::StartupTimeout`, and
  `LifecycleError::StartupAborted`: keep the same three-part layout and direct
  the operator to rerun in the foreground using
  `WEAVER_FOREGROUND=1 weaver daemon start`.

Then route both print sites through this helper:

1. the auto-start failure branch in
   `crates/weaver-cli/src/lib.rs::execute_daemon_command(...)`;
2. the `AppError::Lifecycle` case in
   `crates/weaver-cli/src/lib.rs::CliRunner::map_result_to_exit_code(...)`.

Keep the exit codes unchanged. This stage is about rendering only.

### Stage D: update the design and user-facing documents

Update `docs/weaver-design.md` so it explains:

- the shared three-part error template;
- the decision to keep unknown-operation alternatives daemon-sourced while
  finishing the human render in the CLI;
- the startup-guidance policy for missing `weaverd`, installation checks, and
  `WEAVERD_BIN`.

Update `docs/users-guide.md` so the operator-visible examples match the shipped
outputs for:

- bare invocation;
- unknown domain with and without a suggestion;
- unknown operation in human mode;
- daemon auto-start or explicit start failure caused by a missing `weaverd`
  binary.

Update `docs/roadmap.md` only after the code and all documentation are final,
and only when the validation gates are green.

### Stage E: run the full validation suite and capture evidence

Run fast focused tests while iterating, then run the full gates sequentially.
Do not run format, lint, and test commands in parallel in this repository.

## Concrete steps

All commands below are run from the repository root.

During red/green iteration, use focused tests such as:

```sh
cargo test -p weaver-cli bare_invocation
cargo test -p weaver-cli missing_operation_guidance
cargo test -p weaver-cli auto_start
cargo test -p weaver-cli behaviour
```

If Stage C requires daemon payload changes, add:

```sh
cargo test -p weaverd dispatch_behaviour
```

When the feature is ready, run the full validation sequence in this exact order:

```sh
make fmt
make markdownlint
make nixie
make check-fmt
make lint
make test
```

Expected success indicators:

```plaintext
$ make check-fmt
cargo fmt --workspace -- --check

$ make lint
cargo clippy --workspace --all-targets --all-features -- -D warnings

$ make test
cargo test --workspace
```

The behavioural evidence to inspect manually after the tests pass is:

```plaintext
$ weaver
error: ...

Usage: ...
...

Next command:
  weaver --help
```

and:

```plaintext
$ weaver --output human observe nonexistent
error: unknown operation 'nonexistent' for domain 'observe'

Available operations:
  get-definition
  ...

Next command:
  weaver observe get-definition --help
```

and:

```plaintext
$ weaver observe get-definition --symbol main
Waiting for daemon start...
error: ...

Valid alternatives:
  ...

Next command:
  command -v weaverd
```

The exact prose may differ slightly after implementation, but the three-part
layout and stable non-zero exits must match.

## Validation and acceptance

Acceptance is behavioural, not structural.

- Tests:
  - unit tests prove every Level 10 path has the expected three-part layout;
  - `rstest-bdd` scenarios prove the CLI emits the new guidance in realistic
    command flows;
  - existing success-path scenarios continue to pass unchanged.
- Lint and formatting:
  - `make fmt`, `make markdownlint`, `make nixie`, `make check-fmt`, and
    `make lint` all pass.
- Runtime contract:
  - unknown-operation JSON output still forwards the daemon payload unchanged;
  - unknown-operation failures still exit with daemon status `1`;
  - preflight and startup failures still exit non-zero;
  - unknown-domain and missing-operation failures still happen before daemon
    startup.

This task is done only when the roadmap item can honestly be marked complete.

## Idempotence and recovery

The implementation steps are safe to repeat.

If a focused test fails partway through the refactor, revert only the most
recent guidance-path changes and rerun the focused test before touching the
docs. Do not mark the roadmap item done until the full validation sequence is
green.

If the work unexpectedly requires a daemon wire-contract change, stop, document
that in `Decision Log`, and re-scope before editing
`crates/weaver-daemon-types/`.

## Artefacts and notes

The most important artefacts for this task are short stderr transcripts and the
validator output from the final sequential run.

Capture at least one transcript each for:

- bare invocation;
- unknown domain with suggestion;
- unknown operation in human mode;
- missing `weaverd` startup failure.

Keep those artefacts concise and focused on the new layout.

## Interfaces and dependencies

The implementation should stay within existing crates and seams.

- Add one internal formatter module in `crates/weaver-cli/src/` rather than a
  new crate.
- Keep using `ortho_config::Localizer` and
  `crates/weaver-cli/locales/en-US/messages.ftl` for preflight and bare-help
  copy.
- Keep using `weaver_daemon_types::UnknownOperationPayload` as the shared
  daemon/CLI payload for Level 10c unless a blocking gap is proven.
- Keep `LifecycleError` as the lifecycle domain model; add renderer helpers
  around it instead of changing the error enum into a presentation object.

The likely touched source files are:

- `crates/weaver-cli/src/lib.rs`
- `crates/weaver-cli/src/localizer.rs`
- `crates/weaver-cli/src/discoverability.rs`
- `crates/weaver-cli/src/output/mod.rs`
- `crates/weaver-cli/src/lifecycle/error.rs` or a new nearby helper module
- `crates/weaver-cli/locales/en-US/messages.ftl`
- `crates/weaver-cli/src/tests/unit/bare_invocation.rs`
- `crates/weaver-cli/src/tests/unit/missing_operation_guidance.rs`
- `crates/weaver-cli/src/tests/unit/auto_start.rs`
- `crates/weaver-cli/src/tests/behaviour.rs`
- `crates/weaver-cli/tests/features/weaver_cli.feature`
- `docs/weaver-design.md`
- `docs/users-guide.md`
- `docs/roadmap.md`

## Revision note

Initial draft created from roadmap item `2.3.3`, the current CLI and daemon
code, and the adjacent ExecPlans for `2.3.1` and `2.3.2`. No implementation has
started yet.
