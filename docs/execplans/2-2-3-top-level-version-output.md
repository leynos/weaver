# Add top-level version output and long-form command-line interface (CLI) description

This ExecPlan (execution plan) is a living document. The sections
`Constraints`, `Tolerances`, `Risks`, `Progress`, `Surprises & Discoveries`,
`Decision Log`, and `Outcomes & Retrospective` must be kept up to date as work
proceeds.

Status: COMPLETE

This document must be maintained in accordance with `AGENTS.md` at the
repository root.

## Purpose / big picture

After this change, operators can run `weaver --version` or `weaver -V` to see
the version string, exiting 0 and printing to standard output (stdout). Running
`weaver --help` displays a purpose statement and runnable quick-start examples
alongside the existing domain/operation catalogue. Both `--help` and
`--version` now exit 0 and write to stdout, matching standard CLI conventions.

Observable behaviour after this change:

- `weaver --version` prints `weaver 0.1.0` to stdout and exits 0.
- `weaver -V` prints identical output to `weaver --version`.
- `weaver --help` prints to stdout (not stderr), exits 0, and includes a
  "Quick start:" block with at least one runnable command example
  (`weaver observe get-definition`).
- `weaver` (bare invocation) continues to exit 1 and print to standard
  error (stderr) (unchanged).
- Running `make check-fmt && make lint && make test` passes with no
  regressions.

## Constraints

1. **400-line file limit.** No single source file may exceed 400 lines.
   `lib.rs` starts at 399 lines (1 line of headroom), requiring a preparatory
   extraction before adding the exit-code fix.
2. **build.rs dual-compilation.** `cli.rs` is included by `build.rs` via
   `#[path = "src/cli.rs"]` for manpage generation. Any code added to `cli.rs`
   must compile in the build script context. Clap derive attributes (`version`,
   `about`, `long_about`) are safe because they are processed by the derive
   macro, same as the existing `after_help`.
3. **Strict Clippy.** Over 30 denied lint categories including `unwrap_used`,
   `expect_used`, `indexing_slicing`, `string_slice`, `missing_docs`,
   `cognitive_complexity`, `allow_attributes`, and `str_to_string`. All code
   must pass
   `cargo clippy --workspace --all-targets --all-features -D warnings`.
4. **en-GB-oxendict spelling.** Comments and documentation use British English
   with Oxford "-ize" / "-yse" / "-our" spelling.
5. **rstest-bdd v0.5.0.** Behaviour-driven development (BDD) tests use
   v0.5.0. The fixture parameter must be named exactly `world`. Use
   `let _ = world;` to suppress unused warnings.
6. **`concat!()` for multi-line strings.** Per AGENTS.md, use `concat!()` to
   combine long string literals rather than escaping newlines with backslash.
7. **No new external dependencies.** The change uses only clap features already
   available.
8. **weaver-cli does not opt into workspace lints.** Its `Cargo.toml` has no
   `[lints]` section, so workspace-level `allow_attributes = "deny"` does not
   apply. This means `#[allow(dead_code)]` is acceptable if needed for the
   build.rs dual-compilation case.
9. **`str_to_string` denied.** Use `String::from()` or `.into()`, not
   `.to_string()` on `&str`.

## Tolerances (exception triggers)

- **Scope:** If implementation requires changes to more than 12 files or more
  than 200 net lines of code, stop and escalate.
- **Interface:** If a public application programming interface (API)
  signature must change beyond adding the clap attributes, stop and escalate.
- **Dependencies:** If a new external dependency is required, stop and
  escalate.
- **Iterations:** If tests still fail after 3 attempts at fixing, stop and
  escalate.
- **Line budget:** If any file cannot stay within 400 lines after the
  preparatory extraction, stop and escalate.

## Risks

- Risk: `lib.rs` line budget (399/400 lines). Adding the exit-code fix
  requires net new lines. Severity: high. Likelihood: certain. Mitigation:
  Extract the standalone `is_apply_patch()` function (7 lines) from `lib.rs` to
  `command.rs` as a method on `CommandInvocation` before adding the exit-code
  fix. This reclaims 7 lines, providing headroom for the +3 line exit-code
  change.

- Risk: Changing `--help` to stdout/exit-0 breaks existing integration test
  `help_output_lists_all_domains_and_operations` in `main_entry.rs`. Severity:
  medium. Likelihood: low. Mitigation: The existing test (lines 28-54)
  intentionally uses combined stdout+stderr output
  (`format!("{stdout}{stderr}")`) and does not assert on exit code. Its comment
  states, "The test intentionally avoids asserting on the exit code so this
  test remains valid if --help is later changed to exit 0." No change is needed.

