# Extend `daemon start` help with config and environment guidance

This ExecPlan (execution plan) is a living document. The sections
`Constraints`, `Tolerances`, `Risks`, `Progress`, `Surprises & Discoveries`,
`Decision Log`, and `Outcomes & Retrospective` must be kept up to date as work
proceeds.

Status: DRAFT

This document must be maintained in accordance with `AGENTS.md` at the
repository root. Approval from the user is required before implementation
begins.

## Purpose / big picture

Roadmap item `3.2.2` ("Extend `daemon start` help with config and environment
guidance"; see `docs/roadmap.md` and the `Level 8` remedy in
`docs/ui-gap-analysis.md`) closes the gap where `weaver daemon start --help`
silently omits the two environment variables that materially change start-up
behaviour. Today the rendered help shows only the short description, the
`--help` option, and the cross-cutting ordering caveat. It says nothing about
`WEAVERD_BIN` (overrides the daemon binary the CLI spawns) or
`WEAVER_FOREGROUND` (keeps the daemon attached to the controlling terminal),
even though both are documented elsewhere in the user's guide and surfaced in
actionable error messages.

After this change a newcomer can read `weaver daemon start --help` and
discover, without leaving the terminal:

1. That `WEAVERD_BIN` exists, what it overrides, and what its default
   resolution is (`weaverd` on `PATH`).
2. That `WEAVER_FOREGROUND` exists, what it does (foregrounds the daemon),
   and that it is enabled by *any* non-empty value (the daemon checks
   `env::var_os(...).is_some()` at `crates/weaverd/src/process/launch.rs:42`,
   so the user's guide convention `WEAVER_FOREGROUND=1` is illustrative,
   not required).
3. At least one concrete startup example using one of these overrides — for
   example, `WEAVERD_BIN=/opt/weaver/bin/weaverd weaver daemon start` — so
   operators can copy-paste a working invocation rather than reverse-engineer
   it.

The shared configuration flags (`--config-path`, `--daemon-socket`,
`--log-filter`, `--log-format`, `--capability-overrides`, `--locale`) are
already surfaced under `daemon start --help` by the help-augmentation layer in
`crates/weaver-cli/src/help.rs` (delivered in 3.2.1). This plan adds env-var
guidance and an example without disturbing that layer or the runtime parsing
contract.

Observable outcome:

```plaintext
$ weaver daemon start --help
Starts the daemon and waits for readiness.

Environment variables:
  WEAVERD_BIN
      Overrides the path to the weaverd binary that the CLI spawns. When unset
      the CLI launches `weaverd` from PATH. Used by the auto-start path and by
      `daemon start` itself.

  WEAVER_FOREGROUND
      When set to any non-empty value, runs the daemon in the foreground
      attached to the controlling terminal so its startup output is visible.
      Useful for interactive debugging and CI jobs.

Examples:
  Start the daemon using a custom binary path:
    WEAVERD_BIN=/opt/weaver/bin/weaverd weaver daemon start

  Start the daemon in the foreground for debugging:
    WEAVER_FOREGROUND=1 weaver daemon start

Usage: start

Options:
      --config-path <PATH>
      --daemon-socket <ENDPOINT>
      --log-filter <FILTER>
      --log-format <FORMAT>
      --capability-overrides <DIRECTIVE>
      --locale <LOCALE>
  -h, --help
          Print help

Config flags must appear before the command domain or structured subcommand to
take effect; for example, `weaver daemon start --log-filter debug` is ignored
because `--log-filter` appears after `start`.
```

The exact wording is a draft; the structural requirements are: both env-var
names appear verbatim, the `WEAVER_FOREGROUND=...` semantics are described
honestly (any non-empty value), at least one example uses an override, and the
existing ordering caveat block at the end is preserved.

## Constraints

- Run `make check-fmt`, `make lint`, and `make test` before considering the
  feature complete. These wrap `cargo fmt --workspace -- --check`,
  `cargo clippy --workspace --all-targets --all-features -- -D warnings`, and
  `cargo test --workspace`.
- Because the implementation also updates Markdown, run `make fmt`,
  `make markdownlint`, and `make nixie` before finishing.
