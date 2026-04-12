# Surface configuration flags in clap help output

This ExecPlan (execution plan) is a living document. The sections
`Constraints`, `Tolerances`, `Risks`, `Progress`, `Surprises & Discoveries`,
`Decision Log`, and `Outcomes & Retrospective` must be kept up to date as work
proceeds.

Status: COMPLETE

This document must be maintained in accordance with `AGENTS.md` at the
repository root.

This plan must be approved before implementation begins.

## Purpose / big picture

Roadmap item `3.2.1` closes the current gap where Weaver's configuration flags
exist at runtime but are invisible in clap-generated help. Today
`weaver --help` and `weaver daemon start --help` show the domain catalogue and
runtime options, but they omit the configuration contract that operators
actually need to discover first: `--config-path`, `--daemon-socket`,
`--log-filter`, `--log-format`, `--capability-overrides`, and `--locale`.

After this change, both help surfaces must advertise all six flags directly in
their `Options:` section without requiring the operator to read
`docs/users-guide.md`. The change must preserve the existing configuration
precedence contract (`defaults < files < environment < CLI`) and must not
silently broaden when configuration flags are honoured at runtime. In
particular, help output must become more truthful without making
`weaver daemon start --log-filter debug` look supported if the runtime still
requires configuration flags to appear before the command token.

Observable outcome after implementation:

```plaintext
$ weaver --help
...
Options:
      --config-path <PATH>
      --daemon-socket <ENDPOINT>
      --log-filter <FILTER>
      --log-format <FORMAT>
      --capability-overrides <DIRECTIVE>
      --locale <LOCALE>
...
```

And:

```plaintext
$ weaver daemon start --help
...
Options:
      --config-path <PATH>
      --daemon-socket <ENDPOINT>
      --log-filter <FILTER>
      --log-format <FORMAT>
      --capability-overrides <DIRECTIVE>
      --locale <LOCALE>
...
```

The existing precedence tests must keep passing, and the new `locale` flag must
be an honest part of the shared configuration contract rather than a help-only
fiction.

## Constraints

- Run `make check-fmt`, `make lint`, and `make test` before considering the
  feature complete.
- Because the implementation also updates Markdown, run `make fmt`,
  `make markdownlint`, and `make nixie` before finishing.
- Add unit coverage and behavioural coverage using `rstest-bdd` v0.5.0 for
  the help surfaces, precedence behaviour, and relevant edge cases.
- Preserve the current runtime rule documented in the CLI: configuration flags
  take effect only when they appear before the command domain or structured
  subcommand. Do not silently make post-domain placement work as a side effect
  of the help change.
- Keep the JSONL transport, daemon routing, and auto-start behaviour unchanged.
  `--help` must still complete without daemon startup or socket access.
- Keep `weaver-config` as the authoritative source of the shared configuration
  contract. The CLI may add help-rendering metadata, but it must not invent
  config flags that the loader cannot actually accept.
- Add `locale` to the shared configuration contract in a way that is safe to
  ship now and reusable by roadmap `3.3.1`. This task does not need to complete
  the later bootstrap-localizer work, but it must not contradict it.
- Update `docs/weaver-design.md` with the final help-rendering decision and
  the sequencing decision for `locale`.
- Update `docs/users-guide.md` so the documented help/configuration story
  matches the shipped CLI.
- Mark roadmap item `3.2.1` done only after code, tests, documentation, and
  all gates pass.
- Keep source files under 400 lines. `crates/weaver-cli/src/lib.rs` is already
  above that threshold today, so the implementation must extract logic before
  widening that file further.
- Comments and documentation must use en-GB-oxendict spelling.

## Tolerances (exception triggers)

- Scope: if the work grows beyond about 14 touched files or roughly 350 net
  lines, stop and re-evaluate before continuing.
- Parsing semantics: if showing the flags in help appears to require changing
  the runtime ordering rule for configuration flags, stop and escalate instead
  of changing behaviour implicitly.
- Locale scope: if making `--locale` a real config field forces the full
  bootstrap-localizer implementation from roadmap `3.3.1`, stop and revisit
  sequencing rather than absorbing that larger task silently.