- Risk: BDD tests break because `--help`/`--version` now behaves differently.
  Severity: low. Likelihood: low. Mitigation: No existing BDD scenario tests
  `--help` or `--version` directly. The "Bare invocation shows short help"
  scenario tests bare invocation (no args), which is handled by separate code
  (`write_bare_help` + `BareInvocation` error) and is unaffected.

- Risk: `unit.rs` line budget (397/400 lines). Adding `mod version_output`
  pushes it to 398. Severity: low. Likelihood: certain. Mitigation: One line is
  well within budget.

## Progress

- [x] (2026-03-07) Write ExecPlan to
  `docs/execplans/2-2-3-top-level-version-output.md`.
- [x] (2026-03-07) Stage A: Extract `is_apply_patch` from `lib.rs` to
  `command.rs`.
- [x] (2026-03-07) Stage B: Add `version`, `about`, `long_about` to
  `cli.rs`.
- [x] (2026-03-07) Stage C: Fix exit code for `--help`/`--version` in
  `lib.rs`.
- [x] (2026-03-07) Stage D: Add unit tests in
  `tests/unit/version_output.rs`.
- [x] (2026-03-07) Stage E: Add BDD feature file and register scenario.
- [x] (2026-03-07) Stage F: Add integration tests in
  `tests/main_entry.rs`.
- [x] (2026-03-07) Stage G: Update `docs/users-guide.md` and run
  `make markdownlint` plus `make fmt`.
- [x] (2026-03-07) Stage H: Mark roadmap 2.2.3 as done.
- [x] (2026-03-07) Run `make markdownlint`, `make fmt`, `make check-fmt`,
  `make lint`, and `make test`, and verify all pass.

## Surprises & discoveries

- Observation: `rustfmt` reformatted the `is_apply_patch` method body and
  some assertion macros in `version_output.rs` to more compact forms. Evidence:
  `make check-fmt` diff output. Impact: None — applied `cargo fmt --all` and
  re-verified.

## Decision log

- Decision: Use `clap::Error::use_stderr()` guard rather than matching
  `ErrorKind::DisplayHelp | ErrorKind::DisplayVersion` directly. Rationale:
  `use_stderr()` returns `false` exactly for informational outputs (help and
  version). It is idiomatic, avoids importing `ErrorKind`, is more compact
  (saves lines in the already-tight `lib.rs`), and automatically handles any
  future clap informational error kinds. Date: 2026-03-07.

- Decision: Extract `is_apply_patch` as a method on `CommandInvocation`
  rather than moving to a separate module. Rationale: The function operates
  solely on `CommandInvocation` fields and is a natural method. `command.rs` is
  at 91 lines and has ample headroom. Date: 2026-03-07.

- Decision: Place unit tests in a new `version_output.rs` file rather than
  adding to `bare_invocation.rs`. Rationale: The tests cover a distinct feature
  (version and help exit behaviour) that is thematically separate from
  bare-invocation help. Keeping them in a dedicated file aids discoverability
  and stays within the 400-line budget for both files. Date: 2026-03-07.

- Decision: Use `about` + `long_about` rather than only `long_about`.
  Rationale: `about` shows in short help (`-h`) and subcommand listings, giving
  users a purpose statement even in abbreviated output. `long_about` extends
  this with quick-start examples in `--help`. Both are standard clap
  attributes. Date: 2026-03-07.

## Outcomes & retrospective

All acceptance criteria are met:

1. `weaver --version` and `weaver -V` both exit 0 and emit the same
   version string (`weaver 0.1.0`) to stdout.
2. `weaver --help` exits 0, writes to stdout, and includes a "Quick start:"
   block with runnable examples including `weaver observe get-definition`.
3. `weaver` (bare invocation) continues to exit 1 and print to stderr.
4. `make check-fmt`, `make lint`, and `make test` all pass clean.

Line budget management was the key constraint. Extracting `is_apply_patch` to
`command.rs` as a preparatory refactoring reclaimed 7 lines in `lib.rs` (399 to
392), providing headroom for the +3 line exit-code fix (final: 395). All files
remain well within the 400-line limit.

The `clap::Error::use_stderr()` guard proved to be the right abstraction for
detecting informational clap errors — compact, idiomatic, and
forward-compatible.