- Add unit coverage and behavioural coverage using `rstest-bdd` v0.5.0 for
  the new help text, including a happy path (the variables and example are
  present) and unhappy-path / edge-case coverage (help still renders correctly
  when `WEAVERD_BIN` or `WEAVER_FOREGROUND` are set in the environment, and
  when the daemon binary is absent — help must not depend on the daemon
  process).
- Do not change runtime parsing semantics. The runtime CLI must still
  reject configuration flags that appear after the command domain or
  structured subcommand. Adding `long_about` text is help-only.
- Do not introduce new runtime configuration. `WEAVERD_BIN` and
  `WEAVER_FOREGROUND` already exist in the codebase. This plan documents
  them; it does not change their semantics.
- The new help text must remain consistent with the canonical wording in
  `docs/users-guide.md` lines 165–262. Where the help text is intentionally
  briefer, the user's guide remains the authoritative long-form reference.
- The shared `cli.rs` module is consumed by both the runtime parser and the
  `clap_mangen` build script (see `crates/weaver-cli/Cargo.toml`
  `[build-dependencies]`). The new `long_about` must therefore not contain
  characters or patterns that break manpage generation. Plain ASCII with
  newlines is sufficient.
- Help rendering must not require configuration loading. The existing
  `PanickingLoader` in `crates/weaver-cli/src/tests/unit/help_output.rs`
  guarantees this for the augmented command; new tests must preserve that
  invariant.
- The augmented snapshot at
  `crates/weaver-cli/src/tests/unit/snapshots/weaver_cli__tests__unit__help_output__daemon_start_augmented_help.snap`
  will need to be regenerated. Do that by running the relevant test under
  `INSTA_UPDATE=auto cargo test -p weaver-cli ...` and then committing the
  updated `.snap` file alongside the source change. Do not blindly accept;
  inspect the diff.
- All Markdown edits must use en-GB-oxendict spelling and grammar per
  `AGENTS.md`. Comments and docstrings in Rust source must follow the same
  convention.

## Tolerances (exception triggers)

- Scope: if implementation requires changes to more than 8 files or more than
  ~250 lines of code (net), stop and escalate. Expected scope is `cli.rs` plus
  one or two test files plus two doc files.
- Interface: this is a help-text-only change. If any change to a public Rust
  API or to runtime parsing semantics becomes necessary, stop and escalate.
- Dependencies: no new external dependencies are expected. If one becomes
  necessary, stop and escalate.
- Iterations: if the BDD or unit suite still fails after 3 fix attempts on the
  same scenario, stop and escalate with the failing diff and the latest
  output.
- Snapshot churn: if the snapshot diff includes anything other than the new
  `long_about` block, stop and investigate before accepting — that signals an
  unintended change to the augmented command.
- Time: if any milestone takes more than 4 wall-clock hours of active work,
  stop and escalate.
- Ambiguity: if the wording of the env-var description is ambiguous between
  "set to 1" (per existing user-guide examples) and "set to any value" (per
  actual code semantics), present both options with trade-offs.

## Risks

- Risk: the `attach_ordering_caveat` recursion in
  `crates/weaver-cli/src/help.rs:133-140` operates on the augmented command
  and may interact with a per-variant `after_help` if we add one. Severity:
  medium. Likelihood: medium. Mitigation: keep the env-var documentation in
  `long_about` (not `after_help`) so the existing ordering-caveat composition
  is undisturbed.
- Risk: the `clap_mangen` build script (see `crates/weaver-cli/build.rs`,
  consuming `cli.rs`) may render the new `long_about` differently from the
  terminal, producing a manpage section that is misformatted. Severity: low.
  Likelihood: low. Mitigation: include a smoke step that builds the crate
  (`cargo build -p weaver-cli`) and inspects the generated manpage if the
  build script writes one to `OUT_DIR`. If formatting issues appear, prefer
  `concat!()` of plain ASCII lines without leading whitespace beyond what
  clap's renderer will indent.
