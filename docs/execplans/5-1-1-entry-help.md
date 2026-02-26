# 5.1.1 Show short help on bare invocation (Fluent-localised)

This ExecPlan (execution plan) is a living document. The sections
`Constraints`, `Tolerances`, `Risks`, `Progress`, `Surprises & Discoveries`,
`Decision Log`, and `Outcomes & Retrospective` must be kept up to date as work
proceeds.

Status: COMPLETE

## Purpose / big picture

When an operator runs `weaver` with no arguments today, the CLI prints the
terse message `the command domain must be provided` to stderr and exits 1.
There is no hint about what domains exist, no usage line, and no pointer to
`--help`. A newcomer who has just installed Weaver has no idea what to type
next.

After this change, running `weaver` with no arguments will print a short,
self-contained help block to stderr listing the three valid domains (`observe`,
`act`, `verify`), a `Usage:` line, and exactly one pointer to `weaver --help`.
The output requires no configuration file and no running daemon. The exit code
remains non-zero. All user-facing text is resolved through the
`ortho_config::Localizer` trait backed by Fluent `.ftl` resources, establishing
the translation pipeline for the entire CLI.

Observable outcome: run `weaver` with no arguments and see:

```text
Usage: weaver <DOMAIN> <OPERATION> [ARG]...

Domains:
  observe   Query code structure and relationships
  act       Perform code modifications
  verify    Validate code correctness

Run 'weaver --help' for more information.
```