- File size: if `crates/weaver-cli/src/lib.rs`, `crates/weaver-cli/src/cli.rs`,
  or `crates/weaver-config/src/lib.rs` would exceed 400 lines after the change,
  extract a focused helper or submodule before proceeding.
- Build-script coupling: if the chosen help solution cannot be shared cleanly
  with `crates/weaver-cli/build.rs` for manpage generation, stop and document
  the options before continuing.
- Dependency churn: if validated locale parsing requires more than one new
  dependency or a non-trivial dependency tree change, stop and review whether
  `locale` should remain a string-backed value until roadmap `3.3.1`.

## Risks

- Risk: `weaver-config` already declares `config_cli_visible = true`, but the
  runtime help still hides `--config-path` because `Cli::try_parse_from(...)`
  and `Config::load_from_iter(...)` are separate parsing paths. Mitigation:
  treat this as a help-rendering integration problem, not as proof that the
  loader metadata is sufficient on its own.

- Risk: adding visible global clap args directly to `Cli` could accidentally
  make post-subcommand forms such as `weaver daemon start --log-filter debug`
  parse successfully even though the config loader would ignore them.
  Mitigation: keep runtime parsing and help rendering separate if needed, and
  add regression tests for the ordering rule.

- Risk: `crates/weaver-cli/src/lib.rs` is already 426 lines. Mitigation: move
  config-flag metadata and any help-specific dispatch into a dedicated helper
  module before adding more runtime branches.

- Risk: `Config` does not currently contain `locale`, while the roadmap item
  requires the flag to be visible. Mitigation: add the field now as part of the
  shared config contract, but keep the later locale-bootstrap and translated
  help work explicitly out of scope for this roadmap item.

- Risk: clap help output also drives manpage generation through
  `crates/weaver-cli/build.rs`. Mitigation: use one shared help-command builder
  so `weaver --help` and generated roff output cannot drift.

- Risk: precedence tests currently cover only daemon socket defaults and
  overrides. Mitigation: preserve the existing scenarios untouched and add a
  focused locale-precedence scenario only if `locale` becomes a real loader
  field in this task.

## Progress

- [x] (2026-04-10 00:00Z) Read `docs/roadmap.md`,
      `docs/ui-gap-analysis.md`, `docs/weaver-design.md`,
      `docs/users-guide.md`, and the referenced testing guides.
- [x] (2026-04-10 00:10Z) Confirmed the live implementation seam:
      `crates/weaver-cli/src/config.rs::split_config_arguments(...)` strips
      only leading config flags before clap parses the rest of the CLI.
- [x] (2026-04-10 00:12Z) Confirmed the help gap:
      `crates/weaver-cli/src/cli.rs::Cli` declares no config flags, so
      clap-generated help cannot display them.
- [x] (2026-04-10 00:15Z) Confirmed the contract mismatch:
      `crates/weaver-config/src/lib.rs::Config` exposes
      `daemon_socket`, `log_filter`, `log_format`, and
      `capability_overrides`, but no `locale` field yet.
- [x] (2026-04-10 00:20Z) Confirmed the build coupling:
      `crates/weaver-cli/build.rs` uses `Cli::command()` for manpage
      generation, so the final help solution must be shared there as well.
- [x] (2026-04-10 00:35Z) Drafted this ExecPlan in
      `docs/execplans/3-2-1-surface-configuration-flags-in-clap-help-output.md`.
- [x] (2026-04-11 00:20Z) Stage A: added unit coverage in
      `crates/weaver-cli/src/tests/unit/help_output.rs`, integration coverage
      in `crates/weaver-cli/tests/main_entry.rs`, behavioural coverage in
      `crates/weaver-cli/tests/features/weaver_cli.feature`, and a feature-gated
      `rstest-bdd` locale-precedence scenario in
      `crates/weaver-config/tests/features/configuration_precedence.feature`.
- [x] (2026-04-11 00:35Z) Stage B: added `Config::locale`, surfaced it as
      `--locale`, `WEAVER_LOCALE`, and a config-file key, and updated
      `CONFIG_CLI_FLAGS` so the CLI still strips leading locale config flags
      before runtime clap parsing.