- Risk: the canonical user's-guide wording uses `WEAVER_FOREGROUND=1` while
  the daemon code accepts any non-empty value. Help text that says "set to
  any non-empty value" may surprise readers who only know the canonical
  example. Severity: low. Likelihood: medium. Mitigation: use the wording
  "When set to any non-empty value (for example `WEAVER_FOREGROUND=1`)" so
  both conventions are reconciled.
- Risk: another in-flight change touches `cli.rs` or the augmented help
  snapshot. Severity: low. Likelihood: low. Mitigation: `git pull --rebase`
  before starting and re-run the snapshot test to confirm the baseline before
  introducing the new content.

## Progress

- [ ] Stage A — Orientation and baseline (no code changes).
- [ ] Stage B — Failing tests and feature scenarios written first.
- [ ] Stage C — Implementation of `long_about` on `DaemonAction::Start`.
- [ ] Stage D — Snapshot regeneration, documentation updates, gateways.
- [ ] Branch renamed to `3-2-2-extend-daemon-start-help-with-config-guidance`
      and tracking `origin/3-2-2-extend-daemon-start-help-with-config-guidance`.
- [ ] Draft PR opened with title prefixed `(3.2.2)` and body referencing this
      ExecPlan and the roadmap entry.
- [ ] Roadmap entry `3.2.2` marked `[x]` in `docs/roadmap.md`.

## Surprises & discoveries

- (To be populated during implementation.)

## Decision log

- Decision: place the env-var documentation and example in `long_about` on
  `DaemonAction::Start`, not in `after_help`.
  Rationale: `crates/weaver-cli/src/help.rs::attach_ordering_caveat`
  recursively appends a cross-cutting ordering caveat to every subcommand's
  `after_help`. Adding `after_help` content on the `Start` variant would
  interleave the env-var docs with the caveat rather than keeping them with
  the description. The top-level `Cli` already places narrative content
  (Quick start, configuration ordering note) in `long_about` —
  `crates/weaver-cli/src/cli.rs:31-44`. Mirroring that placement keeps the
  help structure consistent.
  Date/Author: 2026-05-04, planning agent.

- Decision: use `concat!(...)` of `&'static str` literals rather than a single
  multi-line string with embedded escapes.
  Rationale: this is the established idiom in `cli.rs` (see lines 27–30,
  31–44, 45–59) and in the test harness (`tests/behaviour.rs:31-42`). It
  keeps line lengths reasonable, makes diffs readable, and avoids subtle
  whitespace issues with raw multi-line literals.
  Date/Author: 2026-05-04, planning agent.

- Decision: describe `WEAVER_FOREGROUND` as activated by "any non-empty value
  (for example `WEAVER_FOREGROUND=1`)".
  Rationale: the daemon implementation at
  `crates/weaverd/src/process/launch.rs:42` uses `env::var_os(...).is_some()`,
  so any non-empty value triggers foreground mode. The canonical user-guide
  examples use `=1`. Reconciling both keeps the help honest without
  contradicting existing wording.
  Date/Author: 2026-05-04, planning agent.

- Decision: cover env-var-set unhappy paths via `#[rstest]` unit tests in
  `crates/weaver-cli/src/tests/unit/help_output.rs`, not via new BDD steps.
  Rationale: the existing `TestWorld` does not expose environment mutation,
  and adding a `Given the environment variable {name} is {value}` step would
  introduce process-global state that races other tests in parallel
  execution. Unit tests can use `temp_env` (already an indirect dependency
  via the workspace) or scoped `std::env::set_var` within a serial test
  guarded by a mutex. Behavioural scenarios remain focused on what an
  operator types.
  Date/Author: 2026-05-04, planning agent.

- Decision: defer any change to `actionable_guidance.rs` wording.
  Rationale: 2.3.3 (already complete) standardised that wording and tests
  in `crates/weaver-cli/src/tests/unit/actionable_guidance.rs` and
  `crates/weaver-cli/src/tests/unit/auto_start.rs` assert specific phrases.
  This plan cross-references that wording for consistency but must not
  change it.
  Date/Author: 2026-05-04, planning agent.

## Outcomes & retrospective

- (To be filled in at completion.)

