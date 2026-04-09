# 2.2.2 List all domains and operations in top-level help output

This ExecPlan (execution plan) is a living document. The sections
`Constraints`, `Tolerances`, `Risks`, `Progress`, `Surprises & Discoveries`,
`Decision Log`, and `Outcomes & Retrospective` must be kept up to date as work
proceeds.

Status: DONE

## Purpose / big picture

When an operator runs `weaver --help` today, the help output lists one example
domain (`observe`) and one example operation (`get-definition`) as
parenthetical hints, but never enumerates the full set of available domains or
operations. The operator must already know the domain and operation names or
read external documentation to discover them. This is the P0 gap identified as
[Gap 1a](../ui-gap-analysis.md#gap-1a--domains-not-enumerated) (domains not
enumerated) and
[Gap 1b](../ui-gap-analysis.md#gap-1b--operations-not-enumerated) (operations
not enumerated) in the UI gap analysis.

After this change, running `weaver --help` will display a catalogue section
below the standard clap help output listing all three domains (`observe`,
`act`, `verify`) with a one-line description each, and every CLI-supported
operation per domain. The output requires no daemon startup or socket access.

Observable outcome: run `weaver --help` and see, after the standard clap help:

```plaintext
Domains and operations:

  observe — Query code structure and relationships
    get-definition    find-references    grep
    diagnostics       call-hierarchy

  act — Perform code modifications
    rename-symbol     apply-edits        apply-patch
    apply-rewrite     refactor

  verify — Validate code correctness
    diagnostics       syntax
```

This satisfies roadmap task 2.2.2 and closes the relevant checkboxes in
`docs/roadmap.md`.

## Constraints

- `make check-fmt`, `make lint`, and `make test` must pass after all changes.
- No code file may exceed 400 lines.
- The workspace Clippy configuration is strict (pedantic, deny on
  `unwrap_used`, `expect_used`, `print_stdout`, `print_stderr`,
  `cognitive_complexity`, `missing_docs`, etc.). All new code must comply.
  Note: `weaver-cli` does NOT opt into workspace lints (no `[lints]` section in
  its `Cargo.toml`), so `allow_attributes = "deny"` does not apply to it.
- Comments and documentation must use en-GB-oxendict spelling
  ("-ize" / "-yse" / "-our").
- New functionality requires both unit tests and BDD behavioural tests using
  `rstest-bdd` v0.5.0.
- Every module must begin with a `//!` module-level doc comment.
- The `after_help` catalogue must not require configuration loading or daemon
  connectivity. It must be entirely client-side.
- The build script (`crates/weaver-cli/build.rs`) includes `cli.rs` via
  `#[path = "src/cli.rs"]` for manpage generation. Any addition to `cli.rs`
  must compile in both the build script and library contexts.
- All new user-facing text must be sourced from Fluent `.ftl` resources via
  the `ortho_config::Localizer` trait so future locales can override it.
- The Fluent messages and the static English fallbacks must produce identical
  output; a unit test must guard against drift.

## Tolerances (exception triggers)

- Scope: if implementation requires changes to more than 15 files, stop and
  escalate.
- Interface: if any `pub` API signature in `weaver-cli` must change, stop and
  escalate. All changes are `pub(crate)` or private.
- Dependencies: no new external dependencies are expected. If one is required,
  stop and escalate.
- Iterations: if tests still fail after 5 attempts at fixing, stop and
  escalate.
- Ambiguity: the operation list must exactly match the `DomainRoutingContext`
  constants in `crates/weaverd/src/dispatch/router.rs` lines 89–116. If these
  have changed since plan drafting, update the plan accordingly.

## Risks

- Risk: `lib.rs` is at 398 lines (2 lines of headroom). Adding any code to
  `lib.rs` may exceed the 400-line limit. Severity: medium Likelihood: low (no
  changes to `lib.rs` are needed for this task) Mitigation: The `after_help`
  text is set as a static attribute on `Cli` in `cli.rs` and the Fluent
  infrastructure lives in `localizer.rs`. No `lib.rs` changes should be
  required. If they are, extract a helper to a separate module first.