- [x] (2026-04-11 00:45Z) Stage C: extracted help rendering into
      `crates/weaver-cli/src/help.rs`, moved preflight logic into
      `crates/weaver-cli/src/preflight.rs`, and kept
      `crates/weaver-cli/src/lib.rs` under 400 lines while rendering help from
      an augmented clap command only when clap itself requests help.
- [x] (2026-04-11 01:20Z) Stage D: updated `crates/weaver-cli/build.rs`,
      `docs/weaver-design.md`, `docs/users-guide.md`, and `docs/roadmap.md`,
      then passed `make fmt`, `make markdownlint`, `make nixie`,
      `make check-fmt`, `make lint`, and `make test`.

## Surprises & Discoveries

- `config_cli_visible = true` already exists on the `weaver-config` discovery
  attribute, but that visibility applies only inside `ortho-config`'s loader.
  It does not automatically affect `weaver-cli`'s clap help because the CLI and
  config loader are parsed separately.

- The current `CONFIG_CLI_FLAGS` list in `crates/weaver-cli/src/lib.rs`
  contains only five flags and omits `--locale`, so the roadmap and runtime
  have already diverged.

- `crates/weaver-cli/src/lib.rs` is already over the repository's 400-line
  limit, so even a small feature here should begin with extraction rather than
  direct growth.

- The safest way to keep help honest may be to render help from an augmented
  clap `Command` while leaving the runtime parser strict. That is a more
  deliberate design than simply adding shadow fields to `Cli`.

- `unic_langid::LanguageIdentifier` is not serde-enabled through the current
  workspace dependency surface, so a direct `Config::locale` field would not
  compile. A small validated `Locale` newtype in `weaver-config` keeps the
  config contract honest without adding new dependency churn.

## Decision Log

- Decision: this plan assumes `--locale` should become a real member of the
  shared configuration contract during `3.2.1`, even though the later locale
  bootstrap and translated help work remains in roadmap `3.3.1`. Rationale:
  showing a flag in help that the loader cannot accept would be misleading.
  Date: 2026-04-10.

- Decision: prefer a shared help-command builder over adding shadow config
  fields directly to the runtime `Cli` parser if direct clap registration would
  relax the existing "flags must come first" rule. Rationale: the operator
  should gain discoverability without accidental new parsing semantics. Date:
  2026-04-10.

- Decision: keep `weaver-config` as the source of truth for accepted config
  flags, and keep `weaver-cli` as the place where those flags are rendered for
  help and operator guidance. Rationale: that split matches the existing
  architecture and avoids inventing a second config contract. Date: 2026-04-10.

- Decision: ship `locale` in `3.2.1` as a validated newtype inside
  `weaver-config` instead of storing `unic_langid::LanguageIdentifier`
  directly. Rationale: the direct type is not serde-enabled in the current
  workspace dependency graph, while the newtype preserves validation and keeps
  dependency churn below the tolerance threshold. Date: 2026-04-11.

## Outcomes & Retrospective

Target outcome at completion:

1. `weaver --help` lists all six configuration flags in its clap `Options:`
   section and still exits 0 without daemon startup.
2. `weaver daemon start --help` lists the same six flags in its clap
   `Options:` section and still exits 0 without daemon startup.
3. The shared configuration loader accepts `--locale` alongside the existing
   config flags and preserves the documented precedence ordering.
4. The runtime still requires configuration flags to appear before the command
   domain or structured subcommand to take effect.
5. Unit tests, integration tests, and `rstest-bdd` scenarios cover happy
   paths, unhappy paths, and the relevant ordering and precedence edge cases.
6. `docs/weaver-design.md`, `docs/users-guide.md`, and `docs/roadmap.md`
   reflect the shipped behaviour.
7. `make fmt`, `make markdownlint`, `make nixie`, `make check-fmt`,
   `make lint`, and `make test` all pass.

Retrospective notes:

- Final solution: use an augmented help-only clap `Command` in
  `crates/weaver-cli/src/help.rs`, shared by the runtime help path and
  `crates/weaver-cli/build.rs`, while leaving the runtime parser strict.