## Context and orientation

The Weaver CLI is defined in `crates/weaver-cli/src/cli.rs` using `clap`
derive macros. The top-level `Cli` struct sets `about`, `long_about`, and
`after_help` declaratively. The `daemon` subcommand is modelled by
`enum CliCommand::Daemon { action: DaemonAction }`, and `enum DaemonAction`
has three unit variants `Start`, `Stop`, `Status`, each with a single `///`
doc comment that becomes its short `about`. There is currently no
`long_about` or `after_help` on any `DaemonAction` variant.

A help-augmentation layer in `crates/weaver-cli/src/help.rs` builds an
augmented `clap::Command` on first use (see `build_command`), grafting the
shared configuration flags onto the rendered help and recursively appending
an "ordering caveat" to every subcommand's `after_help` via
`attach_ordering_caveat` (lines 133–140). This augmented command is what
backs `weaver --help` and `weaver daemon start --help` at runtime — see
`crates/weaver-cli/src/help.rs::write_help_for_args` and the snapshot tests
at `crates/weaver-cli/src/tests/unit/help_output.rs`.

The two environment variables this plan documents are:

- `WEAVERD_BIN` — read by `resolve_daemon_binary` at
  `crates/weaver-cli/src/lifecycle/spawning.rs:37-42`, falling back to the
  string `"weaverd"` (resolved against `PATH`).
- `WEAVER_FOREGROUND` — read by the daemon at
  `crates/weaverd/src/process/launch.rs:42` via
  `env::var_os(FOREGROUND_ENV_VAR).is_some()`. The constant is defined at
  `crates/weaverd/src/process/mod.rs:17`.

The behavioural test harness lives at
`crates/weaver-cli/src/tests/behaviour.rs` and is wired up to feature files
in `crates/weaver-cli/tests/features/`. The reusable BDD steps include
`When the operator runs {command}`, `Then stdout contains {snippet}`, and
`Then no daemon command was sent` (see `behaviour.rs` lines 248–254, 332–344,
291–297). Existing scenarios already exercise `daemon start --help` (see
`crates/weaver-cli/tests/features/weaver_cli.feature` lines 130–133), which
makes the new scenarios additive — no new step definitions are required for
the happy path.

The user's guide already documents both variables (`docs/users-guide.md`
lines 165–262), and the ui-gap-analysis explicitly recommends this work in
its Level 8 remedy (`docs/ui-gap-analysis.md` lines 376–393). The new help
text must echo, not contradict, the user's-guide wording.

Skills and references that should be consulted while implementing:

- `docs/rust-testing-with-rstest-fixtures.md` — fixture and parameterisation
  conventions for the `rstest` crate, applied throughout the unit tests.
- `docs/rstest-bdd-users-guide.md` — current `rstest-bdd` v0.5.0 step,
  scenario, and feature-file conventions.
- `docs/rust-doctest-dry-guide.md` — for any rustdoc examples we add to
  `cli.rs` or `help.rs`.
- `docs/reliable-testing-in-rust-via-dependency-injection.md` — patterns for
  isolating help rendering from configuration loading.
- `docs/complexity-antipatterns-and-refactoring-strategies.md` — baseline
  review heuristics; the change is intentionally minimal so no refactor is
  expected, but apply this lens before merging.
- `docs/ortho-config-users-guide.md` — covers how the augmented help is
  built from `OrthoConfigDocs` metadata; reference if the shared-flag block
  needs adjustment.
- `docs/weaver-design.md` — section discussing daemon lifecycle and
  environment.
- Skills: `rust-router` (always first for Rust changes), then the relevant
  language and domain skills it dispatches to. For this work the most
  useful follow-on skills are `domain-cli-and-daemons` (process lifecycle
  framing) and, if needed, `rust-types-and-apis` (only if `DaemonAction`
  needs structural change — it should not). The `execplans` skill governs
  this document.

## Plan of work

The work is divided into four short stages with explicit go/no-go points.

### Stage A — Orientation and baseline (no code changes)