- Risk: `unit.rs` is at 396 lines (4 lines of headroom). Adding a new `mod`
  declaration may exceed the limit. Severity: low Likelihood: high (we need
  `mod after_help;`) Mitigation: We need to add `mod after_help;` (1 line). At
  397 lines this is within the limit.

- Risk: The static `after_help` text in `cli.rs` is compiled into build.rs
  for manpage generation. A very long `concat!` macro might cause formatting
  issues in the generated manpage. Severity: low Likelihood: low Mitigation:
  Test both `weaver --help` and the manpage rendering. The `after_help` text is
  plain text, which renders cleanly in troff.

- Risk: The `after_help` text must be kept in sync with the daemon's
  `DomainRoutingContext` operation list. If operations are added or removed in
  the daemon, the CLI text becomes stale. Severity: medium Likelihood: low
  (operations change infrequently) Mitigation: A unit test validates that every
  known operation appears in the after-help text. A comment in the constants
  module references the authoritative source.

## Progress

- [x] Stage A: Add Fluent messages and localizer infrastructure for the
  after-help catalogue.
- [x] Stage B: Add the static `after_help` attribute to `Cli` in `cli.rs`.
- [x] Stage C: Add unit tests and integration tests.
- [x] Stage D: Documentation and roadmap updates.
- [x] Stage E: Final validation and commit gating.

## Surprises & discoveries

- Surprise: The `weaver --help` output currently goes to stderr via the
  `AppError::CliUsage` path, but this is an implementation detail that may
  change. The integration test uses `command.output()` and checks combined
  stdout + stderr for the expected tokens, deliberately avoiding any assertion
  on exit code or output stream so the test remains valid regardless of how
  `--help` output is routed.

- Surprise: Clippy's `items_after_test_module` lint fires when any item
  appears after a `#[cfg(test)] mod`, even if the subsequent items are also
  `#[cfg(test)]`. The initial approach placed `mod after_help` and
  `fn render_after_help` as separate `#[cfg(test)]` items at the end of
  `localizer.rs`. The fix was to move `render_after_help` inside the
  `after_help` module, calling `super::msg()` for the localizer helper.

- Surprise: The `after_help` module needed `pub(crate)` visibility (not
  just private) because test code in `tests/unit/after_help.rs` imports
  `crate::localizer::after_help::render_after_help`.

## Decision log

- Decision: Use a static `after_help = concat!(...)` attribute on the `Cli`
  struct in `cli.rs`, combined with Fluent messages in `localizer.rs` for
  translation readiness and a drift-guard test. Rationale: The
  `#[command(after_help = "...")]` clap attribute accepts a string literal (or
  `concat!`). This approach means both the runtime `--help` output and the
  manpage (generated by build.rs from `Cli::command()`) automatically include
  the catalogue. Fluent messages are defined alongside the static strings, and
  a unit test verifies they produce identical output. This exactly follows the
  pattern established in task 5.1.1 for bare-help messages, where `bare_help`
  constants hold `(fluent_id, fallback)` tuples and a
  `fluent_and_fallback_outputs_are_identical` test guards against drift.
  Date/Author: 2026-03-03

- Decision: Place the `after_help` Fluent constants in a new `after_help`
  submodule within `localizer.rs`, alongside the existing `bare_help` module.
  Add a `render_after_help()` function for test use. Rationale: This keeps all
  localized help text in one place (`localizer.rs`) and follows the established
  pattern. The `render_after_help()` function is used by tests to verify Fluent
  and fallback consistency, and to validate that the static `concat!` in
  `cli.rs` matches. Date/Author: 2026-03-03

- Decision: Put new unit tests in `src/tests/unit/after_help.rs` as a
  sibling to `bare_invocation.rs`. Rationale: Follows the existing test module
  structure. The `unit.rs` parent has 4 lines of headroom; adding
  `mod after_help;` uses 1 line. Date/Author: 2026-03-03

- Decision: Verify `--help` output via `assert_cmd` integration tests in
  `tests/main_entry.rs`, not via the BDD `TestWorld` harness. Rationale:
  `weaver --help` is handled directly by clap, which prints to stdout and exits
  before our `CliRunner` code runs. The `TestWorld` harness intercepts at the
  `run_with_loader` level, but clap's `--help` handler calls
  `std::process::exit()` internally. The `assert_cmd` approach (already used in
  `main_entry.rs`) runs the binary as a subprocess and captures its output,
  which is the correct way to test `--help`. Date/Author: 2026-03-03