- `locale` shipped as a validated `weaver-config::Locale` newtype because the
  direct `unic_langid::LanguageIdentifier` type was not serde-enabled through
  the current workspace dependency surface. This preserved validation without
  dragging roadmap `3.3.1` into the current task.
- Shared builder verification caught the main drift risk up front: top-level
  help and generated manpages were both sourcing `Cli::command()`, so
  augmenting only the runtime path would have left the roff output stale.

## Context and orientation

The implementation surface is concentrated in `weaver-cli` and `weaver-config`.

- `crates/weaver-cli/src/config.rs`

  Owns `split_config_arguments(...)` and `prepare_cli_arguments(...)`. This is
  where leading configuration flags are separated from the clap-parsed command
  surface today.

- `crates/weaver-cli/src/cli.rs`

  Owns `Cli`, `CliCommand`, and `DaemonAction`. This is the clap definition
  whose help text currently omits configuration flags.

- `crates/weaver-cli/src/lib.rs`

  Owns `CONFIG_CLI_FLAGS`, `CliRunner::run_with_handler(...)`, and
  `handle_preflight(...)`. This is the runtime entry point and the current
  pressure point for any help interception or config-flag extraction.

- `crates/weaver-cli/build.rs`

  Generates the manpage from `Cli::command()`. Any help-surface solution that
  is not reused here will create drift immediately.

- `crates/weaver-config/src/lib.rs`

  Owns `Config` and the `ortho-config` derive annotations that define the
  shared config contract.

- `crates/weaver-config/tests/configuration_precedence.rs` and
  `crates/weaver-config/tests/features/configuration_precedence.feature`

  Already provide a `rstest-bdd` harness for precedence testing and should be
  extended rather than replaced.

- `crates/weaver-cli/src/tests/unit/*.rs`,
  `crates/weaver-cli/tests/main_entry.rs`, and
  `crates/weaver-cli/tests/features/weaver_cli.feature`

  Already provide unit, integration, and behavioural coverage for help,
  discoverability, and daemon-start avoidance.

## Plan of work

### Stage A: Lock the expected behaviour down with failing tests

Begin with red tests so the help contract is explicit before code changes.

Add a new unit test module under `crates/weaver-cli/src/tests/unit/` dedicated
to configuration help surfaces. Keep it separate from `version_output.rs` so
the tests remain cohesive and the file-size pressure on the existing
help/version tests stays low. These tests should assert that:

- the top-level help text contains all six flags;
- `weaver daemon start --help` contains the same six flags;
- `--capability-overrides` is shown with the correct repeatable/directive
  shape;
- `split_config_arguments(...)` recognises `--locale` in both separate-value
  and inline-value forms; and
- configuration flags placed after the domain or structured subcommand are not
  accidentally reclassified as working runtime overrides.

Extend `crates/weaver-cli/tests/main_entry.rs` with executable integration
tests for `weaver --help` and `weaver daemon start --help`. These should assert
on stdout, exit code 0, the six flag names, and the absence of
`Waiting for daemon start...`.

Extend `crates/weaver-cli/tests/features/weaver_cli.feature` and the existing
step definitions in `crates/weaver-cli/src/tests/behaviour.rs` with
`rstest-bdd` scenarios covering:

- top-level help lists all six configuration flags;
- `daemon start --help` lists all six configuration flags; and
- help rendering does not start or contact the daemon.

If `locale` is added as a real config field in Stage B, extend
`crates/weaver-config/tests/features/configuration_precedence.feature` with a
new scenario proving `file < env < CLI` precedence for `locale`, and add one
unhappy-path test for malformed locale input if the chosen type validates it.

### Stage B: Make `locale` part of the shared configuration contract

Add a `locale` field to `crates/weaver-config/src/lib.rs::Config` so `--locale`
is a real accepted config flag rather than help-only decoration. Prefer a small
dedicated module such as `crates/weaver-config/src/locale.rs` if a newtype or
parser keeps the logic clearer and reusable for roadmap `3.3.1`.

The field should:

- participate in the same layered precedence as the existing config fields;
- be surfaced as `--locale` and `WEAVER_LOCALE`;
- have a documented default, likely `en-US`; and
- stop short of wiring the localizer bootstrap or translated help selection in
  this roadmap item.