Verify the baseline state before changing anything. Run `make check-fmt`,
`make lint`, and `make test` from a clean tree and note the elapsed time and
pass count. Inspect the current snapshot at
`crates/weaver-cli/src/tests/unit/snapshots/weaver_cli__tests__unit__help_output__daemon_start_augmented_help.snap`
to confirm it matches the file shown in this plan's "Purpose" section
(modulo the new `long_about` block, which is what we will add). Confirm that
`weaver daemon start --help`, when run from a debug build, produces a help
surface that contains the shared configuration flags but does not mention
either env var.

Go/no-go: do not proceed if the baseline gateways fail. Investigate and
escalate.

### Stage B — Failing tests and feature scenarios

Write the new tests first so they fail before implementation:

1. Extend
   `crates/weaver-cli/tests/features/weaver_cli.feature` with new
   scenarios in the existing "help surfaces" group, all using only the
   reusable steps:

   ```gherkin
   Scenario: Daemon start help documents the WEAVERD_BIN override
     When the operator runs "daemon start --help"
     Then stdout contains "WEAVERD_BIN"
     And stdout contains "weaverd binary"
     And no daemon command was sent

   Scenario: Daemon start help documents WEAVER_FOREGROUND
     When the operator runs "daemon start --help"
     Then stdout contains "WEAVER_FOREGROUND"
     And stdout contains "foreground"
     And no daemon command was sent

   Scenario: Daemon start help includes a startup example with an override
     When the operator runs "daemon start --help"
     Then stdout contains "WEAVERD_BIN="
     And stdout contains "weaver daemon start"
     And no daemon command was sent
   ```

   These scenarios are wired into the existing `weaver_cli_behaviour`
   `#[scenario(...)]` runner at
   `crates/weaver-cli/src/tests/behaviour.rs:375-376`. No new step
   definitions are required.

2. Extend
   `crates/weaver-cli/src/tests/unit/help_output.rs` with `#[rstest]`
   parameterised cases that assert on the rendered `daemon start` long
   help directly via `help::command()`. The new cases should:

   - Cover both env-var names appearing verbatim.
   - Cover the example line containing `WEAVERD_BIN=` and ending with
     `weaver daemon start`.
   - Re-assert that the ordering caveat is still appended (regression
     guard for the `attach_ordering_caveat` interaction).
   - Continue to use `PanickingLoader` so a regression that causes
     configuration loading during help rendering fails loudly.

3. Add a unit-level "edge case" test that sets `WEAVERD_BIN` and
   `WEAVER_FOREGROUND` in the test process's environment (using
   `temp_env::with_vars` from the `temp_env` crate, or a serial mutex
   guarding `std::env::set_var`/`remove_var`) and asserts that
   `daemon start --help` still renders the same content. This is the
   "unhappy" / edge-case path: help must be independent of host
   environment.

4. Re-run `cargo test -p weaver-cli` and confirm the new scenarios fail
   for the right reason ("stdout did not contain `WEAVERD_BIN`", etc.).
   This is the "red" step.

Go/no-go: do not proceed to Stage C until every new test fails for the
expected reason and existing tests still pass.

### Stage C — Implementation

Edit `crates/weaver-cli/src/cli.rs` only. Replace the unit `Start` variant of
`DaemonAction` with one that carries a `#[command(long_about = concat!(...))]`
attribute. Keep the existing `///` short description as the `about`. Do not
change `Stop` or `Status`. Sketch:

```rust
#[derive(Subcommand, Debug, Clone, Copy)]
pub(crate) enum DaemonAction {
    /// Starts the daemon and waits for readiness.
    #[command(long_about = concat!(
        "Starts the daemon and waits for readiness.\n",
        "\n",
        "Environment variables:\n",
        "  WEAVERD_BIN\n",
        "      Overrides the path to the weaverd binary that the CLI spawns.\n",
        "      When unset the CLI launches `weaverd` from PATH.\n",
        "\n",
        "  WEAVER_FOREGROUND\n",
        "      When set to any non-empty value (for example WEAVER_FOREGROUND=1),\n",
        "      runs the daemon in the foreground attached to the controlling\n",
        "      terminal so its startup output is visible. Useful for interactive\n",
        "      debugging and CI jobs.\n",
        "\n",
        "Examples:\n",
        "  Start the daemon using a custom binary path:\n",
        "    WEAVERD_BIN=/opt/weaver/bin/weaverd weaver daemon start\n",
        "\n",
        "  Start the daemon in the foreground for debugging:\n",
        "    WEAVER_FOREGROUND=1 weaver daemon start",
    ))]
    Start,
    /// Stops the daemon gracefully.
    Stop,
    /// Prints daemon health information.
    Status,
}
```