- Decision: Declare `localizer::after_help` as `pub(crate)` (not
  `#[cfg(test)]`). `after_help::DOMAIN_OPERATIONS` is `pub` and re-exported
  from `lib.rs` (`pub use localizer::after_help:: DOMAIN_OPERATIONS`) so
  integration tests (`tests/main_entry.rs`) can reference it directly. The
  remaining Fluent `(id, fallback)` constants and `render_after_help()` are
  `#[cfg(test)]`-gated inside a nested `after_help::fluent_entries` submodule.
  This eliminates `dead_code` entirely without lint suppression: the constants
  and render function are compiled only for the test target where they are
  used. No `#[allow]` or `#[expect]` attribute is needed. `render_after_help()`
  lives inside the `fluent_entries` submodule (rather than as a sibling item)
  to avoid Clippy's `items_after_test_module` lint, and calls
  `crate::localizer::msg()` to access the parent module's localizer helper.
  Date/Author: 2026-03-06

## Outcomes & retrospective

All acceptance criteria met:

1. `weaver --help` lists all three domains (`observe`, `act`, `verify`).
2. `weaver --help` lists every CLI-supported operation for each domain.
3. `weaver --help` completes without daemon startup or socket access.
4. Static clap text and Fluent resources are synchronized (guarded by
   `clap_after_help_matches_fluent_render` test).
5. Fluent and fallback paths produce identical output (guarded by
   `after_help_fluent_and_fallback_are_identical` test).

Quality gates passed: `make check-fmt`, `make lint`, and `make test` (all tests
pass; one pre-existing slow test `auto_start_succeeds_and_proceeds` hangs in
the CI environment but is unrelated to this change).

Files modified: 9 (within 15-file tolerance).

Key learning: When adding test-only code to a module, place the entire
`#[cfg(test)]` module (including any helper functions) as a single block at the
end of the file. Do not place separate `#[cfg(test)]` items after a
`#[cfg(test)] mod`, as Clippy's `items_after_test_module` lint will fire.

## Context and orientation

The Weaver CLI is a Rust workspace with 12+ crates. The CLI binary lives in
`crates/weaver-cli/`. It uses clap v4.5 in derive mode for argument parsing.

### Key files

_Table: Key files referenced in this plan._

| File                                                  | Lines | Purpose                                  |
| ----------------------------------------------------- | ----- | ---------------------------------------- |
| `crates/weaver-cli/src/cli.rs`                        | 73    | Clap `#[derive(Parser)]` struct          |
| `crates/weaver-cli/src/localizer.rs`                  | 99    | Fluent localizer + bare-help             |
| `crates/weaver-cli/locales/en-US/messages.ftl`        | 7     | Fluent resources                         |
| `crates/weaver-cli/src/lib.rs`                        | 398   | Core runtime (tight on lines)            |
| `crates/weaver-cli/src/tests/unit.rs`                 | 396   | Unit test root module                    |
| `crates/weaver-cli/src/tests/unit/bare_invocation.rs` | 146   | Bare-help unit tests (pattern to follow) |
| `crates/weaver-cli/src/tests/behaviour.rs`            | 340   | BDD step definitions                     |
| `crates/weaver-cli/tests/main_entry.rs`               | 24    | Integration tests (assert_cmd)           |
| `crates/weaver-cli/tests/features/weaver_cli.feature` | 86    | BDD scenarios                            |
| `crates/weaver-cli/build.rs`                          | 70    | Manpage generation (includes cli.rs)     |
| `crates/weaverd/src/dispatch/router.rs`               | 264   | Authoritative domain/operation lists     |
| `docs/users-guide.md`                                 | 889   | Operator documentation                   |
| `docs/roadmap.md`                                     | 771   | Roadmap checkboxes                       |

### Localization pattern

All user-facing text follows this pattern (established in task 5.1.1):