This addresses the P0 gap identified in the
[UI gap analysis Level 0](docs/ui-gap-analysis.md#level-0--bare-invocation-weaver)
 and
[Level 10d](docs/ui-gap-analysis.md#level-10--error-messages-and-exit-codes),
and satisfies roadmap task 5.1.1 in `docs/roadmap.md`.

## Constraints

- `make check-fmt`, `make lint`, and `make test` must pass after all changes.
- No code file may exceed 400 lines.
- The workspace Clippy configuration is extremely strict (pedantic, deny on
  `unwrap_used`, `expect_used`, `print_stdout`, `print_stderr`,
  `cognitive_complexity`, `missing_docs`, etc.). All new code must comply.
- Comments and documentation must use en-GB-oxendict spelling
  ("-ize" / "-yse" / "-our").
- New functionality requires both unit tests and BDD behavioural tests.
- Every module must begin with a `//!` module-level doc comment.
- The bare invocation help must not require configuration loading or daemon
  connectivity. This is a hard requirement because an operator with no config
  file should still see guidance.
- The `MissingDomain` error variant and its `Display` message must remain
  unchanged for programmatic contexts (e.g., blank-domain validation in
  `CommandInvocation::try_from`).
- `rstest-bdd` v0.5.0 must be used for BDD tests (as specified in workspace
  `Cargo.toml`).
- All new user-facing text must be sourced from Fluent `.ftl` resources via
  the `ortho_config::Localizer` trait so future locales can override it.

## Tolerances (exception triggers)

- Scope: if implementation requires changes to more than 15 files, stop and
  escalate. (Raised from 10 to accommodate the Fluent integration layer.)
- Interface: if any `pub` API signature in `weaver-cli` must change, stop and
  escalate (all changes are `pub(crate)` or private).
- Dependencies: upgrading ortho-config from v0.6.0 to v0.7.0 is pre-approved.
  Any other new external dependency requires escalation.
- Iterations: if tests still fail after 5 attempts at fixing, stop and
  escalate.

## Risks

- Risk: `unit.rs` is exactly 400 lines; adding a `mod bare_invocation;`
  declaration pushes it over the limit. Severity: low Likelihood: high
  Mitigation: Remove the redundant `///` doc comment on `mod auto_start;` (line
  399) since the module already has its own `//!` doc comment inside
  `auto_start.rs`. This frees one line for the new `mod` declaration.

- Risk: The `BareInvocation` error variant with `#[error("")]` might trigger
  a Clippy lint about empty Display strings. Severity: low Likelihood: medium
  Mitigation: If Clippy complains, use a non-empty message like
  `#[error("bare invocation")]` and ensure the match arm in
  `run_with_handler()` suppresses printing for this variant.

- Risk: The existing unit test `run_with_loader_reports_configuration_failures`
  invokes `weaver` with no args and asserts stderr contains "command domain".
  After the change, bare invocation is intercepted before config loading, so
  this test will fail. Severity: medium Likelihood: certain Mitigation: Update
  the test to pass `["weaver", "observe"]` so the bare invocation check does
  not fire, preserving the test's original intent (verifying that config loader
  errors reach stderr).

- Risk: Upgrading ortho-config from v0.6.0 to v0.7.0 may introduce subtle
  behavioural changes or new transitive dependencies (fluent-bundle,
  fluent-syntax, unic-langid). Severity: medium Likelihood: low (v0.7.0
  migration guide states "additive, no mandatory breaking changes") Mitigation:
  Run the full test suite after upgrade, before making any other changes. If
  any pre-existing tests break, fix the upgrade issues first as a separate
  commit.

- Risk: Fluent localizer construction could fail at runtime (e.g., if an
  `.ftl` resource has a syntax error). Severity: low Likelihood: low (resources
  are compiled-in via `include_str!` and validated by unit tests) Mitigation:
  Use `NoOpLocalizer` as a graceful fallback, which causes
  `Localizer::message()` to return the hardcoded English fallback string. Unit
  tests validate that the `.ftl` resources parse successfully.

## Progress

- [x] Stage A: Upgrade ortho-config to v0.7.0 and verify baseline.
- [x] Stage B: Scaffold Fluent resources and localizer module.
- [x] Stage C: Scaffold tests (red phase).
- [x] Stage D: Wire implementation (green phase).
- [x] Stage E: Documentation and roadmap updates.
- [x] Stage F: Final validation and commit gating.

## Surprises & discoveries

- The build script (`build.rs`) includes `cli.rs` via `#[path = "src/cli.rs"]`
  for manpage generation. The new `is_bare_invocation()` method triggered a
  `dead_code` warning in the build script context because it is only called
  from `lib.rs`. Resolved with a tightly scoped `#[allow(dead_code)]` attribute
  with a reason string explaining the dual-compilation context.
- The `#[error("")]` on `BareInvocation` did not trigger any Clippy lint (the
  low-probability risk from the plan did not materialise).
- `unit.rs` compaction required reformatting one test to single-line style and
  collapsing the `is_daemon_not_running_rejects_non_connect_errors` assertions
  to recover lines.

## Decision log

- Decision: Intercept bare invocation in `run_with_handler()` BEFORE config
  loading, not by changing the `MissingDomain` error Display impl. Rationale:
  Help output should not require a valid config file or daemon. A new operator
  with no config should still see guidance. The `MissingDomain` variant is also
  triggered by blank-domain strings in `CommandInvocation::try_from`, where the
  full help block would be inappropriate. Early interception cleanly separates
  the two concerns. Date/Author: 2026-02-25

- Decision: Add a `BareInvocation` sentinel variant to `AppError` rather than
  restructuring the `and_then` chain to use `Option<ExitCode>`. Rationale: The
  existing method returns `Result<ExitCode, AppError>` via an `and_then` chain.
  A sentinel error variant allows early return without restructuring the entire
  method. The match arm in the error handler suppresses the empty print for
  this specific variant. Date/Author: 2026-02-25

- Decision: Place the `is_bare_invocation()` method on the `Cli` struct rather
  than as a free function. Rationale: The method queries only `Cli` fields and
  represents a semantic property of the parsed arguments. It belongs on the
  struct. Date/Author: 2026-02-25

- Decision: Use a `PanickingLoader` in unit tests to prove bare invocation
  short-circuits before config loading. Rationale: If the interception point is
  wrong, the test panics rather than silently passing. This provides a stronger
  guarantee than a mock that returns a success. Date/Author: 2026-02-25

- Decision: Upgrade ortho-config from v0.6.0 to v0.7.0 and use
  `FluentLocalizer` with `NoOpLocalizer` fallback. Rationale: The user
  requirement is that all user-facing text is translatable using Fluent.
  ortho-config v0.7.0 provides the `Localizer` trait, `FluentLocalizer`,
  `NoOpLocalizer`, and `localize_clap_error` helpers. The upgrade is additive
  with no mandatory breaking changes per the migration guide. This establishes
  the localization pipeline for all future CLI text. Date/Author: 2026-02-26

- Decision: Create a new `crates/weaver-cli/src/localizer.rs` module rather
  than inlining Fluent setup in `runtime_utils.rs`. Rationale: Localization is
  a cross-cutting concern that will grow as more CLI text is localized. A
  dedicated module keeps it cohesive and testable. `runtime_utils.rs` remains
  for small stateless helpers. Date/Author: 2026-02-26

- Decision: Store `.ftl` resources under `crates/weaver-cli/locales/en-US/`
  following the ortho-config hello_world example convention. Rationale: This
  mirrors the upstream pattern (`locales/<locale>/messages.ftl`) and makes
  adding future locales straightforward. Date/Author: 2026-02-26

- Decision: The `write_bare_help()` function accepts `&dyn Localizer` and
  resolves each line via `Localizer::message()` with English fallbacks.
  Rationale: This means the English text is always available even when the
  Fluent pipeline fails (via `NoOpLocalizer`), while genuine translations
  override it cleanly. The function composes the help block from individual
  Fluent messages so translators can work on each line independently.
  Date/Author: 2026-02-26

- Decision: Construct `FluentLocalizer` once at the start of
  `CliRunner::run()`, falling back to `NoOpLocalizer` on error. Rationale: The
  localizer must be available before config loading (since bare invocation
  fires pre-config). Construction is cheap (one `include_str!` parse). Falling
  back to `NoOpLocalizer` ensures the CLI never crashes due to a localization
  issue — it simply serves the hardcoded English fallbacks. Date/Author:
  2026-02-26

## Outcomes & retrospective

All acceptance criteria met:

1. `weaver` with no arguments exits non-zero.
2. Output includes `Usage: weaver <DOMAIN> <OPERATION> [ARG]...`.
3. Lists three domains: `observe`, `act`, `verify`.
4. Includes exactly one `weaver --help` pointer.
5. Does not require configuration loading (proved by `PanickingLoader`).
6. All text sourced through `ortho_config::Localizer` with Fluent `.ftl`
   resources.

Quality gates passed: `make check-fmt`, `make lint`, `make test` all exit 0.
Files modified: 12 (within the 15-file tolerance). No pub API changes.

## Context and orientation

The Weaver CLI is a Rust workspace with 12 crates. The CLI binary lives in
`crates/weaver-cli/`. It uses `clap` (v4.5, derive mode) for argument parsing.

The entry point is `crates/weaver-cli/src/main.rs`, which delegates to
`weaver_cli::run()` in `crates/weaver-cli/src/lib.rs`. The `run()` function
creates a `CliRunner` and calls `CliRunner::run()`, which calls
`run_with_handler()`. This method:

1. Splits config arguments from command arguments
   (`split_config_arguments()`).
2. Parses CLI arguments via `Cli::try_parse_from()`.
3. Loads configuration via `self.loader.load()`.
4. Checks for `--capabilities` mode.
5. Checks for `daemon` subcommand.
6. Builds `CommandInvocation::try_from(cli)` — this is where
   `AppError::MissingDomain` is returned when `domain` is `None`.
7. Executes the daemon command.

The error is caught at the bottom of `run_with_handler()` (line 180-186 of
`lib.rs`) and printed to stderr.

### ortho-config and Fluent localization

The workspace currently uses `ortho_config` v0.6.0 (workspace `Cargo.toml` line
33). There is no existing Fluent infrastructure: no `.ftl` files, no `locales/`
directories, no Fluent imports anywhere.

`ortho_config` v0.7.0 (published 2026-01-02) adds:

- `Localizer` trait: `lookup(id, args) -> Option<String>` and
  `message(id, args, fallback) -> String` (fallback returned when lookup
  misses).
- `FluentLocalizer`: layered Fluent implementation constructed via
  `FluentLocalizer::builder(langid)` or convenience constructors (`embedded()`,
  `with_en_us_defaults(resources)`).
- `NoOpLocalizer`: zero-sized type that always returns `None` from `lookup()`,
  causing `message()` to use the hardcoded English fallback.
- `localize_clap_error()` / `localize_clap_error_with_command()`: rewrite
  clap error messages through the Fluent pipeline.
- `FluentLocalizerError`: `UnsupportedLocale`, `Parser`, `Registration`
  variants.

The v0.7.0 upgrade is additive with no mandatory breaking changes. New
transitive dependencies: `fluent-bundle`, `fluent-syntax`, `unic-langid`.

The hello_world example in ortho-config demonstrates the pattern:

1. `.ftl` files under `locales/en-US/messages.ftl`.
2. Embed via `include_str!("../locales/en-US/messages.ftl")`.
3. Build: `FluentLocalizer::builder(langid!("en-US"))
   .with_consumer_resources([APP_FTL]).try_build()?`.
4. Fall back to `NoOpLocalizer` on error.
5. Resolve strings via `localizer.message("msg-id", None, "fallback")`.

Fluent message IDs use hyphens (dots are normalised to hyphens by
ortho-config's `normalize_identifier()`). The embedded en-US catalogue in
ortho-config already provides `clap-error-*` messages for clap error
localization.

### Key files involved in this change

| File                                                  | Lines | Purpose                            |
| ----------------------------------------------------- | ----- | ---------------------------------- |
| `Cargo.toml` (workspace)                              | —     | ortho-config version               |
| `crates/weaver-cli/Cargo.toml`                        | —     | weaver-cli dependencies            |
| `crates/weaver-cli/src/cli.rs`                        | 73    | Clap `#[derive(Parser)]` struct    |
| `crates/weaver-cli/src/command.rs`                    | 91    | `CommandInvocation::try_from(Cli)` |
| `crates/weaver-cli/src/errors.rs`                     | 65    | `AppError` enum                    |
| `crates/weaver-cli/src/lib.rs`                        | 375   | Core runtime, `CliRunner`          |
| `crates/weaver-cli/src/runtime_utils.rs`              | 28    | Small runtime helpers              |
| `crates/weaver-cli/src/localizer.rs`                  | NEW   | Fluent localizer module            |
| `crates/weaver-cli/locales/en-US/messages.ftl`        | NEW   | Fluent resources                   |
| `crates/weaver-cli/src/tests/unit.rs`                 | 400   | Unit tests (at limit)              |
| `crates/weaver-cli/src/tests/unit/auto_start.rs`      | 227   | Auto-start tests                   |
| `crates/weaver-cli/src/tests/behaviour.rs`            | 340   | BDD step definitions               |
| `crates/weaver-cli/src/tests/support/mod.rs`          | 323   | Test world/helpers                 |
| `crates/weaver-cli/tests/features/weaver_cli.feature` | 77    | BDD scenarios                      |
| `docs/roadmap.md`                                     | 424   | Roadmap checkboxes                 |
| `docs/users-guide.md`                                 | 837   | User documentation                 |

### Test infrastructure

BDD tests use `rstest-bdd` v0.5.0 with `.feature` files in `tests/features/`
and step definitions in `src/tests/behaviour.rs`. The `TestWorld` struct (in
`src/tests/support/mod.rs`) provides a `run()` method that exercises the full
CLI flow. The `#[when("the operator runs {command}")]` step accepts a quoted
command string; an empty string `""` produces `["weaver"]` (bare invocation)
because `TestWorld::build_args()` skips extending the args vector when the
trimmed command is empty.

Existing BDD steps that can be reused without modification:

- `#[when("the operator runs {command}")]` — runs the CLI with given args.
- `#[then("the CLI fails")]` — asserts exit code is `FAILURE`.
- `#[then("stderr contains {snippet}")]` — asserts stderr contains substring.

Existing unit test helpers that can be reused:

- `run_with_loader()` — runs CLI with a custom config loader.
- `IoStreams::new()` — creates IO streams from in-memory buffers.
- `decode_utf8()` — converts a `Vec<u8>` buffer to a `String`.

## Plan of work

### Stage A: Upgrade ortho-config and verify baseline

**A1. Upgrade ortho-config to v0.7.0.**

In workspace `Cargo.toml` line 33, change `ortho_config = "0.6.0"` to
`ortho_config = "0.7.0"`.

**A2. Verify the upgrade is clean.**

Run `cargo check --workspace && make test`. If any pre-existing tests break,
fix the upgrade issues before proceeding (separate commit).

### Stage B: Scaffold Fluent resources and localizer module

**B1. Create the Fluent resource file.**

Create `crates/weaver-cli/locales/en-US/messages.ftl`:

```ftl
# Bare-invocation help block shown when weaver is run without arguments.
weaver-bare-help-usage = Usage: weaver <DOMAIN> <OPERATION> [ARG]...
weaver-bare-help-header = Domains:
weaver-bare-help-domain-observe = observe   Query code structure and relationships
weaver-bare-help-domain-act = act       Perform code modifications
weaver-bare-help-domain-verify = verify    Validate code correctness
weaver-bare-help-pointer = Run 'weaver --help' for more information.
```

**B2. Create the localizer module `crates/weaver-cli/src/localizer.rs`.**

This module:

1. Embeds the `.ftl` resource via `include_str!`.
2. Exposes `build_localizer() -> Box<dyn Localizer>` which constructs a
   `FluentLocalizer` with `en-US` defaults, falling back to `NoOpLocalizer`.
3. Exposes `write_bare_help(writer, &dyn Localizer) -> io::Result<()>` which
   resolves each message via `Localizer::message()` with English fallbacks and
   writes the composed help block.

```rust
//! Localization support for the Weaver CLI.
//!
//! Constructs a Fluent-backed localizer from embedded resources so
//! user-facing text can be translated without code changes. Falls back
//! to `NoOpLocalizer` (hardcoded English) when the Fluent pipeline fails.

use std::io::Write;

use ortho_config::{
    FluentLocalizer, Localizer, NoOpLocalizer, langid,
};

static WEAVER_EN_US: &str =
    include_str!("../locales/en-US/messages.ftl");

/// Builds the application localizer.
///
/// Returns a `FluentLocalizer` loaded with the embedded en-US catalogue
/// and any consumer overrides. Falls back to `NoOpLocalizer` on error so
/// the CLI never crashes due to a localization failure.
pub(crate) fn build_localizer() -> Box<dyn Localizer> {
    match FluentLocalizer::with_en_us_defaults([WEAVER_EN_US]) {
        Ok(loc) => Box::new(loc),
        Err(_) => Box::new(NoOpLocalizer),
    }
}

/// Writes the bare-invocation help block to `writer`.
///
/// Each line is resolved through the localizer with a hardcoded English
/// fallback, so the output is correct even without Fluent resources.
pub(crate) fn write_bare_help<W: Write>(
    writer: &mut W,
    localizer: &dyn Localizer,
) -> std::io::Result<()> {
    let usage = localizer.message(
        "weaver-bare-help-usage", None,
        "Usage: weaver <DOMAIN> <OPERATION> [ARG]...",
    );
    let header = localizer.message(
        "weaver-bare-help-header", None, "Domains:",
    );
    let observe = localizer.message(
        "weaver-bare-help-domain-observe", None,
        "observe   Query code structure and relationships",
    );
    let act = localizer.message(
        "weaver-bare-help-domain-act", None,
        "act       Perform code modifications",
    );
    let verify = localizer.message(
        "weaver-bare-help-domain-verify", None,
        "verify    Validate code correctness",
    );
    let pointer = localizer.message(
        "weaver-bare-help-pointer", None,
        "Run 'weaver --help' for more information.",
    );
    write!(
        writer,
        "{usage}\n\n{header}\n  {observe}\n  {act}\n  {verify}\n\n{pointer}\n",
    )
}
```

Note: The two-space indent before each domain line is part of the format
template, not the `.ftl` value. This keeps domain descriptions
alignment-agnostic in translations.

**B3. Add `BareInvocation` variant to `AppError` in `errors.rs`.**

Below the `MissingOperation` variant:

```rust
    #[error("")]
    BareInvocation,
```

**B4. Add `is_bare_invocation()` method to `Cli` in `cli.rs`.**

```rust
impl Cli {
    /// Returns true when no domain, subcommand, or probe flag was supplied.
    ///
    /// This detects the case where the operator invoked `weaver` with no
    /// meaningful arguments, so the runner can emit short help guidance
    /// before attempting configuration loading or daemon contact.
    pub(crate) fn is_bare_invocation(&self) -> bool {
        self.domain.is_none() && self.command.is_none() && !self.capabilities
    }
}
```

**B5. Register the localizer module in `lib.rs`.**

Add `mod localizer;` to the module list, and update the use statement:

```rust
mod localizer;
// ...
use localizer::{build_localizer, write_bare_help};
```

### Stage C: Scaffold tests (red phase)

**C1. Add BDD scenario.**

In `crates/weaver-cli/tests/features/weaver_cli.feature`, append:

```gherkin
  Scenario: Bare invocation shows short help
    When the operator runs ""
    Then the CLI fails
    And stderr contains "Usage: weaver"
    And stderr contains "observe"
    And stderr contains "act"
    And stderr contains "verify"
    And stderr contains "weaver --help"
```

No new step definitions required.

**C2. Create unit test file `src/tests/unit/bare_invocation.rs`.**

Tests that:

1. Bare invocation exits with `FAILURE`.
2. Bare invocation emits text containing the expected fragments to stderr.
3. Bare invocation produces no stdout.
4. Bare invocation does not attempt config loading (`PanickingLoader`).
5. `write_bare_help()` with `NoOpLocalizer` produces the expected English
   help block (tests the fallback path).
6. `write_bare_help()` with the real `FluentLocalizer` produces the same
   English help block (tests the Fluent resolution path).
7. The help text contains all three domains.
8. The help text contains a `Usage:` line.
9. The help text contains exactly one `weaver --help` pointer.

```rust
//! Tests for bare-invocation help output.
//!
//! Verifies that running `weaver` with no arguments emits the short help
//! block to stderr and exits non-zero, without requiring configuration
//! loading or daemon connectivity.

use std::ffi::OsString;
use std::io::Cursor;
use std::process::ExitCode;

use ortho_config::{Localizer, NoOpLocalizer};

use crate::localizer::{build_localizer, write_bare_help};
use crate::{AppError, ConfigLoader, IoStreams, run_with_loader};
use weaver_config::Config;

/// A config loader that panics if called, proving that bare invocation
/// short-circuits before configuration loading.
struct PanickingLoader;

impl ConfigLoader for PanickingLoader {
    fn load(&self, _args: &[OsString]) -> Result<Config, AppError> {
        panic!("bare invocation must not attempt configuration loading");
    }
}

/// Renders the bare help block using the given localizer.
fn render_help(localizer: &dyn Localizer) -> String {
    let mut buf = Vec::new();
    write_bare_help(&mut buf, localizer)
        .expect("write bare help");
    String::from_utf8(buf).expect("utf8")
}

#[test]
fn bare_invocation_exits_with_failure() {
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    let mut stdin = Cursor::new(Vec::new());
    let mut io = IoStreams::new(&mut stdin, &mut stdout, &mut stderr, false);
    let exit = run_with_loader(
        vec![OsString::from("weaver")],
        &mut io,
        &PanickingLoader,
    );
    assert_eq!(exit, ExitCode::FAILURE);
}

#[test]
fn bare_invocation_emits_help_to_stderr() {
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    let mut stdin = Cursor::new(Vec::new());
    let mut io = IoStreams::new(&mut stdin, &mut stdout, &mut stderr, false);
    let _ = run_with_loader(
        vec![OsString::from("weaver")],
        &mut io,
        &PanickingLoader,
    );
    let stderr_text = String::from_utf8(stderr).expect("stderr utf8");
    assert!(stderr_text.contains("Usage: weaver"));
    assert!(stderr_text.contains("observe"));
    assert!(stderr_text.contains("act"));
    assert!(stderr_text.contains("verify"));
    assert!(stderr_text.contains("weaver --help"));
}

#[test]
fn bare_invocation_produces_no_stdout() {
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    let mut stdin = Cursor::new(Vec::new());
    let mut io = IoStreams::new(&mut stdin, &mut stdout, &mut stderr, false);
    let _ = run_with_loader(
        vec![OsString::from("weaver")],
        &mut io,
        &PanickingLoader,
    );
    assert!(
        stdout.is_empty(),
        "bare invocation must not write to stdout"
    );
}

#[test]
fn write_bare_help_with_noop_produces_english_fallback() {
    let text = render_help(&NoOpLocalizer);
    assert!(text.contains("Usage: weaver"));
    assert!(text.contains("observe"));
    assert!(text.contains("act"));
    assert!(text.contains("verify"));
    assert!(text.contains("weaver --help"));
}

#[test]
fn write_bare_help_with_fluent_produces_english() {
    let localizer = build_localizer();
    let text = render_help(localizer.as_ref());
    assert!(text.contains("Usage: weaver"));
    assert!(text.contains("observe"));
    assert!(text.contains("act"));
    assert!(text.contains("verify"));
    assert!(text.contains("weaver --help"));
}

#[test]
fn bare_help_contains_usage_line() {
    let text = render_help(&NoOpLocalizer);
    assert!(text.contains("Usage:"));
}

#[test]
fn bare_help_contains_single_help_pointer() {
    let text = render_help(&NoOpLocalizer);
    let count = text.matches("weaver --help").count();
    assert_eq!(count, 1, "expected exactly one --help pointer");
}
```

**C3. Add `mod bare_invocation;` declaration to `unit.rs`.**

Replace line 399 (`/// Tests for automatic...` doc comment on `auto_start`)
with the module declarations:

```rust
mod auto_start;
/// Tests for bare-invocation help output.
mod bare_invocation;
```

This keeps the file at exactly 400 lines.

**C4. Verify the project compiles but new tests fail.**

Run `cargo check --workspace` — should compile. Run `make test` — new tests
should fail because the interception logic is not yet wired.

### Stage D: Wire implementation (green phase)

**D1. Wire the bare invocation interception in `lib.rs`.**

In `CliRunner::run()` (the non-handler version, line 115-123), construct the
localizer before entering the handler:

```rust
fn run<I>(&mut self, args: I) -> ExitCode
where
    I: IntoIterator<Item = OsString>,
{
    let localizer = build_localizer();
    let mut lifecycle = SystemLifecycle;
    self.run_with_handler(args, localizer.as_ref(), |invocation, context, output| {
        lifecycle.handle(invocation, context, output)
    })
}
```

Update `run_with_handler()` to accept `localizer: &dyn Localizer` and use it in
the bare invocation check:

```rust
fn run_with_handler<I, F>(
    &mut self,
    args: I,
    localizer: &dyn Localizer,
    mut handler: F,
) -> ExitCode
```

In the `and_then` chain, after `Cli::try_parse_from`:

```rust
.and_then(|cli| {
    if cli.is_bare_invocation() {
        let _ = write_bare_help(&mut *self.io.stderr, localizer);
        return Err(AppError::BareInvocation);
    }
    self.loader
        .load(&split.config_arguments)
        .map(|config| (cli, config))
})
```

**D2. Suppress duplicate error printing for `BareInvocation`.**

In the `match result` block:

```rust
match result {
    Ok(exit_code) => exit_code,
    Err(AppError::BareInvocation) => ExitCode::FAILURE,
    Err(error) => {
        let _ = writeln!(self.io.stderr, "{error}");
        ExitCode::FAILURE
    }
}
```

**D3. Update `run_with_daemon_binary` (test-only) to pass localizer.**

The `#[cfg(test)]` function `run_with_daemon_binary()` creates a `CliRunner`
and calls `run_with_handler`. It needs to accept and forward a localizer. Use
`&NoOpLocalizer` as the default for existing test callers. Update the test
support `TestWorld::run()` to pass `&NoOpLocalizer`.

**D4. Update the existing unit test
`run_with_loader_reports_configuration_failures`.**

Change args from `vec![OsString::from("weaver")]` to
`vec![OsString::from("weaver"), OsString::from("observe")]` so the bare
invocation check does not fire.

**D5. Verify all tests pass (green phase).**

Run `make check-fmt && make lint && make test`.

### Stage E: Documentation and roadmap updates

**E1. Update `docs/users-guide.md`.**

Add a "Bare invocation" subsection within "Command reference", before "Output
formats":

Add a heading `### Bare invocation` describing the new behaviour.  Include a
`text` code block showing the help output and a note that it does not require a
configuration file or a running daemon.  See the actual `docs/users-guide.md`
change for the rendered version.

**E2. Mark roadmap task 5.1.1 as done in `docs/roadmap.md`.**

Change the three `[ ]` checkboxes for task 5.1.1 to `[x]`.

**E3. Run `make fmt` and `make markdownlint`.**

### Stage F: Final validation and commit gating

**F1. Run full commit gating suite.**

```sh
set -o pipefail
make check-fmt 2>&1 | tee /tmp/check-fmt.log
make lint 2>&1 | tee /tmp/lint.log
make test 2>&1 | tee /tmp/test.log
```

All three must pass with zero exit code.

## Concrete steps

All commands run from the workspace root `/home/user/project`.

The concrete steps mirror the Plan of Work stages A through F. See the Plan of
Work for full code listings; this section summarises the edits and commands.

### Stage A: Upgrade ortho-config

1. Edit `Cargo.toml` line 33: `"0.6.0"` → `"0.7.0"`.
2. Run `cargo check --workspace && make test` to verify no regressions.

### Stage B: Scaffold Fluent resources and localizer

1. Create `crates/weaver-cli/locales/en-US/messages.ftl` with bare-help
   Fluent messages (see Plan of Work B1).
2. Create `crates/weaver-cli/src/localizer.rs` with `build_localizer()` and
   `write_bare_help()` (see Plan of Work B2).
3. Add `BareInvocation` to `AppError` in `errors.rs` (B3).
4. Add `is_bare_invocation()` to `Cli` in `cli.rs` (B4).
5. Register `mod localizer;` and update imports in `lib.rs` (B5).

### Stage C: Scaffold tests

1. Append BDD scenario to `weaver_cli.feature` (C1).
2. Create `src/tests/unit/bare_invocation.rs` with unit tests (C2).
3. Update `unit.rs` to declare `mod bare_invocation` (C3).
4. Run `cargo check --workspace` then `make test` — new tests should fail
   (red phase).

### Stage D: Wire implementation

1. Construct localizer in `CliRunner::run()` and thread through
   `run_with_handler()` (D1).
2. Add `BareInvocation` match arm in error handler (D2).
3. Update `run_with_daemon_binary` and `TestWorld::run()` to pass
   `&NoOpLocalizer` (D3).
4. Fix `run_with_loader_reports_configuration_failures` test args (D4).
5. Run `make check-fmt && make lint && make test` — all must pass (D5).

### Stage E: Documentation

1. Add "Bare invocation" subsection to `docs/users-guide.md`.
2. Mark 5.1.1 as done in `docs/roadmap.md`.
3. Run `make fmt && make markdownlint`.

### Stage F: Final validation

```sh
set -o pipefail
make check-fmt 2>&1 | tee /tmp/check-fmt.log
make lint 2>&1 | tee /tmp/lint.log
make test 2>&1 | tee /tmp/test.log
```

## Validation and acceptance

**Acceptance criteria** (from roadmap):

1. `weaver` with no arguments exits non-zero — verified by BDD scenario
   `Bare invocation shows short help` (`Then the CLI fails`) and unit test
   `bare_invocation_exits_with_failure`.
2. Prints a `Usage:` line — verified by BDD `stderr contains "Usage: weaver"`
   and unit test `bare_help_contains_usage_line`.
3. Lists the three valid domains — verified by BDD assertions for `observe`,
   `act`, `verify` and unit tests
   `write_bare_help_with_noop_produces_english_fallback` /
   `write_bare_help_with_fluent_produces_english`.
4. Includes exactly one pointer to `weaver --help` — verified by unit test
   `bare_help_contains_single_help_pointer`.
5. Does not require config loading — verified by `PanickingLoader` in unit
   tests (loader panics if called).
6. All user-facing text is sourced through `ortho_config::Localizer` — verified
   by unit tests exercising both `NoOpLocalizer` and `FluentLocalizer` paths.

**Quality criteria:**

- Tests: `make test` passes with zero exit code, including all new tests.
- Lint: `make lint` passes (Clippy pedantic, deny warnings).
- Format: `make check-fmt` passes.
- Markdown: `make markdownlint` passes after doc changes.

**Quality method:**

```sh
make check-fmt && make lint && make test && make markdownlint
```

## Idempotence and recovery

All steps are file edits and can be re-applied. If any step fails partway
through, the working tree can be reset with `git checkout -- .` and the steps
re-executed from the beginning. No external state is modified.

## Interfaces and dependencies

**Upgraded dependency:** `ortho_config` v0.6.0 → v0.7.0 (additive, no breaking
changes). Brings transitive deps: `fluent-bundle`, `fluent-syntax`,
`unic-langid`.

New `pub(crate)` interfaces:

In `crates/weaver-cli/src/cli.rs`:

```rust
impl Cli {
    pub(crate) fn is_bare_invocation(&self) -> bool;
}
```

In `crates/weaver-cli/src/localizer.rs`:

```rust
pub(crate) fn build_localizer() -> Box<dyn Localizer>;
pub(crate) fn write_bare_help<W: Write>(
    writer: &mut W,
    localizer: &dyn Localizer,
) -> std::io::Result<()>;
```

In `crates/weaver-cli/src/errors.rs`:

```rust
#[derive(Debug, Error)]
pub(crate) enum AppError {
    // ... existing variants ...
    #[error("")]
    BareInvocation,
}
```

## Files modified (summary)

| File                                                  | Change                              |
| ----------------------------------------------------- | ----------------------------------- |
| `Cargo.toml`                                          | Upgrade `ortho_config` to `"0.7.0"` |
| `crates/weaver-cli/locales/en-US/messages.ftl`        | New: Fluent resources               |
| `crates/weaver-cli/src/localizer.rs`                  | New: localizer module               |
| `crates/weaver-cli/src/cli.rs`                        | Add `is_bare_invocation()` method   |
| `crates/weaver-cli/src/errors.rs`                     | Add `BareInvocation` variant        |
| `crates/weaver-cli/src/lib.rs`                        | Wire localizer + bare invocation    |
| `crates/weaver-cli/src/tests/unit.rs`                 | Fix test args; add `mod` decl       |
| `crates/weaver-cli/src/tests/unit/bare_invocation.rs` | New: unit tests                     |
| `crates/weaver-cli/src/tests/support/mod.rs`          | Pass `NoOpLocalizer`                |
| `crates/weaver-cli/tests/features/weaver_cli.feature` | Add BDD scenario                    |
| `docs/users-guide.md`                                 | Add "Bare invocation" subsection    |
| `docs/roadmap.md`                                     | Mark 5.1.1 checkboxes as done       |