The exact wording is the responsibility of the implementer; the structural
requirements are encoded in the Stage B tests.

Run `cargo test -p weaver-cli` and confirm every new BDD scenario and unit
case turns green. The snapshot test at
`tests/unit/help_output.rs::daemon_start_help_snapshot_matches_augmented_command`
will fail with an `insta` diff — that is expected and handled in Stage D.

Go/no-go: do not proceed to Stage D unless the only remaining failure is the
snapshot mismatch on
`weaver_cli__tests__unit__help_output__daemon_start_augmented_help.snap`.

### Stage D — Snapshot, documentation, and gateways

1. Regenerate the snapshot:

   ```sh
   INSTA_UPDATE=auto cargo test -p weaver-cli \
     --test 'tests' \
     -- daemon_start_help_snapshot_matches_augmented_command
   ```

   Inspect the diff and confirm it contains only the new `long_about` block
   and no incidental changes. Commit the updated `.snap` alongside the
   source change in the same atomic commit.

2. Update `docs/users-guide.md`. The canonical env-var section already
   exists at lines 165–262; add a short paragraph in the lifecycle-commands
   section noting that `weaver daemon start --help` now lists these
   variables and includes a copy-paste example. Do not duplicate the
   variable definitions — link by name only.

3. Update `docs/developers-guide.md`. Add a brief note under the section
   that discusses the help-augmentation layer (or, if no such section
   exists, add a short subsection) describing the convention: per-variant
   `long_about` is the canonical place for environment-variable
   documentation on lifecycle subcommands, with `after_help` reserved for
   the cross-cutting ordering caveat.

4. Run, in this order, on a single shell with `tee`-captured logs:

   ```sh
   make fmt | tee /tmp/fmt-weaver-3-2-2-extend-daemon-start-help-with-config-guidance.out
   make markdownlint | tee /tmp/markdownlint-weaver-3-2-2-extend-daemon-start-help-with-config-guidance.out
   make nixie | tee /tmp/nixie-weaver-3-2-2-extend-daemon-start-help-with-config-guidance.out
   make check-fmt | tee /tmp/check-fmt-weaver-3-2-2-extend-daemon-start-help-with-config-guidance.out
   make lint | tee /tmp/lint-weaver-3-2-2-extend-daemon-start-help-with-config-guidance.out
   make test | tee /tmp/test-weaver-3-2-2-extend-daemon-start-help-with-config-guidance.out
   ```

   Address any warning or failure at its root rather than silencing it.

5. Mark roadmap item 3.2.2 as `[x]` in `docs/roadmap.md`.

6. Rename the working branch to
   `3-2-2-extend-daemon-start-help-with-config-guidance` (no PR exists
   yet, so a local rename + push is acceptable):

   ```sh
   git branch -m 3-2-2-extend-daemon-start-help-with-config-guidance
   git push -u origin 3-2-2-extend-daemon-start-help-with-config-guidance
   git push origin --delete feat/daemon-start-help-plan
   ```

   If a PR has already been opened against the old branch name by the time
   this stage runs, use GitHub's branch-rename flow instead so the PR
   follows the rename automatically; do not push a renamed local branch
   over a PR.

7. Open a draft PR with title prefixed `(3.2.2)` and a body that summarises
   the change, links this ExecPlan, and references roadmap item 3.2.2 and
   the Level 8 entry in `docs/ui-gap-analysis.md`.

Go/no-go: do not mark the roadmap item complete until every gateway passes
on the renamed branch and the draft PR is open.

## Concrete steps

