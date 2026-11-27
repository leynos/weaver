# 13.2.1 Implement the localized human renderer for `definitions get`

This ExecPlan (execution plan) is a living document. The sections `Constraints`,
`Tolerances`, `Risks`, `Progress`, `Surprises & discoveries`, `Decision log`,
`Outcomes & retrospective`, `Conformance basis`, and `Verification plan` must
be kept up to date as work proceeds.

Status: DRAFT (approver's questions answered; awaiting approval to implement)

Branch: `13-2-1-localized-human-renderer-for-definitions-get`

Roadmap item: `13.2.1` in `docs/roadmap.md`.

## Purpose / big picture

Weaver is a command-line tool (`weaver`) that asks a background daemon
(`weaverd`) questions about source code. This plan changes the command that
finds where a symbol is defined:

```sh
weaver definitions get --uri file:///work/src/main.rs --position 10:5
```

Today a person at a terminal sees an untranslated, fixed layout:

```plaintext
/work/src/settings/config.rs
  --> 42:8
   |
40 | /// Loads settings.
41 | #[must_use]
42 | pub fn load() -> Settings {
   |        ^ definition
43 |     Settings::default()
44 | }
```

The text ignores the terminal width, has no screen-reader-friendly form, and
cannot be translated. It also copies source lines to the terminal unchanged,
which is a defect: a source file containing terminal escape bytes can move the
cursor, recolour the screen, or set the window title.

After this change, a sufficiently wide UTF-8 terminal shows a summary table
followed by source context. The second context block is elided here:

```plaintext
2 definitions

No.  File                          Line  Column
───  ────────────────────────────  ────  ──────
  1  /work/src/settings/config.rs    42       8
  2  /work/src/settings/legacy.rs     7       5

/work/src/settings/config.rs
  ╭─▶ 42:8
   │
40 │ /// Loads settings.
41 │ #[must_use]
42 │ pub fn load() -> Settings {
   │        ^ definition
43 │     Settings::default()
44 │ }
```

When the table does not fit, labelled blocks replace it. At 24 columns the
first definition is summarized as follows, and context blocks fitted to the
same width come after the summaries:

```plaintext
Definition 1 of 2
File: /work/src/settings
  /config.rs
Line: 42
Column: 8
```

With `--plain`, output is labelled text with the same source lines and no
table, box drawing, caret, colour, pager, or animation:

```plaintext
2 definitions

Definition 1 of 2
File: /work/src/settings/config.rs
Line: 42
Column: 8
Source line 40: /// Loads settings.
Source line 41: #[must_use]
Source line 42 (definition): pub fn load() -> Settings {
Source line 43:     Settings::default()
Source line 44: }
```

`--color never` or `NO_COLOR` removes colour, `--no-pager` prevents paging, and
`--width 60` fixes the width.

The roadmap defines success: "human output does not rely on colour alone and
never emits pager or spinner control flow outside terminal contexts."

Standard output in machine mode (`--output json`, or `--output auto` when
standard output is not a terminal) stays byte-for-byte unchanged. Use
`--output human` to obtain human output through a pipe. Universal `--json`,
structured errors, and bounded collections belong to 13.2.2 and 13.2.3.

Terms used throughout:

- _Terminal context_: the relevant stream is attached to a terminal (a TTY).
  Standard input, output, and error are judged separately.
- _Control flow_ (roadmap wording): starting a pager, moving the cursor,
  erasing a line, or redrawing.
- _SGR sequence_: the `ESC [ … m` escape that selects colour or weight.
- _Cell_: one terminal column. A character may occupy zero, one, or two.
- _Neutralize_: replace a character that a terminal would act upon, or that
  would be invisible, with visible ASCII text such as `<U+001B>`.
- _Fluent_: the `.ftl` message-catalogue format used for translation.
- _Layer_: one source of settings. Precedence, lowest first, is defaults,
  configuration file, environment, command line.
- _Pseudo-terminal_ (PTY): a pair of file descriptors that lets a test stand in
  for a terminal.
- _BDD_: behaviour-driven development; scenarios written as Given, When, Then.
- _Port_ and _adapter_: an interface the program's core owns, and the code that
  connects it to the operating system.
- _Whitaker_: the repository's additional Rust linter.

## Constraints

Hard invariants. Violating one requires escalation, not a workaround.

- C1. Machine standard output and the daemon request must not depend on any
  setting, environment variable, or terminal fact introduced here. How the
  output format is chosen does not change.
  `crates/weaver-cli/tests/golden/request_definitions_get.jsonl` stays as is.
- C2. Do not change `crates/weaverd`, its protocol, or its process model.
- C3. Flags keep ADR 0007's spellings: `--plain`,
  `--color auto|always|never`, `--no-pager`, `--width <columns>`. `--locale`
  already exists, stays before the command, and is not redefined.
- C4. Do not add `--json`, remove `--output`, or add enumerating errors,
  cursors, or truncation protocol fields (13.2.2 and 13.2.3).
- C5. Do not localize protocol values: paths, line and column numbers, URI
  text, capability identifiers, flag names, or JSON.
- C6. No pager and no animation outside terminal contexts, with no override.
- C7. The pager command never comes from a configuration file. Weaver reads
  `weaver.toml` from the working directory, so a file-supplied command would
  let an untrusted repository execute code.
- C8. Use `ortho_config` 0.11.0 with a caret requirement. Do not build a Weaver
  copy of machinery OrthoConfig plans. Locale selection waits for OrthoConfig
  11.3 (D12).
- C9. No Rust file may exceed 400 lines. Neighbours near the limit:
  `src/tests/unit/mod.rs` 396, `command_ir/tests.rs` 391,
  `src/tests/behaviour.rs` 383, `runner_glue.rs` 383, `lib.rs` 378,
  `output/models.rs` 371.
- C10. `crates/weaver-cli/build.rs` compiles `src/cli.rs`,
  `src/command_ir/mod.rs`, `src/command_surface/tree.rs`, `src/help.rs`, and
  `src/help_metadata.rs` through `#[path]`. They may use only crates listed
  under `[build-dependencies]`, and `cli.rs` holds declarations only, because a
  helper unused by the build script is dead code there.
- C11. Tests must not change the process environment or working directory.
  Inject `ortho_config::MapEnv` or `mockable::MockEnv`, or use a child process.
- C12. No `#[allow]`; a suppression needs `#[expect(…, reason = "…")]`. No
  `unwrap`, `expect`, indexing, string slicing, integer division, or lossy
  casts in production code. Functions take at most four parameters.
- C13. No compatibility aliases, dual renderers, or shims. Weaver is
  pre-0.1.0; every caller changes in the same commit.
- C14. `docs/ortho-config-users-guide.md` and the 0.11.0 migration guide are
  byte-identical copies of upstream documents.
- C15. Completed ExecPlans are historical and stay unchanged.
- C16. Every item lands with its first production consumer. The lint gate
  denies dead code, and a flag is never accepted before it works.
- C17. Implementation begins only after the user approves this plan.

## Tolerances (exception triggers)

Comparison base: the merge base with `origin/main`. Count with
`git diff --numstat "$(git merge-base origin/main HEAD)"` and these pathspecs.
Limits are net lines, additions minus deletions, enforced independently.
ExecPlan files are excluded from every count, including file counts.

- Code: `crates/**/*.rs`, `crates/**/*.feature`, and test data under
  `crates/**/tests/data/`.
- Configuration: `Cargo.toml`, `crates/*/Cargo.toml`, `.config/**`,
  `typos.local.toml`, `docs/orthoconfig-consumer-boundary.toml`, and
  `crates/**/*.ftl`.
- Authored documentation: `docs/**/*.md` except the other categories.
- Upstream copies: `docs/ortho-config-users-guide.md` and
  `docs/ortho-config-v0-11-0-migration-guide.md`.
- Generated, uncounted: `Cargo.lock`, `typos.toml`, `**/*.snap`, and
  `docs/orthoconfig-consumer-boundary.md`.

| Milestone | Code estimate | Code limit | Configuration limit | Documentation limit |
| --------- | ------------- | ---------- | ------------------- | ------------------- |
| `EP-M1`   | 60            | 80         | 20                  | 260                 |
| `EP-M2`   | 1,000         | 1,200      | 10                  | 40                  |
| `EP-M3`   | 1,300         | 1,560      | 50                  | 140                 |
| `EP-M4`   | 1,500         | 1,800      | 20                  | 100                 |
| `EP-M5`   | 800           | 960        | 30                  | 80                  |
| `EP-M6`   | 450           | 540        | 10                  | 50                  |
| `EP-M7`   | 700           | 840        | 30                  | 100                 |
| `EP-M8`   | 120           | 150        | 20                  | 160                 |
| Total     | 5,930         | 7,130      | 190                 | 930                 |

_Table 1: Net-line estimates and limits._

Stop and escalate when any of these is reached:

- A milestone or total limit in Table 1, or 700 net lines of upstream copies.
- More than 90 non-ExecPlan files change.
- More than 200 lines of existing `.snap` content change.
- More than 40 Fluent messages are added.
- A public interface or dependency beyond `Interfaces and dependencies` is
  needed.
- A change to `weaverd`, the wire protocol, or the variants or `auto` behaviour
  of `OutputFormat` is needed. Making `OutputFormat::resolve` crate-private is
  expected.
- Moving to `ortho_config` 0.11.0 needs more than the `renderer` field and
  mechanical corrections.
- A gate still fails after three fix attempts.
- A milestone takes more than eight hours of active work, excluding compile
  queueing.
- Two valid readings of a requirement would produce different visible output.

`Budget evidence` explains the numbers. The contingency is 20 per cent for code
because tests, private-item documentation, and lint-driven decomposition were
the main omissions in earlier estimates.

## Risks

- R1. The work overruns. Severity: high. Likelihood: medium. Mitigation: eight
  plateaus with separate limits, each a run of commits that can be reverted.
- R2. Claims of absence pass vacuously because no test reaches a terminal.
  Severity: high. Likelihood: high. Mitigation: `EP-M5` adds a pseudo-terminal
  harness; every absence test has a paired presence test using the same input.
- R3. Foreign text reaches the terminal by an unguarded path. Severity: high.
  Likelihood: medium. Mitigation: one constructor for safe text, one standard
  error chokepoint, and whole-output byte tests including hostile `weaver.toml`
  and hostile daemon errors.
- R4. Fluent's isolation marks break ASCII output and width measurement.
  Severity: medium. Likelihood: certain. Mitigation: one shared temporary
  adapter, whose removal gate is leynos/weaver#452.
- R5. Interrupting the pager leaves the shell's terminal unusable. Severity:
  high. Likelihood: high without care; reproduced in review. Mitigation: D6 and
  a PTY interrupt test.
- R6. An automated session attached to a terminal waits forever for a key.
  Severity: high. Likelihood: medium. Mitigation: paging requires all three
  standard streams to be terminals, a usable `TERM`, and a known height.
- R7. A response or source file is unbounded or blocks. Severity: medium.
  Likelihood: low. Mitigation: caps and regular-file checks in D17 and D20.
- R8. Width measured per character differs from a terminal that shapes joined
  emoji. Severity: low. Likelihood: medium. Mitigation: documented limit.
- R9. Presentation keys collide with `Config` keys. Severity: medium.
  Likelihood: low. Mitigation: a test that the field names are disjoint.
- R10. The host's compile queue slows gates; more than 400 requests were queued
  during planning. Severity: low. Likelihood: high. Mitigation: start each gate
  once and wait.
- R11. The upstream copies could fail formatting or spelling gates. Severity:
  low. Likelihood: low; `mdtablefix --check` and `markdownlint-cli2` passed on
  both during planning. Recovery: stop and ask rather than edit them.

## Progress

- [x] (2026-11-24) Reconnaissance by a five-agent Wyvern team completed.
- [x] (2026-11-24) Initial draft completed (`2cfc1c75`).
- [x] (2026-11-24) Five-reviewer design review applied; each verdict was
  "revise". Four rechecked the revision, and their corrections were applied.
- [x] (2026-11-24) leynos/weaver#451 and leynos/weaver#452 raised at the user's
  request.
- [x] (2026-11-24) Approver's questions answered (D23, D24).
- [ ] Plan approved by the user.
- [ ] `EP-M0` baseline gates and settings probe recorded.
- [ ] `EP-M1` OrthoConfig 0.11.0 and recorded decisions.
- [ ] `EP-M2` safe text at every human sink.
- [ ] `EP-M3` localized labelled output and `--plain`.
- [ ] `EP-M4` width-aware tables, blocks, and `--width`.
- [ ] `EP-M5` colour and `--color`, with the PTY harness.
- [ ] `EP-M6` terminal-sensitive progress.
- [ ] `EP-M7` pager and `--no-pager`.
- [ ] `EP-M8` renderer metadata and closure.

## Surprises & discoveries

- Observation: the configured locale has no effect. Evidence:
  `build_localizer()` pins `en-US` and runs before configuration loads;
  `Config::locale()` has no caller outside `weaver-config` tests. Impact:
  raised as leynos/weaver#451, blocked on OrthoConfig 11.3.1 and 11.3.2.
- Observation: Fluent wraps inserted values in U+2068 and U+2069. Evidence:
  `fluent-bundle-0.16.0/src/bundle.rs:641`. Impact: raised as
  leynos/weaver#452, blocked by leynos/ortho-config#718.
- Observation: OrthoConfig 7.2.2 shipped in 0.11.0 as descriptive metadata.
  Evidence: upstream ADR-0016 lists terminal output, a rendering engine, and
  environment probing as non-goals. Impact: Weaver owns the runtime controls,
  and the boundary row's "reusable human-renderer controls" gate cannot be met
  as written (D15).
- Observation: the present renderer copies foreign text to the terminal, and
  so do error paths: TOML errors echo the offending line of a `weaver.toml`
  found in the working directory, and `AppError::ParseMessage` echoes daemon
  text. Impact: the criterion cannot be met by colour controls alone.
- Observation: a derived `no_pager` argument has clap identifier `no_pager`,
  but `help_metadata.rs` calls `mut_arg` with the long name, and clap panics on
  an unknown identifier. `build.rs` takes the same path. Impact: give the
  argument an explicit `id` (D3).
- Observation: the command tree can describe only required string flags.
  Evidence: `CommandArgument` in `command_surface/tree.rs`, and
  `field_metadata` in `command_ir/mod.rs`. Impact: extended in `EP-M3`.
- Observation: `weaver-cli` installs no `tracing` subscriber, so warnings are
  invisible. Impact: user-relevant fallbacks also print one line.
- Observation: U+17D8 measures three cells in `unicode-width`. Impact: it is
  neutralized, so a piece never exceeds two cells.
- Observation: tabs have no measured width, so carets are misplaced on
  tab-indented lines.
- Observation: `build.rs` watches `src/locales/en-US/messages.ftl`, which does
  not exist. Impact: corrected in its own commit in `EP-M1`.
- Observation: killing `weaver` with the interrupt key while `less` runs leaves
  the terminal without echo. Evidence: reproduced during review. Impact: D6.
- Observation: a blocked socket read with a timeout returns `EINTR` after stop
  and continue. Evidence: reproduced during review. Impact: D19.

## Decision log

Made by the planning agent on 2026-11-24, revised after design review, and
effective on user approval.

- D1. One renderer serves `definitions get` and the legacy
  `observe get-definition`; the four flags exist only on `definitions get`.
  Rationale: both reach the daemon as the same operation, and a second renderer
  would violate C13. The legacy spelling forwards unknown flags to the daemon,
  so it takes settings only from files and the environment.
- D2. Output-format selection does not change. Rationale: an earlier draft made
  a human flag select human output when redirected. Scripts pass `--no-pager`
  and `--color never` defensively, so that would have changed the format of
  existing JSON pipelines. `--output human` already exists, and 13.2.2 replaces
  the mechanism.
- D3. Settings live in `weaver_config::PresentationConfig`, separate from
  `Config`, with the same prefix and discovery. It is loaded only when human
  output is selected for this operation, before connecting. Rationale: fields in
  `Config` would be forwarded to `weaverd`, and a bad colour would prevent
  `weaver daemon start`. Its derived flags are disabled with `skip_cli`; clap
  owns the flags, with `id = "no-pager"` stated explicitly.
- D4. `plain` removes all styling. Otherwise only `--color always` given on the
  command line forces styling over `NO_COLOR` or outside a terminal. In every
  other case styling needs a terminal on standard output, an empty or absent
  `NO_COLOR`, a usable `TERM`, and a colour other than `never`. `always` from a
  file or the environment therefore behaves as `auto`. Rationale: ADR 0007
  disables styling under `NO_COLOR` and outside terminals, while design §2.1.3
  allows explicit forcing. A file may come from an untrusted repository. Both
  documents are amended to state the one rule.
- D5. Nothing forces a pager or animation outside terminal contexts. Rationale:
  the success criterion is absolute; design §2.1.3 is amended to match.
- D6. The pager is `PAGER`, or `less` when `PAGER` is unset, split on ASCII
  whitespace without quoting and run without a shell. Empty or `cat` disables
  it. When `LESS` is unset the child receives `LESS=FRX`: quit if one screen,
  pass SGR only, and do not clear the screen. From before the pager starts
  until it has been waited for, Weaver holds handlers for the interrupt and
  quit signals. They are registered conditionally, so that the default action
  is restored afterwards, and the signals are not ignored, because an ignored
  signal would be inherited. `NotStarted` (spawn failure, or exit status 126 or
  127) writes the text directly and, unless the missing program is the default
  `less`, prints one line on standard error. `Failed` (another non-zero status
  or another signal) prints one line naming `--no-pager` and does not write the
  text again. `Delivered` is success, including a pager that closes its input
  early or ends by an interrupt, quit, or broken-pipe signal. No outcome
  changes Weaver's exit status. Paging happens only when the response completed
  with status zero.
- D7. Progress appears only while waiting for the first response byte, only on
  standard error, and only when it is a terminal. It is animated on a capable
  terminal and one static line when `plain` is set, `TERM` is unusable, or the
  width is unknown. Otherwise, and in JSON mode, there is none. The existing
  `Waiting for daemon start...` line is unchanged plain text for every command.
  Rationale: `docs/ui-gap-analysis.md` requires a terminal on standard error.
  `lifecycle/` also serves `daemon start`, and a starting daemon inherits
  standard error, so animating there could interleave.
- D8. Unicode decoration needs a terminal on standard output, a UTF-8 locale
  (`LC_ALL`, else `LC_CTYPE`, else `LANG`), a usable `TERM`, and `plain` off.
  Otherwise decoration is ASCII. Message text follows the catalogue.
- D9. Width is the command line, then the settings layers, then the terminal
  attached to standard output, then a parseable `COLUMNS`, then 80. Outside a
  terminal, and in plain mode, output is unbounded unless a width is given
  explicitly. Explicit widths are 20 to 65,535; smaller detected widths count
  as 20. Rationale: wrapping paths harms speech, braille, copying, and `grep`.
- D10. A summary table with headings precedes source-context blocks. If it does
  not fit, labelled blocks replace the table and context blocks remain, fitted
  to the width. Plain mode uses labelled blocks whose context lines carry the
  same source text with spoken labels, so nothing is lost to a screen reader.
- D11. Foreign text becomes `SafeText` one line at a time; line feeds and
  carriage-return line feeds separate lines first. Neutralized characters are
  category Cc; category Cf except U+200C, U+200D, and the
  Prepended_Concatenation_Mark characters; U+2028 and U+2029; any character
  wider than two cells; and, after four in a row, further zero-width
  characters. Tabs expand to multiples of four cells. Rationale: covers escape,
  bidirectional, invisible, and tag characters without harming scripts that
  need joiners.
- D12. Every human message resolves through the injected
  `ortho_config::Localizer` with an English fallback. Choosing the locale is
  not part of this work. Rationale: OrthoConfig plans the lifecycle as 11.3.1
  and 11.3.2, C8 forbids a Weaver copy, and the user asked for this to be
  tracked as blocked work. It is leynos/weaver#451. Tests inject a
  pseudo-locale to prove that translation works. The isolation marks are
  removed by one shared helper, moved from `discoverability.rs` to
  `localizer.rs`, until leynos/weaver#452 retires it.
- D13. No Verus or Kani milestone; see `Formal verification decision`.
- D14. The OrthoConfig upgrade is its own milestone and commit.
- D15. The boundary row becomes `consumes`, `shipped_in = "v0.11.0"`, for
  renderer metadata. A `consumes` row must leave the gate and anchor fields
  empty, so ADR 0007 itself records that runtime controls are Weaver-owned.
- D16. New dependencies: `terminal_size` and `signal-hook` at run time, and
  `rustix` for tests. Reasons are in Table 7.
- D17. In human mode this operation collects all standard-output text until the
  exit message, concatenates it, then renders and delivers it. Reading stops at
  the exit message, as the protocol allows. The collected text is capped at 16
  MiB; beyond it the held text is written neutralized and streaming continues
  unpaged. A single response line is read through a 64 MiB limit, sized for
  JSON escaping, and a longer line is a read error.
- D18. `IoStreams::new` takes `HostFacts` instead of a boolean, whose default
  has no terminal and no pager, so no in-process test can start a real pager.
- D19. When progress may appear, a 120-millisecond read timeout is set before
  the request is sent and is never cleared. The reader treats `WouldBlock`,
  `TimedOut`, and `Interrupted` as ticks throughout the response. If the
  timeout cannot be set, Weaver waits without progress. Machine mode never sets
  it. No thread is used. Rationale: changing a socket option after the peer has
  closed can fail on macOS, a release target, and must not fail a complete
  response.
- D20. Source files are opened without blocking, must be regular files, and are
  read through a limit of 16 MiB on the open handle, because reported sizes can
  be zero or stale; otherwise a localized note explains why. Each distinct path
  is read at most once, and only displayed lines are neutralized.
- D21. Invalid values in any file or environment layer are errors naming the
  setting, the accepted values, and the layer or file, even if a higher layer
  would override them. Booleans accept `true`, `false`, `1`, and `0` in every
  layer, through one parser shared with the flags.
- D22. Reject a table, pager, spinner, colour, or `miette` dependency. Design
  §2.1.10 permits a renderer modelled on `miette`. It would replace only the
  context blocks, emits fixed English in its narrated form, and would need
  neutralized spans remapped.
- D23. The user confirmed D1, D4, D6, D12, D13, and D16, and the reach of
  `EP-M2`, on 2026-11-24. The contingent Verus milestone was removed.
- D24. Deliver every milestone in one pull request, as the user directed on
  2026-11-24. The per-milestone limits in Table 1 still apply and still stop
  the work at the milestone that overruns. Date/Author: 2026-11-24, user.

## Outcomes & retrospective

To be completed during implementation.

## Context and orientation

Inspected base revision: `fe56d226`. Relocate symbols with
`grep -rn "<symbol>" crates/`.

### How the command works today

`crates/weaver-cli/src/main.rs` tests whether standard output is a terminal and
builds `IoStreams`. `run` in `src/lib.rs` builds a localizer and calls
`run_with_handler`, which splits leading configuration flags from the rest
(`split_config_arguments` in `config.rs`), parses the remainder into `Cli`
(`cli.rs`), runs `handle_preflight`, loads `weaver_config::Config` through the
`ConfigLoader` trait, resolves `--output`, and calls `execute_daemon_command`
(`runner_glue.rs`).

`execute_daemon_command` connects, starting the daemon if necessary
(`lifecycle/utils.rs`), writes one JSON line, and calls `read_daemon_messages`
(`daemon_output.rs`). Each response line is a stream message or an exit status.
In human mode each payload, from either stream, passes through the public
function `render_human_output` (`output/mod.rs`), which returns `None` for
anything unrecognized; raw text is then written. Its other callers are three
unit tests in `output/tests.rs` and the ignored
`crates/weaver-e2e/tests/human_output.rs`.

A definitions payload is a JSON array such as
`[{"uri":"file:///work/a.rs","line":42,"column":8}]`. Lines and columns start
at one; columns count UTF-16 code units. Daemon failures for this operation are
plain text on standard error.

`output/source.rs` maps a URI to a path. `output/render.rs` reads the file with
`cap_std` and draws the context block for definitions, references, diagnostics,
and verification failures. `map_result_to_exit_code` (`lib.rs`),
`write_error_and_fail` (`runner_glue.rs`), and `write_startup_guidance`
(`actionable_guidance.rs`) print errors.

### Command metadata

`command_surface/tree.rs` is the hand-written command tree. `command_ir/mod.rs`
projects it into `ortho_config::docs::DocMetadata`, which drives help and the
manual page. `command_ir/tests/canonical_projection.rs` is the independent
expectation. Both build `DocMetadata` by literal and so need 0.11.0's
`renderer` field. Nothing detects a flag that exists only in clap.

### Abstraction sweep

Searches for existing equivalents found: `strip_bidi_isolates` (to be moved and
shared), `caret_display_offset` and `num_digits` in `render.rs` (replaced),
`OutputSettings` and `ConfigLoader` (extended), and two fake daemons: one
inside the `weaver-cli` unit tests and one in
`crates/weaver-e2e/tests/test_support/daemon_harness.rs` (the latter extended
with a delay). There is no existing terminal probe, table, wrapping, styling,
pager, progress, or PTY helper.

### Test facilities

Workspace dependencies: `rstest` 0.28, `rstest-bdd` 0.5, `insta`, `proptest`,
`mockable`, and `mockall` 0.15. `googletest` 0.14 and `pretty_assertions` 1.4
are used only by `weaver-config`. CLI behavioural steps are in
`src/tests/behaviour.rs`, whose `OutputStream` and `OutputCheck` stay private
until a second behavioural module needs them (developers' guide §21.6); `EP-M3`
is that moment. Each scenario needs an explicit named binding.

### Gates

| Command             | What it runs                                                          |
| ------------------- | --------------------------------------------------------------------- |
| `make check-fmt`    | `cargo fmt --check`, then `mdtablefix --check`                        |
| `make lint`         | `cargo doc`, Clippy with warnings denied, Whitaker, Python docstrings |
| `make test`         | `cargo nextest run` over the workspace, then doctests                 |
| `make markdownlint` | `make spelling`, then `markdownlint-cli2`                             |
| `make nixie`        | Mermaid validation                                                    |
| `make fmt`          | Formats Rust and Markdown                                             |

_Table 2: Repository gates._

Clippy limits functions to four parameters, 70 lines, and four nesting levels;
Whitaker limits cognitive complexity to nine. Examples in `weaver-config` do
not run as doctests, so examples there are also exercised by unit tests. Kani
0.67 and `cargo-mutants` are on the host but not configured; Verus is absent.
CodeGraph was unavailable while the plan was drafted. It was later used to
confirm the callers of `render_human_output`, `wait_for_ready`, and
`strip_bidi_isolates`.

### Budget evidence

| Plan                       | Limit set             | Outcome                                     |
| -------------------------- | --------------------- | ------------------------------------------- |
| 12.1.2                     | 25 files, 1,200 lines | Squash `41abe94d`: 35 files; net code 1,441 |
| Adopt `ortho_config` 0.9.0 | 12 files, 500 lines   | Paused on an unhealthy baseline             |
| Issue 368, 0.10.0          | None numeric          | Code net 362; upstream guides net 1,900     |

_Table 3: Recorded budgets and outcomes of comparable plans._

Plan 12.1.2 also recorded a pull-request diff of 116 files and 7,797 additions
as a breach noted only afterwards. Both figures exceeded a limit that had
counted neither tests nor documentation. Here, the first draft's estimate was
raised from 4,420 to 5,930 after review because it had under-counted BDD steps,
property tests, private-item documentation, and the PTY harness (about 250
lines within `EP-M5`). The 0.9.0 plan shows why `EP-M0` records the baseline.

### Documents and skills to consult

Documents: `docs/weaver-design.md` §§2.1.3, 2.1.7, 2.1.10, 2.1.11, and 2.3.1;
`docs/adrs/adr-0007-agent-native-command-surface.md`;
`docs/adrs/adr-0005-verification-trust-boundary.md`;
`docs/formal-verification-methods-in-weaver.md`; `docs/ui-gap-analysis.md`;
`docs/ortho-config-users-guide.md`; `docs/rust-testing-with-rstest-fixtures.md`;
`docs/rstest-bdd-users-guide.md`; `docs/rust-doctest-dry-guide.md`;
`docs/reliable-testing-in-rust-via-dependency-injection.md`;
`docs/complexity-antipatterns-and-refactoring-strategies.md`;
`docs/documentation-style-guide.md`; and `docs/developers-guide.md` §§15 and 21
with its command-addition checklist.

Skills: `sdlc-implementation`, `execplans`, `rust-router`, then
`domain-cli-and-daemons`, `rust-types-and-apis`, `rust-errors`,
`rust-unit-testing`, `proptest`, and `rust-verification`;
`hexagonal-architecture`; `arch-decision-records`; `localization-docs`;
`addressing-whitaker-findings`; `nextest`; `en-gb-oxendict-style`;
`commit-message`; `pr-creation`; and `pr-babysitting`.

## Conformance basis

There is no Terms of Reference. Upstream artefacts at `fe56d226`:
`docs/roadmap.md` 13.2.1; `docs/weaver-design.md` §§2.1.3, 2.1.10, 2.1.11; ADR
0007 (Proposed), "Human renderer contract"; ADR 0005; and
`docs/orthoconfig-consumer-boundary.toml` row 13.2.1. External: OrthoConfig
v0.11.0 ADR-0016, and OrthoConfig roadmap 11.3 at `cbba0d03`.

- `REQ-FLAGS`: `definitions get` accepts the four flags.
- `REQ-LAYERED`: their settings layer through OrthoConfig.
- `REQ-L10N`: renderer prose resolves through the localizer.
- `REQ-TABLE`: tables have headings.
- `REQ-NARROW`: narrow output uses labelled blocks.
- `REQ-ASCII`: Unicode decoration has an ASCII fallback.
- `REQ-COLOUR`: meaning never depends on colour alone.
- `REQ-CONTEXT`: no pager or spinner control flow outside terminal contexts.
- `REQ-STREAMS`: results go to standard output; progress and diagnostics to
  standard error.
- `REQ-BLOCKS`: code locations show source context grouped by file.
- `REQ-META`: the renderer is declared through OrthoConfig 7.2.2 metadata.
- `GAP-DEP`, `GAP-ESCAPE`, `GAP-WIDTH`, `GAP-PTY`: the base uses 0.10.0, emits
  foreign controls, ignores width, and has no PTY harness.

```plaintext
GAP-DEP                                 -> EP-M1 -> canonical projection on 0.11.0
GAP-ESCAPE                              -> EP-M2 -> output::text, hostile_text e2e
REQ-L10N, REQ-LAYERED                   -> EP-M3 -> messages, presentation_layers
REQ-TABLE, REQ-NARROW, REQ-ASCII,
REQ-BLOCKS, GAP-WIDTH                   -> EP-M4 -> definitions snapshots, properties
REQ-COLOUR, GAP-PTY                     -> EP-M5 -> erasure property, terminal_contexts
REQ-STREAMS, REQ-CONTEXT (progress)     -> EP-M6 -> terminal_contexts::progress
REQ-FLAGS, REQ-CONTEXT (pager)          -> EP-M7 -> terminal_contexts::pager
REQ-META                                -> EP-M8 -> canonical projection
```

Scope of `REQ-L10N`: headings, labels, notes, counts, progress text, pager
notices, and renderer fallbacks. Outwith scope and still English: clap errors,
`AppError` text, startup guidance, daemon diagnostics, and URI failure reasons
supplied as error values. Those belong to 13.2.2, 13.3.3, and leynos/weaver#451.

## Verification plan

Claims were written before the interfaces and changed them: output is a typed
document with colour applied last; foreign text is a distinct type; explicit
widths have a floor; escapes are single-cell pieces that can wrap; every line
passes through the fitting functions; "may page" is separate from "long enough
to page"; and the pager reports an outcome instead of a bare error.

### Abstract model

```plaintext
Settings = { plain, no_pager: Bool, colour: Auto|Never|Forced,
             width: None | 20..=65535 }
Facts    = { in_tty, out_tty, err_tty, dumb, no_color, utf8: Bool,
             out_size: None | (columns, rows), err_columns: None | 1..=65535,
             env_columns: None | 1..=65535, pager_present: Bool }
Format   = Human | Json
Policy   = { styled, unicode, plain, may_page: Bool,
             width: Unbounded | Bounded(20..=65535),
             progress: None | Static | Animated }
probe    : Environment x Descriptors -> Facts
resolve  : Settings x Facts x Format -> Policy      (total, pure)
```

The resolved command-line value `always` becomes `Forced`; `always` from any
lower layer becomes `Auto`. `pager_present` is a boolean so that policy never
sees a command.

A _piece_ is the smallest unit layout may move: 0, 1, or 2 cells, a short safe
string, and a range of source UTF-16 units. The display width of a line is the
sum of per-scalar widths, not `UnicodeWidthStr`.

Representation: a `SafeText` is the concatenation of its pieces' text. Piece
ranges partition the source line, except that the pieces of one escape or one
tab share that character's range. A document is lines of spans, each pairing a
role with safe text.

Malformed inputs: a file that is not UTF-8, regular, or within the cap yields a
note; an unparseable payload is written neutralized with a one-line notice; an
invalid setting is rejected before connecting.

### Invariants and lemmas

- `INV-PROBE`: facts follow the environment as D4, D8, and D9 state. `dumb`
  holds when `TERM` is absent, empty, or `dumb`; an empty `NO_COLOR` counts as
  absent; unparseable or zero sizes are `None`; only standard output's own
  descriptor supplies `out_size`; `pager_present` follows D6 for `PAGER` unset,
  empty, `cat`, and blank; and colour is Forced ⇔ the command-line override is
  `always`.
- `INV-POLICY`:
  - styled ⇔ ¬plain ∧ (colour = Forced ∨ (colour = Auto ∧ out_tty ∧ ¬no_color
    ∧ ¬dumb));
  - unicode ⇔ ¬plain ∧ out_tty ∧ utf8 ∧ ¬dumb;
  - may_page ⇔ Human ∧ in_tty ∧ out_tty ∧ err_tty ∧ ¬dumb ∧ ¬plain ∧
    ¬no_pager ∧ rows known ∧ pager_present;
  - progress = None ⇔ Json ∨ ¬err_tty; Animated ⇔ Human ∧ err_tty ∧ ¬dumb ∧
    ¬plain ∧ err_columns known; otherwise Static.
- `INV-SIZE`: width is explicit if given; else Unbounded if plain or ¬out_tty;
  else the first known of terminal columns and `COLUMNS`, raised to 20; else 80.
- `INV-INDEPENDENT`: styling, glyphs, and width do not depend on err_tty or
  in_tty; progress does not depend on colour or standard output's facts; paging
  and glyphs do not depend on colour or no_color.
- `INV-SAFE`: whatever the payload, source bytes, configuration file, daemon
  text, and policy, bytes Weaver writes in human mode to standard output, and
  to standard error outside progress, contain no unsafe scalar except line feed
  and, when styled, the sequences in Table 4. Standard error written by an
  inherited child process is outside the claim.
- `INV-ERASE`: removing SGR sequences from styled output gives exactly the
  unstyled output.
- `INV-WIDTH`: under a bounded width every output line's display width is at
  most that bound.
- `INV-CONSERVE`: for a non-empty payload, in order, each layout contains every
  definition's neutralized path, line, and column; removing exactly the
  two-cell continuation indent and joining recovers each value. Grouping uses
  the raw path, not its display form.
- `INV-CONTEXT`: for each definition, plain mode prints the lines of that
  definition's unclipped context block, in order, and marks the target in words.
- `INV-SELECT`: the table is used ⇔ ¬plain, the payload is non-empty, and the
  natural table fits; Unbounded always fits; a table has its heading row.
- `INV-CARET`: the caret's column is that of the first piece representing the
  target unit, or the cell after the last piece when the target is at or beyond
  the end, and is inside the visible window.
- `INV-LAYER`: each effective setting is that of the highest layer supplying
  it.
- `INV-MESSAGE`: `en-US` defines every message and formats it without error; an
  injected localizer changes prose and never a protocol value.
- `INV-JSON`: machine standard output and the request do not depend on
  settings or facts.
- `INV-PAGER-TRUST`: no file layer can influence the executed program.
- `INV-DELIVER`: collected text is handed to exactly one sink; a second sink is
  used only after `NotStarted`; the pager is never called unless the exit
  status is zero; nothing is dropped when the stream ends in success, failure,
  missing exit, malformed message, read error, or the empty-line limit.

Lemmas, stated as hypotheses about the eventual argument:

- `L-POINT`: the neutralization of every scalar contains no unsafe scalar and
  consists of pieces of at most two cells.
- `L-CONCAT`: with s = (column, zero-width run) and δ the state after a
  prefix, n(s, x·y) = n(s, x) · n(δ(s, x), y). It is tested on an internal
  `neutralize_from(s, line)`.
- `L-IDEM`: neutralizing the text of neutralized text changes nothing. Needed
  because formatted messages are neutralized again.
- `L-PARTITION`: `wrap(widths, first, rest)`, with every width at most the
  smaller limit, returns consecutive ranges covering the input once; range sums
  respect their limits; each is greedy-maximal; no range after the first begins
  with a zero-width piece; empty input gives no range.
- `L-WINDOW`: `window(widths, focus, limit)`, where 0 ≤ focus ≤ len and every
  width is at most limit − 1, returns r such that r.start ≤ focus ≤ r.end; the
  pieces before the focus, with one cell for the caret, total at most limit; r
  totals at most limit; and focus < r.end whenever focus < len, so the target
  itself is visible. r is the whole input when that fits with the caret;
  otherwise no strict superset range satisfies the other clauses. r.start is
  not a zero-width piece unless it is 0.
- `L-GUTTER`: a gutter is at most 13 cells (ten digits and three), so at the
  floor seven remain, enough for one clip marker at each end and a two-cell
  piece.

Proof sketch. `L-POINT` is a finite check. `L-CONCAT` follows by induction on
x, because the only state is the column and the zero-width counter. `INV-SAFE`
then follows structurally: `SafeText` has one constructor for foreign text,
`Document` holds only `SafeText`, and `paint` adds only line feeds and Table 4
sequences. `INV-ERASE` is structural induction over spans. `L-PARTITION` and
`L-WINDOW` are induction on the sequence with the running sum as the invariant.
`INV-WIDTH` composes them with `L-GUTTER` and two layout rules: every line,
including headings, notes, location headers, and caret labels, passes through
`wrap`; and a label leaving fewer than seven cells puts its value on the next
line. A caret label that does not fit moves to its own line. Clip markers take
one cell each and are counted before windowing; context lines use the target
line's cell offset.

### Axioms (trusted, not verified here)

- `AXIOM-TTY`: `IsTerminal` and `terminal_size_of` describe the descriptor.
- `AXIOM-WIDTH`: `unicode-width` per-scalar widths; see R8.
- `AXIOM-FLUENT`: Fluent changes argument values only by isolation marks.
- `AXIOM-MERGE`: OrthoConfig composes layers in documented order. Our appended
  command-line layer is tested against the real crate.
- `AXIOM-PAGER`: a pager reads standard input and emits control sequences only
  to its terminal. Its display is not verified.

### Methods per obligation

Test style: `pretty_assertions` for equality of policies, documents, and whole
outputs; `googletest` matchers inside `rstest` cases where several facts about
one value matter; `insta` for layouts across widths and glyph sets. Loops over
many cases collect the first counterexample instead of running a matcher per
case. Negative controls are durable: checker functions also run against
deliberately faulty test doubles and must reject them.

- `INV-POLICY`, `INV-INDEPENDENT`. Exhaustive enumeration of 12,288
  combinations. Oracle: the formulae above, written as predicates in the test.
  Artefact: `src/presentation/policy_tests.rs`. Evidence:
  `cargo nextest run -p weaver-cli presentation`; red because `resolve` is
  absent. Non-vacuity: every outcome occurs at least once; a double that ignores
  `out_tty` is rejected. Discharge: all combinations agree.
- `INV-SIZE`. Every column value 0 to 65,535 and absence, against explicit
  widths None, 20, 80, and 65,535, both values of plain, both terminal states,
  and `COLUMNS` absent, malformed, and 100. Artefact and command as above.
- `INV-PROBE`. `rstest` table over `MockEnv` and descriptor doubles.
  Non-vacuity: each rule has a positive and a negative row.
- `L-POINT`, `L-IDEM`. All 1,112,064 scalars. Oracle: a checked-in list of
  category members from Python's `unicodedata` 15.0.0, with the 13
  Prepended_Concatenation_Mark scalars listed separately: 65 Cc, and 155 of 170
  Cf neutralized. Artefact: `src/output/text/tests.rs`. Non-vacuity: exact
  counts; a double omitting U+202E is rejected. `L-CONCAT`, and `L-IDEM` for
  runs: `proptest` over strings and split points, with tabs and zero-width runs
  required by classification.
- `L-PARTITION`, `L-WINDOW`, `L-GUTTER`. All 9,841 sequences to length eight
  over widths {0, 1, 2}, every focus including the end, limits 3 to 12, then
  `proptest` to length 2,000. Artefact: `src/output/text/fit_tests.rs`. Beyond
  the bound the claim is sampled, not proved. Non-vacuity: empty, whole,
  left-clipped, right-clipped, and both-clipped results occur; off-by-one and
  single-piece doubles are rejected.
- `INV-SAFE`, `INV-ERASE`, `INV-WIDTH`, `INV-CONSERVE`, `INV-CONTEXT`,
  `INV-SELECT`, `INV-CARET`. `proptest` over payloads, sources, and policies,
  with independent SGR removal and width summing. Generators emit escapes, C1,
  bidirectional and tag characters, tabs, wide and combining characters, empty
  files, out-of-range positions, and long paths, without `prop_assume!`.
  Artefact: `src/output/definitions/properties.rs`. Non-vacuity: witness tests
  for every class; faulty doubles (colour in text, missing heading, byte-based
  caret, dropped continuation) are rejected.
- `INV-SAFE` end to end: `crates/weaver-e2e/tests/hostile_text.rs` with a
  hostile source, `weaver.toml`, daemon message, and malformed response.
- `INV-LAYER`, D21. All eight presence subsets of file, environment, and
  command line, for four settings and both polarities, with `MapEnv`. Artefact:
  `crates/weaver-config/tests/presentation_layers.rs`. Non-vacuity: a composer
  with two layers swapped is rejected; invalid values in each layer are
  reported with provenance, including when overridden.
- `INV-MESSAGE`. Parameterized over message identifiers: `en-US` resolves
  without fallback or reported issue; a pseudo-locale differs in every message.
  Artefact: `src/output/messages/tests.rs`.
- `INV-JSON`. `rstest` matrix over settings and facts, comparing bytes.
  Non-vacuity: the same matrix in human mode yields differing output.
- `INV-DELIVER`. `rstest` table of six stream endings and three pager outcomes
  with `mockall`, plus real pagers in `terminal_contexts.rs`. Non-vacuity: a
  double that re-sends after `Failed` is rejected.
- `REQ-CONTEXT`, `INV-PAGER-TRUST`. Table 5, end to end.
- D20 cost: a `mockall` source provider asserts one read per distinct path for
  1,000 definitions.

Keep `proptest` regression files in version control. No property uses a shared
environment or the network.

### Formal verification decision

No lemma is selected for Verus, and Kani is not introduced. `AGENTS.md` asks
for an exhaustive Verus proof of introduced lemmas or contractual business
logic; the task brief asks for selective use and excludes formatting and
plumbing. The plan reads the rule through that criterion:

- per-scalar neutralization and the boolean kernel of `resolve` are enumerated
  in full, and width resolution over the stated classes, so a proof would add
  little but a mirror to maintain;
- `wrap` and `window` are unbounded layout arithmetic. They are checked
  exhaustively to a stated bound and sampled beyond it. They are not proved,
  and a fault there could hide or misplace text.

ADR 0005's verified kernel does not include rendering, roadmap 19.3.1 owns the
pinned tools and depends on 16.1.1, and 19.3.4 wants abstractions that have
already proved stable. Kani over the same bounds would repeat the enumeration.

The approver agreed to omit a Verus milestone if it adds no value. It adds none
here: a proof would cover a specification mirror of `wrap` and `window`, not
production `fit.rs`, so correspondence would still rest on review and the
bounded enumeration. If these functions later become shared kernels, 19.3.4 is
the place to prove them, using the lemma statements above.

### Suggested BDD features

Feature file:
`crates/weaver-cli/tests/features/weaver_cli_human_renderer.feature`, bound by
`crates/weaver-cli/src/tests/behaviour_human.rs`, run with
`cargo nextest run -p weaver-cli behaviour_human`. Each comment names the
milestone in which the scenario is added.

```gherkin
Feature: Localized human output for definitions get

  # Steps append --uri and --position. Background stores the payload; each When
  # starts its own fake daemon, which accepts one connection.
  Background:
    Given a fake daemon prepared with two definitions

  # EP-M3
  Scenario: Plain output is labelled text
    Given standard output is a terminal
    When the operator runs "definitions get --plain"
    Then stdout contains "Definition 1 of 2"
    And stdout contains "(definition): "
    And stdout is ASCII and contains no escape character

  # EP-M3
  Scenario: A translated catalogue changes prose but not values
    Given standard output is a terminal
    And the pseudo-locale localizer is injected
    When the operator runs "definitions get --plain"
    Then stdout contains the pseudo-localized line label
    And stdout contains the unmodified definition path

  # EP-M3
  Scenario: No definitions were found
    Given the fake daemon emits no definitions
    And standard output is a terminal
    When the operator runs "definitions get --plain"
    Then stdout contains "No definitions found."
    And the CLI exits with code 0

  # EP-M3
  Scenario: Source text cannot be read
    Given the first definition names a missing file
    And standard output is a terminal
    When the operator runs "definitions get --plain"
    Then stdout contains a localized note for that definition
    And stdout contains "Definition 2 of 2"

  # EP-M3
  Scenario: The daemon reports a failure
    Given the fake daemon emits an error with status 1
    And standard output is a terminal
    When the operator runs "definitions get --plain"
    Then stderr contains the daemon message on its own line
    And the CLI exits with code 1

  # EP-M3
  Scenario: The response ends without an exit message
    Given the fake daemon emits a definition and closes
    And standard output is a terminal
    When the operator runs "definitions get --plain"
    Then stdout contains "Definition 1 of 1"
    And the CLI exits with code 1

  # EP-M4
  Scenario: A wide terminal shows a headed table and context
    Given standard output is a terminal 100 columns wide
    When the operator runs "definitions get"
    Then stdout contains a heading naming file, line, and column
    And stdout contains "^ definition"

  # EP-M4
  Scenario: A narrow terminal shows labelled blocks
    Given standard output is a terminal 24 columns wide
    When the operator runs "definitions get"
    Then stdout contains "Definition 1 of 2"
    And no stdout line is wider than 24 columns

  # EP-M4
  Scenario Outline: An invalid width is rejected before connecting
    Given the fake daemon is expected to stay idle
    When the operator supplies width "5" through <source>
    Then stderr names "<source>" and the accepted range
    And the fake daemon stayed idle

    Examples:
      | source           |
      | --width          |
      | WEAVER_WIDTH     |
      | the file's path  |

  # EP-M5
  Scenario: NO_COLOR is respected
    Given standard output is a terminal 100 columns wide
    And NO_COLOR is set
    When the operator runs "definitions get"
    Then stdout contains no escape character

  # EP-M5
  Scenario: An explicit flag overrides NO_COLOR
    Given standard output is a terminal 100 columns wide
    And NO_COLOR is set
    When the operator runs "definitions get --color always"
    Then stdout contains a colour sequence

  # EP-M5
  Scenario: JSON output ignores presentation settings
    Given standard output is a terminal 100 columns wide
    When the operator runs "--output json definitions get --plain --color always"
    Then stdout is the daemon payload unchanged
```

## Plan of work

Each milestone is developed red, green, refactor. "Refactor" means rerunning
the focused tests after tidying, then the gates in `Concrete steps`, then
committing. Documentation for a user-visible change lands in its milestone.

### `EP-M0` baseline and probe (no product code)

Run the gates on the branch and record results. If one fails, record the log
and ask. Confirm with a throwaway crate outside the repository that a
`PresentationConfig` shaped as below shares files and prefix with `Config`,
that appending `MergeLayer::cli` inside `into_merge_result` wins, that unknown
keys are ignored, and that `skip_cli` removes its flags. Review supported these
by reading the v0.11.0 source only; see `Artefacts and notes`. If any claim
fails, revise D3 before continuing.

### `EP-M1` OrthoConfig 0.11.0 and recorded decisions

Discharges `GAP-DEP`. Raise the workspace requirement and update only the
OrthoConfig crates in the lock file. Name `renderer: None` in both literals and
add `renderer` to `assert_canonical_options`. In a separate commit correct the
`build.rs` watch path. Copy the two upstream guides from tag `v0.11.0`, verify
the digests in `Artefacts and notes`, and update the version statements in
`docs/developers-guide.md` §§15.1 and 15.2, `docs/weaver-design.md` §2.3.1,
`docs/rust-extricate-actuator-plugin-technical-design.md`, and
`docs/contents.md`. Write
`docs/adrs/adr-0014-human-renderer-presentation-policy.md` (status Proposed)
for D3 to D11 and D17 to D22, with the abstraction sweep, reuse policy, and
proof-driven interface decisions, and amend ADR 0007 for D4, D5, D8, and D15,
and design §§2.1.3 and 2.1.10 for D4, D5, and D8.

Red: the version change alone fails with a missing `renderer` field. Green: add
it. Accept: `cargo nextest run -p weaver-cli command_ir` and the configuration
suites pass; help snapshots are unchanged.

### `EP-M2` safe text at every human sink

Discharges `GAP-ESCAPE`. Add `output/text/mod.rs` (`SafeText`, `Piece`,
`neutralize`, `neutralize_lines`) and `output/document.rs` (`Span`, `Line`,
`Document`, unstyled painting). Rework `render.rs` into one context builder
used by all its callers, fixing tab carets. Convert
`render_capability_resolution` and `render_unknown_operation`. Neutralize
forwarded daemon text in human mode by line, and errors at
`map_result_to_exit_code`, `write_error_and_fail`, and
`write_startup_guidance`. Render only standard-output payloads as definitions.

Red: a test shows an escape byte from a source file reaching output; another
shows it from `weaver.toml`. Green: the modules above. Accept:
`cargo nextest run -p weaver-cli output::` and
`cargo nextest run -p weaver-e2e --test hostile_text`. This affects references,
diagnostics, and other human output: only unsafe characters and misplaced
carets change. Users' guide: one paragraph on visible escapes.

### `EP-M3` localized labelled output and `--plain`

Discharges `REQ-L10N` and `REQ-LAYERED`. Add
`weaver-config/src/presentation.rs` with `plain` only. Extend `ConfigLoader`
with `load_presentation`; most test loaders return fixed settings, and one calls
`load_layered` with a `MapEnv` and a temporary file for the layer scenarios,
so no test reads the host. A helper in `config.rs` extracts `--config-path`, in
both spellings, from the configuration arguments; it is tested beside every
other configuration flag, which the derived parser would reject. Extend
`CommandArgument` with a value kind, beginning with string and optional-value
boolean; `EP-M4` adds integer and `EP-M5` adds enumeration with possible
values, so no variant precedes its first argument (C16). Add a default; project
them; show optional flags in brackets in `command_signature`; and compare clap
arguments with the tree in both directions, including arity and values. Add
`HumanRenderArgs` with `--plain[=BOOL]` to `cli.rs`.

Move `strip_bidi_isolates` to `localizer.rs` as the shared temporary adapter.
Add `output/messages.rs`, one enumeration of messages with identifiers and
English fallbacks; arguments are `SafeText`, and counts select plurals. Add
`output/definitions/` with the labelled layout and `SourceProvider`. Split
`read_daemon_messages` into collect, render, and deliver (D17), replacing the
format in `OutputSettings` with `OutputMode`: `Json`, `Human` for other
operations, or `Definitions(&HumanRenderer)`. Settings, policy, progress, and
paging apply only to the third. Make `render_human_output` crate-private and
move the end-to-end test to the real binary and `FakeDaemon`. Move `CliRunner`
and `IoStreams` out of `lib.rs`. Make `OutputStream` and `OutputCheck` shared
test support, and give the in-crate fake daemon a "stays idle" outcome, because
waiting for an absent connection now fails after two seconds.

Red: layer, message, projection, and `EP-M3` BDD tests. Green: the above.
Accept: `cargo nextest run -p weaver-config presentation`, then
`cargo nextest run -p weaver-cli`. Documentation: `--plain`, `WEAVER_PLAIN`,
the file key, and their place in root after-help and the manual page, so a
screen-reader user can find them from `weaver --help`.

### `EP-M4` width-aware tables, blocks, and `--width`

Discharges `REQ-TABLE`, `REQ-NARROW`, `REQ-ASCII`, `REQ-BLOCKS`, and
`GAP-WIDTH`. Add `terminal/` (the process probe, public because `main.rs` is a
separate crate), `presentation/` (`HostFacts`, `Policy`, `resolve`), and
`output/text/fit.rs`. `IoStreams::new` takes `HostFacts`; its 14 call sites
change together. Add the table, narrow, and fitted-context layouts, both glyph
sets, and `--width`.

Red: `INV-POLICY` for glyphs and width, `INV-SIZE`, fit kernel, snapshots at
20, 24, 60, and 100 columns in both glyph sets (plain only once), properties,
and `EP-M4` BDD. Green: the above. Accept:
`cargo nextest run -p weaver-cli presentation output::`.

### `EP-M5` colour and `--color`, with the PTY harness

Discharges `REQ-COLOUR` and `GAP-PTY`. Add roles and `paint(styled)`, the
setting, and the flag. Add `crates/weaver-e2e/tests/support/pty.rs` with
`rustix`: open a pair, set the size, give the child a new session and
controlling terminal, choose a pipe or PTY for each stream independently,
collect until `EIO`, allow for `ONLCR`, and kill the process group at a
deadline. The child runs with an empty environment plus explicit values.

| Role                | Sequence    | Textual equivalent     |
| ------------------- | ----------- | ---------------------- |
| Heading             | `ESC[1m`    | Position and rule line |
| File header         | `ESC[1;36m` | The path itself        |
| Caret and its label | `ESC[1;32m` | `^` and the word       |
| Note                | `ESC[33m`   | The word `note`        |
| Reset after each    | `ESC[0m`    | None                   |

_Table 4: The only sequences Weaver emits for style._

Red: colour truth table, erasure property, colour rows of Table 5, and `EP-M5`
BDD. Green: the above. Accept:
`cargo nextest run -p weaver-e2e --test terminal_contexts`.

### `EP-M6` terminal-sensitive progress

Discharges `REQ-STREAMS` and the progress half of `REQ-CONTEXT`. Add
`Connection::set_read_timeout` in `transport.rs`, hoist the buffered reader,
and wait for the first byte under D19. `presentation/progress.rs` is a pure
state machine, `frame(elapsed) -> Option<SafeText>`, whose writer erases its
line with `CR ESC[K`, truncates to standard error's width, never hides the
cursor, and clears on drop. Nothing appears in the first 300 milliseconds. Add
`respond_after` to the end-to-end `FakeDaemon`.

Red: state-machine tests; a scripted reader returning each retryable error; the
progress rows of Table 5. Green: the above. Accept: the same two suites. The
spinner text is localized and the static line is the same text.

### `EP-M7` pager and `--no-pager`

Discharges `REQ-FLAGS` and the pager half of `REQ-CONTEXT`. Add the `Pager`
port and `terminal/pager.rs`, whose `ProcessPager` inherits real descriptors.
The public `run` constructs it; `run_with_loader` and the test entry points
receive a double. `execute_daemon_command` returns the collected output and
status, and `CliRunner` delivers it, so no function gains a fifth parameter.
Flush standard output first. Page only when `may_page` holds, the document has
more lines than rows, and the response completed with status zero. Hold signal
handlers as D6 states.

| Claim                           | Presence                            | Absence, same long input        |
| ------------------------------- | ----------------------------------- | ------------------------------- |
| Paging needs three terminals    | All PTY: probe ran                  | Each stream piped in turn       |
| `--no-pager`, `--plain`         | Neither given: probe ran            | Either given                    |
| `PAGER` only from environment   | `PAGER=B`, file `pager=A`: B ran    | A did not                       |
| Unusable `TERM`, zero rows      | `TERM=xterm`, 24 rows: probe ran    | Unset, `dumb`, or zero rows     |
| Interrupt is survivable         | Probe records `\x03`; Weaver lives  | Terminal settings match at exit |
| Colour follows D4               | PTY, or piped with `--color always` | Piped, `NO_COLOR`, `never`      |
| Animation needs terminal stderr | PTY, delayed reply: erase seen      | Piped, JSON, `dumb`, fast reply |

_Table 5: Paired terminal-context tests. Piped cases use `--output human`._

The probe is a script named through `PAGER` that records a marker and copies
its input. For the interrupt row it also disables canonical mode and echo while
keeping signals, traps the interrupt to a second marker, and announces
readiness before `\x03` is sent; after a normal exit the terminal settings must
match those recorded beforehand. Further cases: a missing program, exit status
127, exit status 1, and a probe that closes early.

Red: pager rows of Table 5 and `INV-DELIVER`. Green: the above. Accept: as
before, plus manual checks in `Validation and acceptance`.

### `EP-M8` renderer metadata and closure

Discharges `REQ-META`. Add a `Copy` renderer discriminant to `CommandNode`,
whose tree is `const` and framework-independent, and convert it to
`RendererMetadata` in `command_ir`. For `definitions get` declare: human
supported, `tty` adapts, `closed_stdin` ignored; colour, progress, and pager
auto; emoji never; width terminal; accessibility text_equivalent; plain
available; stdout and stderr text; `subprocess_inheritance` allowed, because
the pager inherits standard output; `progress_inheritance` forbidden; fields
`path`, `line`, and `column` non-localized, and `label` localized. Leave the
machine record undeclared until 13.2.2, and note that agent context then
reports machine support as false.

Complete `docs/users-guide.md`, `docs/developers-guide.md` (§§15, 21.4, and
21.6, module map, the safe-text rule, adding a message, the PTY harness),
`docs/ui-gap-analysis.md`, design §2.1.11 (pointing to leynos/weaver#451),
`docs/contents.md`, and `docs/repository-layout.md`. Set ADR 0014 to Accepted.
Update and regenerate the boundary matrix, and re-review rows 12.1.2 and
12.1.4. Add locale bootstrap and localized parse errors to roadmap 13.3.3, and
mark 13.2.1 done. Shell completions stay with 13.3.3.

Red: the canonical projection expects the declaration. Green: add it. Accept:
all gates and the boundary gate.

## Milestones and plateaus

Compatibility decision for every milestone: none. Weaver is pre-0.1.0, no
external consumer or deployed format depends on these surfaces, and no data is
persisted. Recovery for every milestone: revert its commits; later milestones
are not affected because each adds its own setting and consumer.

| Milestone | Discharges                  | End state                                   | Remaining        |
| --------- | --------------------------- | ------------------------------------------- | ---------------- |
| `EP-M1`   | `GAP-DEP`                   | 0.11.0, unchanged behaviour, decisions kept | All behaviour    |
| `EP-M2`   | `GAP-ESCAPE`                | No unsafe text in human output              | Layout, flags    |
| `EP-M3`   | `REQ-L10N`, `REQ-LAYERED`   | Accessible, translatable `--plain`          | Width, colour    |
| `EP-M4`   | Table, narrow, ASCII, block | Adapts to the terminal; `--width`           | Colour, progress |
| `EP-M5`   | `REQ-COLOUR`, `GAP-PTY`     | Optional colour, proved in a terminal       | Progress, pager  |
| `EP-M6`   | `REQ-STREAMS`, progress     | Progress only in terminal contexts          | Pager            |
| `EP-M7`   | `REQ-FLAGS`, pager          | Paging only in terminal contexts            | Metadata         |
| `EP-M8`   | `REQ-META`                  | Contract, documents, and roadmap agree      | None             |

_Table 6: Plateaus._

At each boundary, record in `Progress`: requirements satisfied; design still
followed; no upstream assumption falsified; no unapproved interface,
dependency, trust boundary, or persisted format; trace links current; net lines
against Table 1.

## Concrete steps

Working directory: the repository root. Delegate full gates to `scrutineer` and
run them in this order, one at a time:

```sh
set -o pipefail
B=$(git branch --show-current); P=weaver
make fmt
make check-fmt    2>&1 | tee "/tmp/check-fmt-$P-$B.out"
make lint         2>&1 | tee "/tmp/lint-$P-$B.out"
make test         2>&1 | tee "/tmp/test-$P-$B.out"
make markdownlint 2>&1 | tee "/tmp/markdownlint-$P-$B.out"
make nixie        2>&1 | tee "/tmp/nixie-$P-$B.out"
```

For focused runs use the Makefile's flags, or Cargo rebuilds everything:

```sh
export RUSTFLAGS="-D warnings -Zthreads=8 -Clink-arg=-fuse-ld=mold"
cargo nextest run -p weaver-cli command_ir                       # EP-M1, EP-M8
cargo nextest run -p weaver-cli output::                         # EP-M2 onwards
cargo nextest run -p weaver-e2e --test hostile_text              # EP-M2
cargo nextest run -p weaver-config presentation                  # EP-M3 to EP-M7
cargo nextest run -p weaver-cli behaviour_human                  # EP-M3 to EP-M5
cargo nextest run -p weaver-cli presentation                     # EP-M4 onwards
cargo nextest run -p weaver-e2e --test terminal_contexts         # EP-M5 to EP-M7
cargo run -p weaver-docs-gate --example render_boundary_matrix -- \
  docs/orthoconfig-consumer-boundary.toml \
  docs/orthoconfig-consumer-boundary.md                          # EP-M8
cargo test -p weaver-docs-gate --test boundary_manifest          # EP-M8
```

Expect each to fail for the stated red reason before the change and to report
zero failures after it. Update `Progress` with both transcripts.

All milestones are delivered in one pull request (D24). Each milestone is a
separate run of commits ending at a gated plateau, and `EP-M7` in particular
must be revertible alone, so later commits must not mix pager code with other
changes.

## Validation and acceptance

Every milestone requires `make check-fmt`, `make lint`, and `make test`, and
`make markdownlint` and `make nixie` when Markdown changes. Manual acceptance
after `EP-M7`, in a terminal with a running daemon:

```sh
weaver definitions get --uri "$U" --position 10:5
weaver definitions get --uri "$U" --position 10:5 --width 24
weaver definitions get --uri "$U" --position 10:5 --plain
NO_COLOR=1 weaver definitions get --uri "$U" --position 10:5
weaver --output human definitions get --uri "$U" --position 10:5 | cat -v
weaver --output json definitions get --uri "$U" --position 10:5
```

Expect table and context; labelled blocks; labelled text; no colour; no `^[`
and no pager; and the daemon's JSON unchanged.

Done means every requirement traces to passing evidence, every obligation is
discharged with its negative control, the gates pass, documents and metadata
agree, and the roadmap entry is marked done. Performance: one read per distinct
source path, and no test in this work exceeds nextest's 60-second slow limit.

## Idempotence and recovery

Every step can be repeated. Accept `insta` snapshots only after reading the
diff. PTY tests kill their process group on deadline. Regenerate the boundary
matrix; never edit it. The throwaway probe crate lives outside the repository
and can be deleted.

## Artefacts and notes

Upstream copies, from tag `v0.11.0`:

```plaintext
0e022f67a4523bb4f54d6a7e596c5e3214d8392b05d3501e96323e22d0a2f91a  docs/users-guide.md
9883c6ad7d91e38444d828a6f187d9800a771dbe6e52caf76f27c94f176bfeb1  docs/v0-11-0-migration-guide.md
```

Settings probe: a throwaway crate for the `EP-M0` claims was written during
planning at `~/.cache/w1321-probe`, but its build was still waiting in the
shared compile queue when this plan was delivered. The claims rest on reading
the v0.11.0 source until `EP-M0` runs it.

External guidance consulted: <https://no-color.org/>, <https://clig.dev/>, and
OrthoConfig v0.11.0's ADR-0016, migration guide, and agent-native design.

Design review, 2026-11-24. Reviewers covered structure and alternatives,
contracts, terminal safety, verification, and delivery with accessibility. The
main changes made: vertical milestones instead of horizontal layers; explicit
injection points and public interfaces; the format-selection rule withdrawn
(D2); colour forcing limited to the command line (D4); pager outcomes, signals,
and three-terminal rule (D6); progress confined to the response wait (D7);
wider neutralization and error paths (D11, `EP-M2`); plain mode made equivalent
and unbounded (D9, D10); restated and weakened invariants; corrected budgets;
and an explicit account of the Verus rule. The strongest alternative offered
was to defer the pager. It is kept, isolated in `EP-M7`, because a `--no-pager`
flag without a pager would be inert; see question 2.

Questions for the approver, and the user's answers of 2026-11-24:

1. 13.2.1 makes output translatable while locale selection waits for
   OrthoConfig 11.3 through leynos/weaver#451: yes.
2. Build the pager now, in `EP-M7`: yes.
3. D4, with its amendments to ADR 0007 and design §2.1.3: yes.
4. Let `observe get-definition` output change (D1), and let `EP-M2` replace
   unsafe characters in other commands' human output: yes to both.
5. Omit a Verus milestone: yes, if it adds no value.
6. The dependencies in Table 7: yes.
7. One stacked pull request per milestone: no; a single pull request.

## Interfaces and dependencies

`presentation` and `output` are pure and import no adapter. Ports: `Pager`,
`SourceProvider`, `ConfigLoader`, and `ortho_config::Localizer`. Adapters: the
process probe and `ProcessPager` in `terminal/`, and the `cap_std` source
reader. The probe is called in `main.rs`; the other adapters are chosen in
`run`.

In `weaver-config`, public, with fields added by the milestone that uses them:

```rust
// Both implement FromStr, Serialize, and Deserialize; ColorChoice also
// implements clap::ValueEnum and Default.
pub enum ColorChoice { Auto, Always, Never }
pub struct OutputColumns(std::num::NonZeroU16); // 20..=65535, with get()
pub struct PresentationConfig {
    pub plain: bool,                  // EP-M3
    pub width: Option<OutputColumns>, // EP-M4
    pub color: ColorChoice,           // EP-M5
    pub no_pager: bool,               // EP-M7
}
pub struct PresentationOverrides { /* the same fields, each Option; Serialize */ }
pub enum PresentationError {
    InvalidSetting { /* setting, accepted values, layer or path */ },
    Composition(std::sync::Arc<ortho_config::OrthoError>),
}
impl PresentationConfig {
    pub fn load_layered<I, T, E>(
        arguments: I,               // program name and --config-path only
        environment: std::sync::Arc<E>,
        overrides: &PresentationOverrides,
    ) -> Result<Self, PresentationError>;
}
```

Settings names: `WEAVER_PLAIN`, `WEAVER_WIDTH`, `WEAVER_COLOR`,
`WEAVER_NO_PAGER`, and the file keys `plain`, `width`, `color`, `no_pager`.
Discovery is OrthoConfig's existing first-match order, which is shared with
`Config`.

In `weaver-cli`:

```rust
// public, because main.rs is a separate crate; opaque, and each fact is added
// by the milestone that reads it: output and size in EP-M4, NO_COLOR in EP-M5,
// standard error in EP-M6, standard input, rows, and pager in EP-M7
pub struct HostFacts { /* Facts */ }
pub trait StreamProbe { /* is_terminal and size, per stream */ }
pub struct ProcessStreams;
impl HostFacts {
    pub fn probe(environment: &impl mockable::Env, streams: &impl StreamProbe) -> Self;
    pub const fn non_interactive() -> Self;
}
// IoStreams::new(stdin, stdout, stderr, facts: HostFacts)
// OutputContext, ResolvedOutputFormat, OutputFormat::resolve, and
// render_human_output become pub(crate) together

pub(crate) trait Pager {
    fn page(&mut self, text: &str) -> PagerOutcome;
}
pub(crate) enum PagerOutcome { Delivered, NotStarted, Failed }
pub(crate) trait SourceProvider {
    fn read(&self, path: &std::path::Path) -> Result<String, SourceUnavailable>;
}
pub(crate) trait ConfigLoader {
    fn load(&self, args: &[OsString]) -> Result<Config, AppError>;
    fn load_presentation(
        &self, args: &[OsString], overrides: &PresentationOverrides,
    ) -> Result<PresentationConfig, AppError>;
}
// Built from PresentationConfig and PresentationOverrides. Colour is Forced only
// when overrides.color == Some(ColorChoice::Always).
pub(crate) struct RenderSettings { /* Settings */ }
pub(crate) fn resolve(
    settings: &RenderSettings, facts: &HostFacts, format: ResolvedOutputFormat,
) -> Policy;
pub(crate) fn neutralize(line: &str) -> SafeText;
pub(crate) fn neutralize_lines(text: &str) -> Vec<SafeText>;
pub(crate) fn wrap(
    widths: &[usize], first: usize, rest: usize,
) -> Vec<std::ops::Range<usize>>;
pub(crate) fn window(
    widths: &[usize], focus: usize, limit: usize,
) -> std::ops::Range<usize>;
pub(crate) fn paint(document: &Document, styled: bool) -> String;
```

Errors: an invalid layer is `AppError::LoadPresentation(PresentationError)`; an
unreadable source is a localized note; pager and payload fallbacks print one
neutralized, localized line on standard error and also emit `tracing` events
without source text or paths. No metrics are added.

Reuse policy. `output::text` and `output::document` are the only route by which
human output may carry foreign text; other modules must not escape or strip
characters, except the temporary isolation helper tracked by leynos/weaver#452.
`presentation::resolve` alone decides style, glyphs, width, paging eligibility,
and progress. Later resource commands flatten `HumanRenderArgs` and reuse
`PresentationConfig`, adding layouts under `output/<resource>/`. A trait is
used for `SourceProvider` so tests can count reads and inject failures.

| Crate               | Requirement | Where and when                | Reason                                    |
| ------------------- | ----------- | ----------------------------- | ----------------------------------------- |
| `ortho_config`      | `0.11.0`    | workspace, `EP-M1`            | C8                                        |
| `terminal_size`     | `0.4.4`     | `weaver-cli`, `EP-M4`         | Size of one descriptor; MSRV 1.71         |
| `signal-hook`       | `0.4`       | `weaver-cli` on Unix, `EP-M7` | D6; already locked for `weaverd`          |
| `rustix`            | `1.1`       | `weaver-e2e` tests, `EP-M5`   | `pty`, `termios`, `process`; 1.1.4 locked |
| `googletest`        | workspace   | `weaver-cli` tests, `EP-M2`   | Authorized by the task                    |
| `pretty_assertions` | workspace   | `weaver-cli` tests, `EP-M2`   | Authorized by the task                    |
| `mockall`           | workspace   | `weaver-cli` tests, `EP-M3`   | Port doubles                              |

_Table 7: Dependencies. Windows is not a release target for the pager._

The PTY harness needs one `unsafe` `pre_exec` call in test support, documented
with a `SAFETY` comment.

## Revision note

- 2026-11-24: initial draft (`2cfc1c75`).
- 2026-11-24: revised after a five-reviewer design review and the user's
  request to track locale selection and Fluent isolation as issues. Milestones
  were reordered into vertical slices, invariants restated, budgets corrected,
  and locale selection removed from scope. All implementation work remains.
- 2026-11-24: four reviewers rechecked the revision. Corrected the pager's route
  to delivery, socket timeouts, signal scope, response and file bounds,
  `L-WINDOW`, `L-CONCAT`, `INV-CONTEXT`, two BDD scenarios, and the public
  interface list.
- 2026-11-24: recorded the user's answers. Delivery becomes a single pull
  request, the contingent Verus milestone is removed, and the callers of
  `render_human_output` are corrected. The milestones are otherwise unchanged.