Update any loader-facing constants or metadata so the runtime recognises
`--locale` during config-argument splitting. This includes removing the
hard-coded five-flag assumption from `crates/weaver-cli/src/lib.rs`.

### Stage C: Centralize config-help metadata and build a truthful help surface

Extract the config-flag catalogue out of `crates/weaver-cli/src/lib.rs` into a
focused helper module or into `crates/weaver-cli/src/config.rs`. That metadata
should describe the six flags once, including long name, value placeholder,
repeatability, and help text.

Use that catalogue in two places:

1. config-argument splitting, so the runtime still knows which leading tokens
   belong to `weaver-config`; and
2. a new augmented clap `Command` builder for help rendering.

The recommended implementation is:

- keep `Cli::try_parse_from(...)` strict for normal execution;
- add a helper such as `cli::build_help_command()` that starts from
  `Cli::command()` and appends the six config flags as visible global args; and
- add a narrow early branch in `CliRunner::run_with_handler(...)` that detects
  help requests on the raw `argv` and renders help from that augmented command
  instead of the stripped runtime parser.

This preserves the current rule that configuration flags must appear before the
command token to take effect, while still making clap help and manpage output
accurate.

### Stage D: Reuse the same help builder in the build script

Update `crates/weaver-cli/build.rs` to generate the manpage from the same
augmented help command used at runtime. Do not keep a separate roff-only code
path. The manpage should inherit the same six visible config flags
automatically.

If `cli.rs` becomes too crowded, move the augmented-command helper into a new
module that both `lib.rs` and `build.rs` can include safely.

### Stage E: Document the shipped contract

Update `docs/weaver-design.md` in the sections covering localized help surfaces
and the configuration contract. Record two decisions explicitly:

- configuration flags are surfaced in help through a shared augmented command,
  while runtime parsing remains strict about flag ordering; and
- `locale` enters the shared config contract in this task, but the bootstrap
  locale-selection behaviour remains the responsibility of roadmap `3.3.1`.

Update `docs/users-guide.md` so the configuration section and help/discovery
section both reflect the new operator experience. The guide should say that the
flags are visible in `--help`, should list `--locale`, and should restate that
the flags must appear before the command domain or structured subcommand to
take effect.

Only after the implementation, tests, and docs are complete should
`docs/roadmap.md` mark `3.2.1` as done.

## Validation

Use focused commands first so failures are easier to interpret:

```plaintext
set -o pipefail; cargo test -p weaver-cli help_output 2>&1 | tee /tmp/3-2-1-weaver-cli-help.log
set -o pipefail; cargo test -p weaver-cli daemon_start_help 2>&1 | tee /tmp/3-2-1-weaver-cli-daemon-help.log
set -o pipefail; cargo test -p weaver-config configuration_precedence -- --nocapture 2>&1 | tee /tmp/3-2-1-weaver-config-precedence.log
```

Then verify the observable help surfaces manually:

```plaintext
cargo run -p weaver-cli -- --help
cargo run -p weaver-cli -- daemon start --help
```

Expected observable properties:

- both commands exit 0;
- both write help to stdout;
- both list the six configuration flags;
- neither prints `Waiting for daemon start...`; and
- `daemon start --help` still reads like a help surface, not a lifecycle
  execution path.

Finish with the full required gates:

```plaintext
set -o pipefail; make fmt 2>&1 | tee /tmp/3-2-1-make-fmt.log
set -o pipefail; make markdownlint 2>&1 | tee /tmp/3-2-1-make-markdownlint.log
set -o pipefail; make nixie 2>&1 | tee /tmp/3-2-1-make-nixie.log
set -o pipefail; make check-fmt 2>&1 | tee /tmp/3-2-1-make-check-fmt.log
set -o pipefail; make lint 2>&1 | tee /tmp/3-2-1-make-lint.log
set -o pipefail; make test 2>&1 | tee /tmp/3-2-1-make-test.log
```

## Approval

Do not begin implementation when this document is first written. Present the
plan, call out the assumption that `locale` enters the shared config contract
in this roadmap item, and wait for explicit user approval before editing Rust
code or updating the roadmap entry.