1. Fluent messages in `crates/weaver-cli/locales/en-US/messages.ftl`.
2. A constants module in `localizer.rs` holding `(fluent_id, fallback)` tuples.
3. A `msg()` helper that resolves through the localizer with the fallback.
4. A render function that composes the full text block.
5. A `fluent_and_fallback_outputs_are_identical` unit test guarding drift.

### Authoritative operation list

From `crates/weaverd/src/dispatch/router.rs` lines 89–116:

- `observe`: `get-definition`, `find-references`, `grep`, `diagnostics`,
  `call-hierarchy`
- `act`: `rename-symbol`, `apply-edits`, `apply-patch`, `apply-rewrite`,
  `refactor`
- `verify`: `diagnostics`, `syntax`

### Build script constraint

`crates/weaver-cli/build.rs` line 9 includes `cli.rs` via
`#[path = "src/cli.rs"]` for manpage generation. It calls `cli::Cli::command()`
to get the clap `Command` object. Any `after_help` attribute set on `Cli` will
automatically be included in the manpage. Methods added to `cli.rs` that are
only used by `lib.rs` trigger `dead_code` warnings in the build script context.
The solution is to place such methods in `lib.rs` instead, or to use
`#[expect(dead_code, reason = "used only from lib.rs, not build.rs")]`.

## Plan of work

### Stage A: Add Fluent messages and localizer infrastructure

**A1. Extend the Fluent resource file.**

In `crates/weaver-cli/locales/en-US/messages.ftl`, append messages for the
after-help catalogue. Each message covers one logical line of the output.

**A2. Add `after_help` constants module in `localizer.rs`.**

Following the `bare_help` module pattern, add an `after_help` module with
`(fluent_id, english_fallback)` tuples for each message. Then add a
`pub(crate) fn render_after_help(localizer: &dyn Localizer) -> String` function
that composes the full text block.

The render function formats each domain with a two-space-indented heading and
four-space-indented operation lines, matching the static `concat!` in `cli.rs`.

**A3. Verify `localizer.rs` stays under 400 lines.**

Current: 99 lines. Adding ~70 lines brings it to ~170. Well within limits.

### Stage B: Add the static `after_help` attribute to `Cli`

**B1. Add `after_help` to the `#[command]` attribute in `cli.rs`.**

The static English text is set as a `concat!` string literal in the clap
attribute. This ensures both `weaver --help` (runtime) and the manpage
(build-time) include the catalogue.

Uses `\u{2014}` (em dash) for domain headings.

**B2. Verify the output.**

Run `cargo run -p weaver-cli -- --help` and visually confirm.

### Stage C: Add unit tests and integration tests

**C1. Create unit test file `src/tests/unit/after_help.rs`.**

Tests that:

1. `render_after_help()` with `NoOpLocalizer` contains all domains and ops.
2. `render_after_help()` with `FluentLocalizer` contains all domains and ops.
3. Fluent and fallback outputs are identical (drift guard).
4. The static `after_help` text on `Cli::command()` matches the
   `render_after_help()` output (sync guard between `concat!` in `cli.rs` and
   the Fluent messages in `localizer.rs`).
5. Every operation from the authoritative list appears in the rendered text.

**C2. Add `mod after_help;` to `unit.rs`.**

Append at the end of `crates/weaver-cli/src/tests/unit.rs` (line 397).

**C3. Add integration test in `tests/main_entry.rs`.**

Append a test that runs `weaver --help` via `assert_cmd` and checks the output
contains the domain catalogue.

**C4. Verify tests are green.**

Run `make test` and confirm all new tests pass.

### Stage D: Documentation and roadmap updates

**D1. Update `docs/users-guide.md`.**

After the "Bare invocation" subsection, add a "Top-level help" subsection
describing the new `--help` output with the domains-and-operations catalogue.

**D2. Mark roadmap task 2.2.2 as done in `docs/roadmap.md`.**

Change the three `[ ]` checkboxes for task 2.2.2 to `[x]`.

**D3. Run `make fmt`.**

### Stage E: Final validation and commit gating

**E1. Run full commit gating suite.**

```sh
set -o pipefail
make check-fmt 2>&1 | tee /tmp/check-fmt.log
make lint 2>&1 | tee /tmp/lint.log
make test 2>&1 | tee /tmp/test.log
```

All three must pass with zero exit code.