## Context and orientation

The Weaver CLI is defined in `crates/weaver-cli/`. The main clap struct lives
in `src/cli.rs` and is parsed by `src/lib.rs`. The build script at `build.rs`
includes `cli.rs` via `#[path = "src/cli.rs"]` for manpage generation. This
dual-compilation means any code in `cli.rs` must compile in both contexts.

The workspace version is `0.1.0` (set in the workspace `Cargo.toml` at line
22). Clap's derive macro reads `CARGO_PKG_VERSION` automatically when the bare
`version` attribute is present in `#[command()]`.

Currently, `Cli::try_parse_from()` returns `Err(clap::Error)` for both `--help`
and `--version`. This error is wrapped in `AppError::CliUsage` and handled in
`run_with_handler`'s match block (lib.rs lines 202-209), which writes all
errors to stderr and returns `ExitCode::FAILURE`. The fix adds a guard that
checks `clap_err.use_stderr()` — when false (for help and version), it writes
to stdout and returns `ExitCode::SUCCESS`.

Key files and their current line counts:

- `crates/weaver-cli/src/cli.rs` — 87 lines (clap struct)
- `crates/weaver-cli/src/lib.rs` — 399 lines (runtime, at limit)
- `crates/weaver-cli/src/command.rs` — 91 lines (command types)
- `crates/weaver-cli/src/errors.rs` — 68 lines (error enum)
- `crates/weaver-cli/src/tests/unit.rs` — 397 lines (unit test index)
- `crates/weaver-cli/src/tests/behaviour.rs` — 340 lines (BDD steps)
- `crates/weaver-cli/tests/main_entry.rs` — 54 lines (integration tests)
- `crates/weaver-cli/tests/features/weaver_cli.feature` — 87 lines

## Plan of work

### Stage A: Preparatory extraction (separate refactoring commit)

Move the standalone `is_apply_patch()` function from `lib.rs` to `command.rs`
as a method on `CommandInvocation`. This reclaims 7 lines in `lib.rs`.

**`crates/weaver-cli/src/command.rs`** — Add an `impl CommandInvocation` block:

```rust
impl CommandInvocation {
    /// Returns true when this invocation targets the `act apply-patch`
    /// operation.
    pub(crate) fn is_apply_patch(&self) -> bool {
        self.domain.eq_ignore_ascii_case("act")
            && self
                .operation
                .eq_ignore_ascii_case("apply-patch")
    }
}
```

**`crates/weaver-cli/src/lib.rs`** — Remove the standalone function
`is_apply_patch` (lines 343-349). Update the call site in `build_request` (line
329) from `is_apply_patch(&invocation)` to `invocation.is_apply_patch()`.

Net effect on `lib.rs`: -7 lines (399 to 392).

Validation: `make check-fmt && make lint && make test`.

### Stage B: Add `version`, `about`, and `long_about` to `cli.rs`

**`crates/weaver-cli/src/cli.rs`** — In the `#[command()]` attribute block
(lines 22-40), add three new attributes after `name = "weaver"`:

```rust
#[command(
    name = "weaver",
    version,
    disable_help_subcommand = true,
    subcommand_negates_reqs = true,
    about = concat!(
        "Semantic code intelligence tool for observing, ",
        "acting on, and verifying code",
    ),
    long_about = concat!(
        "Semantic code intelligence tool for observing, ",
        "acting on, and verifying code.\n",
        "\n",
        "Quick start:\n",
        "\n",
        "  weaver observe get-definition \\\n",
        "    --uri file:///src/main.rs --position 10:5\n",
        "  weaver act apply-patch < changes.patch\n",
        "  weaver daemon status\n",
        "\n",
        "Configuration flags such as --config-path and --daemon-socket\n",
        "must appear before the command domain.",
    ),
    after_help = concat!(
        // ... existing after_help content unchanged ...
    )
)]
```

The bare `version` attribute causes clap to read `CARGO_PKG_VERSION` (`0.1.0`).
The `about` shows in `-h` and subcommand listings. The `long_about` shows in
`--help` and includes the required quick-start examples.

Line count: `cli.rs` goes from 87 to approximately 102 lines.

### Stage C: Fix exit code for `--help` and `--version` in `lib.rs`

**`crates/weaver-cli/src/lib.rs`** — Replace the `match result` block (lines
202-209) in `run_with_handler`. Add a new arm before the catch-all `Err(error)`:

```rust
match result {
    Ok(exit_code) => exit_code,
    Err(AppError::BareInvocation) => ExitCode::FAILURE,
    Err(AppError::CliUsage(ref clap_err)) if !clap_err.use_stderr() => {
        let _ = write!(self.io.stdout, "{clap_err}");
        ExitCode::SUCCESS
    }
    Err(error) => {
        let _ = writeln!(self.io.stderr, "{error}");
        ExitCode::FAILURE
    }
}
```

The guard `!clap_err.use_stderr()` is `true` for `DisplayHelp` and
`DisplayVersion`. The output is written to stdout (not stderr) and the function
returns `ExitCode::SUCCESS`.

Line count: +3 lines net. After Stage A, `lib.rs` is at 392, so result is
approximately 395.

### Stage D: Unit tests

**New file: `crates/weaver-cli/src/tests/unit/version_output.rs`**

Tests using the same `PanickingLoader` pattern from `bare_invocation.rs` to
prove that version and help output short-circuit before configuration loading:

1. `version_long_flag_exits_with_success` — `--version` returns
   `ExitCode::SUCCESS`.
2. `version_short_flag_exits_with_success` — `-V` returns
   `ExitCode::SUCCESS`.
3. `version_output_goes_to_stdout` — stdout contains "weaver", stderr is
   empty.
4. `version_output_contains_version_number` — stdout contains
   `env!("CARGO_PKG_VERSION")`.
5. `version_long_and_short_produce_identical_output` — `--version` and `-V`
   yield the same stdout.
6. `help_flag_exits_with_success` — `--help` returns `ExitCode::SUCCESS`.
7. `help_output_goes_to_stdout` — stdout contains "Usage:", stderr is empty.
8. `help_output_contains_quick_start_example` — stdout contains
   "Quick start:" and "weaver observe get-definition".

**`crates/weaver-cli/src/tests/unit.rs`** — Add `mod version_output;` (line
398).

### Stage E: BDD tests

**New file: `crates/weaver-cli/tests/features/weaver_cli_version.feature`**

```gherkin
Feature: Weaver CLI version output

  Scenario: Version flag outputs version and exits successfully
    When the operator runs "--version"
    Then stdout contains "weaver"
    And the CLI exits with code 0

  Scenario: Short version flag outputs version and exits successfully
    When the operator runs "-V"
    Then stdout contains "weaver"
    And the CLI exits with code 0

  Scenario: Version flag produces no stderr output
    When the operator runs "--version"
    Then stderr is ""
    And the CLI exits with code 0

  Scenario: Help flag includes quick-start example
    When the operator runs "--help"
    Then stdout contains "Quick start:"
    And stdout contains "weaver observe get-definition"
    And the CLI exits with code 0
```

All step definitions already exist in `behaviour.rs`. No new steps needed.

**`crates/weaver-cli/src/tests/behaviour.rs`** — Add scenario registration:

```rust
#[scenario(path = "tests/features/weaver_cli_version.feature")]
fn weaver_cli_version_behaviour(world: RefCell<TestWorld>) {
    let _ = world;
}
```

### Stage F: Integration tests

**`crates/weaver-cli/tests/main_entry.rs`** — Add binary-level assertions:

```rust
#[test]
fn version_flag_exits_successfully() {
    let mut command = cargo_bin_cmd!("weaver");
    command.arg("--version");
    command
        .assert()
        .success()
        .stdout(contains("weaver"))
        .stdout(contains(env!("CARGO_PKG_VERSION")));
}

#[test]
fn short_version_flag_exits_successfully() {
    let mut command = cargo_bin_cmd!("weaver");
    command.arg("-V");
    command.assert().success().stdout(contains("weaver"));
}

#[test]
fn help_flag_exits_successfully_with_quick_start() {
    let mut command = cargo_bin_cmd!("weaver");
    command.arg("--help");
    command
        .assert()
        .success()
        .stdout(contains("Quick start:"))
        .stdout(contains("weaver observe get-definition"));
}
```

### Stage G: Documentation updates and Markdown quality gates

**`docs/users-guide.md`** — Insert a "Version" subsection between "Bare
invocation" (line 243) and "Top-level help" (line 245).

Update the "Top-level help" section to mention exit code 0 and the quick-start
block. Run `make markdownlint` and `make fmt` after the documentation edits so
the guide and plan changes are validated and normalized before the Rust gates
run.

### Stage H: Mark roadmap done