The Stage D commands above are the canonical concrete steps. The only
additional discoverability commands worth recording for future readers are:

- Inspect the current rendered help at any time:

  ```sh
  cargo run -p weaver-cli --quiet -- daemon start --help
  ```

  Expected (after change): the rendered help contains both `WEAVERD_BIN` and
  `WEAVER_FOREGROUND`, at least one example line beginning `WEAVERD_BIN=`,
  and ends with the ordering caveat block.

- Re-run only the relevant unit tests during the red/green cycle:

  ```sh
  cargo test -p weaver-cli --lib tests::unit::help_output
  cargo test -p weaver-cli --lib tests::behaviour
  ```

## Validation and acceptance

Quality criteria — what "done" means:

- Tests:
  - Every new BDD scenario in
    `crates/weaver-cli/tests/features/weaver_cli.feature` passes.
  - Every new unit test in
    `crates/weaver-cli/src/tests/unit/help_output.rs` passes, including
    the env-var-set edge case.
  - The regenerated snapshot at
    `crates/weaver-cli/src/tests/unit/snapshots/weaver_cli__tests__unit__help_output__daemon_start_augmented_help.snap`
    diff is bounded to the new `long_about` block.
- Lint and format:
  - `make check-fmt` exits zero.
  - `make lint` exits zero with no clippy warnings.
  - `make markdownlint` exits zero on the modified Markdown files.
  - `make nixie` exits zero (no Mermaid changes are expected, but run as a
    safety check because docs are touched).
- Behavioural acceptance:
  - Running `weaver daemon start --help` from a release or debug build of
    `weaver-cli` shows both `WEAVERD_BIN` and `WEAVER_FOREGROUND` and at
    least one startup example using one of these overrides.
  - The runtime parser still rejects `weaver daemon start --log-filter
    debug` (where `--log-filter` appears after `start`) — this is the
    pre-existing behaviour, exercised by the ordering caveat tests in
    `crates/weaver-cli/src/tests/unit/help_output.rs`.

Quality method — how we check:

- Run the gateway commands listed in Stage D and review the
  `tee`-captured logs.
- Inspect the snapshot diff manually before committing.
- Open the draft PR and confirm CI is green before requesting review.

## Idempotence and recovery

All edits and tests are idempotent. The snapshot regeneration is the only
generative step; if it produces an unexpected diff, discard the change with
`git checkout -- crates/weaver-cli/src/tests/unit/snapshots/` and re-run
after fixing the source. Branch rename steps are safe to re-run only if no
PR is open; once a PR exists, follow GitHub's rename flow instead.

## Artifacts and notes

- The current `daemon start --help` snapshot baseline is in this repository
  at
  `crates/weaver-cli/src/tests/unit/snapshots/weaver_cli__tests__unit__help_output__daemon_start_augmented_help.snap`
  and is the natural diff target.
- The user's-guide canonical wording is at `docs/users-guide.md` lines
  165–262.
- The Level 8 remedy in the gap analysis is at
  `docs/ui-gap-analysis.md` lines 376–393.

## Interfaces and dependencies

No new public APIs. The change is contained to:

- `crates/weaver-cli/src/cli.rs` — add `#[command(long_about = ...)]`
  to `DaemonAction::Start`. No change to the enum's public shape, derived
  traits, or visibility.
- `crates/weaver-cli/src/tests/unit/help_output.rs` — add `#[rstest]`
  cases.
- `crates/weaver-cli/tests/features/weaver_cli.feature` — add scenarios.
- The `daemon_start_augmented_help.snap` snapshot under
  `crates/weaver-cli/src/tests/unit/snapshots/` — regenerated by `insta`.
- `docs/users-guide.md` — short cross-reference paragraph.
- `docs/developers-guide.md` — short note on the `long_about` convention
  for lifecycle subcommands.
- `docs/roadmap.md` — tick `3.2.2`.

No new crate dependencies. If the env-var-set unit test needs `temp_env`
and that crate is not already a workspace dev-dependency, prefer a simple
serial test using `std::env::set_var`/`remove_var` guarded by a
`std::sync::Mutex` rather than adding a new crate.