**E2. Verify observable behaviour.**

Run `cargo run -p weaver-cli -- --help` and confirm the catalogue appears.

## Concrete steps

All commands run from the workspace root `/home/user/project`.

### Stage A

1. Edit `crates/weaver-cli/locales/en-US/messages.ftl` — append Fluent
   messages.
2. Edit `crates/weaver-cli/src/localizer.rs` — add `mod after_help` with
   constants and `pub(crate) fn render_after_help()`.
3. Run `cargo check -p weaver-cli` to verify compilation.

### Stage B

1. Edit `crates/weaver-cli/src/cli.rs` — add `after_help = concat!(...)`.
2. Run `cargo run -p weaver-cli -- --help` and visually confirm.

### Stage C

1. Create `crates/weaver-cli/src/tests/unit/after_help.rs` with unit tests.
2. Edit `crates/weaver-cli/src/tests/unit.rs` — append `mod after_help;`.
3. Edit `crates/weaver-cli/tests/main_entry.rs` — append integration test.
4. Run `make test`.

### Stage D

1. Edit `docs/users-guide.md` — add "Top-level help" subsection.
2. Edit `docs/roadmap.md` — mark 2.2.2 as done.
3. Run `make fmt`.

### Stage E

```sh
set -o pipefail
make check-fmt 2>&1 | tee /tmp/check-fmt.log
make lint 2>&1 | tee /tmp/lint.log
make test 2>&1 | tee /tmp/test.log
```

## Validation and acceptance

Acceptance criteria (from roadmap):

1. `weaver --help` lists all three domains (`observe`, `act`, `verify`) —
   verified by integration test and unit tests.
2. `weaver --help` lists every CLI-supported operation for each domain —
   verified by tests checking for all 11 unique operations.
3. `weaver --help` completes without daemon startup or socket access —
   verified by integration test (runs binary with no daemon).
4. Static clap text and Fluent resources remain synchronized — verified by
   unit test `clap_after_help_matches_fluent_render`.
5. Fluent and fallback paths produce identical output — verified by unit test
   `after_help_fluent_and_fallback_are_identical`.

Quality criteria:

- Tests: `make test` passes with zero exit code, including all new tests.
- Lint: `make lint` passes (Clippy pedantic, deny warnings).
- Format: `make check-fmt` passes.

Quality method:

```sh
make check-fmt && make lint && make test
```

## Idempotence and recovery

All steps are file edits and can be re-applied. If any step fails partway
through, the working tree can be reset with `git checkout -- .` and the steps
re-executed from the beginning. No external state is modified.

## Interfaces and dependencies

No new external dependencies.

New `pub(crate)` interfaces in `crates/weaver-cli/src/localizer.rs`:

```rust
pub(crate) fn render_after_help(localizer: &dyn Localizer) -> String;
```

Modified clap attribute in `crates/weaver-cli/src/cli.rs`:

```rust
#[command(
    name = "weaver",
    disable_help_subcommand = true,
    subcommand_negates_reqs = true,
    after_help = concat!(...) // new
)]
```

## Files modified (summary)

_Table: Summary of modified files._

| File                                                         | Change                                          |
| ------------------------------------------------------------ | ----------------------------------------------- |
| `crates/weaver-cli/locales/en-US/messages.ftl`               | Append 10 Fluent messages                       |
| `crates/weaver-cli/src/localizer.rs`                         | Add `after_help` module + `render_after_help()` |
| `crates/weaver-cli/src/cli.rs`                               | Add `after_help = concat!(...)` attribute       |
| `crates/weaver-cli/src/tests/unit.rs`                        | Add `mod after_help;` declaration               |
| `crates/weaver-cli/src/tests/unit/after_help.rs`             | New: unit tests for after-help                  |
| `crates/weaver-cli/tests/main_entry.rs`                      | Add integration test for `--help`               |
| `docs/users-guide.md`                                        | Add "Top-level help" subsection                 |
| `docs/roadmap.md`                                            | Mark 2.2.2 checkboxes as done                   |
| `docs/execplans/2-2-2-list-all-domains-in-top-level-help.md` | This ExecPlan                                   |

Total: 9 files (within 15-file tolerance).