**`docs/roadmap.md`** — Change lines 168-178: replace `[ ]` with `[x]` on all
four items (the parent 2.2.3 and its three sub-items).

## Concrete steps

All commands run from workspace root `/home/user/project`.

Stage A (refactoring commit):

```bash
# After making edits:
make markdownlint && make fmt && make check-fmt && make lint && set -o pipefail \
  && make test 2>&1 | tee /tmp/2-2-3-test-a.log
```

Stage B-H (feature commit):

```bash
# After making all edits:
make markdownlint && make fmt && make check-fmt && make lint && set -o pipefail \
  && make test 2>&1 | tee /tmp/2-2-3-test-b.log
```

Manual smoke test:

```sh
cargo run -p weaver-cli -- --version
# Expected: weaver 0.1.0

cargo run -p weaver-cli -- -V
# Expected: weaver 0.1.0

cargo run -p weaver-cli -- --help
# Expected: includes "Quick start:" and "weaver observe get-definition"
```

## Validation and acceptance

After both commits, the following must hold:

1. `weaver --version` prints `weaver 0.1.0` to stdout and exits 0.
2. `weaver -V` prints identical output to `weaver --version`.
3. `weaver --help` prints to stdout (not stderr), exits 0, and includes
   "Quick start:" with at least one runnable example.
4. `weaver` (bare invocation) still exits 1 and prints to stderr
   (unchanged).
5. `make check-fmt` passes.
6. `make lint` passes.
7. `make test` passes (including new unit, BDD, and integration tests).

Quality criteria:

- Tests: all workspace tests pass via `make test`.
- Lint: `make lint` clean (zero warnings).
- Format: `make check-fmt` clean.

Quality method:

```bash
make check-fmt && make lint && set -o pipefail \
  && make test 2>&1 | tee /tmp/2-2-3-final.log
```

## Idempotence and recovery

All steps are idempotent. If a step fails partway through, re-running the
quality gate commands after fixing will verify correctness. The preparatory
refactoring commit is behaviour-preserving and can be reverted independently if
needed.

## Interfaces and dependencies

No new external dependencies. No new public API surfaces. The only interface
change is the addition of clap-standard `--version`/`-V` flags and `about`/
`long_about` text to the `Cli` struct's derive macro attributes.

Existing reusable code:

- `PanickingLoader` pattern from
  `crates/weaver-cli/src/tests/unit/bare_invocation.rs` — reused for the new
  version output unit tests.
- BDD step definitions in
  `crates/weaver-cli/src/tests/behaviour.rs` — all steps needed by the new
  feature file already exist.
- `run_with_loader()` from `crates/weaver-cli/src/lib.rs:353` — used by
  unit tests to exercise the CLI without a real binary.

## File change summary

| File                                        | Change                                     | Lines before | Lines after |
| ------------------------------------------- | ------------------------------------------ | ------------ | ----------- |
| `src/command.rs`                            | Add `is_apply_patch` method                | 91           | ~98         |
| `src/lib.rs`                                | Remove `is_apply_patch`, add exit-code fix | 399          | ~395        |
| `src/cli.rs`                                | Add `version`, `about`, `long_about`       | 87           | ~102        |
| `src/tests/unit.rs`                         | Add `mod version_output`                   | 397          | 398         |
| `src/tests/unit/version_output.rs`          | New file                                   | 0            | ~100        |
| `src/tests/behaviour.rs`                    | Add scenario registration                  | 340          | ~344        |
| `tests/features/weaver_cli_version.feature` | New file                                   | 0            | ~20         |
| `tests/main_entry.rs`                       | Add version/help integration tests         | 54           | ~79         |
| `docs/users-guide.md`                       | Add version section, update help section   | ~999         | ~1015       |
| `docs/roadmap.md`                           | Mark 2.2.3 done                            | ---          | ---         |

All paths are relative to `crates/weaver-cli/` except `docs/` which is relative
to workspace root.

## Commit sequence

1. **Refactoring commit:** "Extract is_apply_patch to CommandInvocation
   method"
   - Moves function from `lib.rs` to `command.rs`.
   - No behaviour change.
   - Must pass all quality gates.

2. **Feature commit:** "Add --version/-V support and long_about quick-start
   block"
   - `cli.rs`: `version`, `about`, `long_about` attributes.
   - `lib.rs`: exit-code fix for informational clap errors.
   - New unit tests, BDD tests, integration tests.
   - Documentation and roadmap updates.
   - Must pass all quality gates.
