# 13.2.1 Implement the localized human renderer for `definitions get`

This ExecPlan (execution plan) is a living document. The sections `Constraints`,
`Tolerances`, `Risks`, `Progress`, `Surprises & discoveries`, `Decision log`,
`Outcomes & retrospective`, `Conformance basis`, and `Verification plan` must
be kept up to date as work proceeds.

Status: DRAFT (awaiting design review and user approval)

Branch: `13-2-1-localized-human-renderer-for-definitions-get`

Roadmap item: `13.2.1` in `docs/roadmap.md`.

## Purpose / big picture

Weaver is a command-line tool (`weaver`) that asks a background daemon
(`weaverd`) questions about source code. The command this plan changes is the
one that finds where a symbol is defined:

```sh
weaver definitions get --uri file:///work/src/main.rs --position 10:5
```

Today a person at a terminal sees an untranslated, fixed-layout block:

```plaintext
/work/src/config.rs
  --> 42:8
   |
40 | /// Loads settings.
41 | #[must_use]
42 | pub fn load() -> Settings {
   |        ^ definition
43 |     Settings::default()
44 | }
```

The text never adapts to the terminal width, cannot be switched to a
screen-reader-friendly form, ignores the configured locale, and copies source
lines to the terminal unchanged. The last point is a defect as well as a
limitation: a source file containing terminal escape bytes can move the cursor,
recolour the screen, or set the window title.

After this change the same command, at a sufficiently wide UTF-8 terminal,
prints a localized summary table followed by source context:

```plaintext
2 definitions

#  File                 Line  Column
─  ───────────────────  ────  ──────
1  /work/src/config.rs    42       8
2  /work/src/legacy.rs     7       5

/work/src/config.rs
  ╭─▶ 42:8
   │
40 │ /// Loads settings.
41 │ #[must_use]
42 │ pub fn load() -> Settings {
   │        ^ definition
43 │     Settings::default()
44 │ }
```

When the table does not fit, each definition becomes a labelled block:

```plaintext
2 definitions

Definition 1 of 2
File: /work/src/
  config.rs
Line: 42
Column: 8
```

With `--plain` the output is labelled text only: no table, box drawing, caret
art, colour, pager, or animation. With `--color never` or `NO_COLOR`, no
colour. With `--no-pager`, no pager. With `--width 60`, sixty columns.

Success, quoting the roadmap, means: "human output does not rely on colour
alone and never emits pager or spinner control flow outside terminal contexts."

Machine output (`--output json`, and `--output auto` when standard output is
not a terminal) stays byte-for-byte unchanged. Universal `--json`, structured
errors, and bounded collections belong to roadmap items 13.2.2 and 13.2.3.

Terms used throughout:

- _Terminal context_: the relevant stream is attached to a terminal, also
  called a TTY. Standard output and standard error are judged separately.
- _Control flow_ (roadmap wording): starting a pager process, moving the
  cursor, erasing lines, or redrawing.
- _SGR sequence_: the `ESC [ … m` escape that selects colour or weight.
- _Display cell_: one terminal column. Some characters occupy zero or two.
- _Neutralize_: replace a character that a terminal would act upon with
  visible ASCII text.
- _Fluent_: the message-catalogue format (`.ftl` files) OrthoConfig uses for
  translations.
- _Layer_: one source of settings. Precedence, lowest first, is defaults,
  configuration file, environment, command line.

## Constraints

Hard invariants. Violating one requires escalation, not a workaround.

- C1. Machine output must not change. The bytes written in JSON mode must not
  depend on any setting, environment variable, or terminal fact this plan
  introduces. `crates/weaver-cli/tests/golden/request_definitions_get.jsonl`
  must not change, so the daemon never receives a presentation flag.
- C2. Do not change the daemon, its protocol, or `crates/weaverd`.
- C3. Public flags keep the spellings fixed by ADR 0007: `--plain`,
  `--color auto|always|never`, `--no-pager`, `--width <columns>`. `--locale`
  already exists and is not redefined.
- C4. Do not add `--json`, remove `--output`, or add enumerating errors,
  cursors, or truncation markers. Those are 13.2.2 and 13.2.3.
- C5. Do not localize protocol values: paths, line and column numbers, URI
  text, capability identifiers, flag names, and JSON.
- C6. No pager and no animation outside terminal contexts, with no override.
- C7. The pager command must never come from a configuration file. Weaver
  discovers `weaver.toml` in the working directory, so a file-supplied command
  would let an untrusted repository execute code (decision D6).
- C8. Use `ortho_config` 0.11.0, with a caret requirement.
- C9. No Rust source file may exceed 400 lines. The neighbouring files are
  near that limit already: `src/tests/unit/mod.rs` 396, `command_ir/tests.rs`
  391, `src/tests/behaviour.rs` 383, `runner_glue.rs` 383, `lib.rs` 378. New
  behaviour goes in new modules.
- C10. `crates/weaver-cli/build.rs` compiles `src/cli.rs`,
  `src/command_ir/mod.rs`, `src/command_surface/tree.rs`, `src/help.rs`, and
  `src/help_metadata.rs` through `#[path]`. Those files may use only crates in
  `[build-dependencies]` and may not name modules the build script lacks.
- C11. Tests must not change the process environment or working directory;
  `clippy.toml` disallows it. Inject `ortho_config::MapEnv` or
  `mockable::MockEnv`, or use a child process.
- C12. No `#[allow]`; a suppression needs `#[expect(…, reason = "…")]`. No
  `unwrap`, `expect`, indexing, string slicing, integer division, or lossy
  casts in production code. The workspace lint table enforces this.
- C13. No compatibility aliases, dual renderers, or shims. Weaver is
  pre-0.1.0. Update every caller in the same commit.
- C14. `docs/ortho-config-users-guide.md` and the version migration guides are
  byte-identical copies of upstream documents. Replace them; do not edit them.
- C15. Completed ExecPlans are historical. Do not rewrite them.
- C16. Implementation begins only after the user approves this plan.

## Tolerances (exception triggers)

Comparison base: the merge base with `origin/main`. Count with
`git diff --numstat "$(git merge-base origin/main HEAD)"` and the pathspecs
below. Each limit is net lines, additions minus deletions, and each category is
enforced independently. This ExecPlan is excluded from every count.

- Code: `crates/**/*.rs` and `crates/**/*.feature`.
- Configuration: `Cargo.toml`, `crates/*/Cargo.toml`, `.config/**`,
  `typos.local.toml`, `docs/orthoconfig-consumer-boundary.toml`, and
  `crates/**/*.ftl`.
- Authored documentation: `docs/**/*.md` except `docs/execplans/**`, the
  OrthoConfig guides below, and the generated matrix.
- Upstream copies: `docs/ortho-config-users-guide.md` and
  `docs/ortho-config-v0-11-0-migration-guide.md`.
- Generated, uncounted: `Cargo.lock`, `typos.toml`, `**/*.snap`, and
  `docs/orthoconfig-consumer-boundary.md`.

| Milestone | Code  | Configuration | Authored documentation |
| --------- | ----- | ------------- | ---------------------- |
| `EP-M1`   | 120   | 20            | 80                     |
| `EP-M2`   | 900   | 20            | 80                     |
| `EP-M3`   | 1,100 | 70            | 120                    |
| `EP-M4`   | 1,100 | 60            | 200                    |
| `EP-M5`   | 1,000 | 30            | 120                    |
| `EP-M6`   | 200   | 30            | 300                    |
| Total     | 4,400 | 230           | 900                    |

_Table 1: Net-line limits per milestone and in total._

Stop and escalate when any of these is reached:

- A milestone or total limit in Table 1.
- Upstream copies exceed 700 net lines.
- More than 70 non-ExecPlan files change.
- More than 200 lines of existing `.snap` content change. New snapshots are
  separate, and each must be reviewed by reading it.
- More than 40 Fluent messages are added per catalogue.
- A public interface beyond `Interfaces and dependencies` is needed.
- A dependency beyond `Interfaces and dependencies` is needed.
- A change to `weaverd`, the wire protocol, or `OutputFormat` values is needed.
- Moving to `ortho_config` 0.11.0 needs more than the `renderer` field and
  mechanical corrections.
- A gate still fails after three fix attempts.
- A milestone takes more than eight hours of active work, excluding time
  waiting for a compile slot.
- Two valid readings of a requirement would produce different user-visible
  output.

The sizes follow the evidence in `Context and orientation`: plan 12.1.2 set a
single limit of 1,200 lines and landed 7,797 additions. The present limits are
therefore per milestone, include tests, and stop the work at the milestone that
overruns.

## Risks

- R1. The scope spans settings, layout, localization, pager, and progress, and
  overruns as 12.1.2 did. Severity: high. Likelihood: medium. Mitigation: six
  plateaus with separate limits; each can be delivered as its own stacked pull
  request.
- R2. Claims about absence ("no pager outside a terminal") pass vacuously
  because no test ever reaches a terminal. Severity: high. Likelihood: high,
  because the repository has no pseudo-terminal harness. Mitigation: `EP-M5`
  adds one, and every absence test is paired with a presence test.
- R3. Untrusted source text reaches the terminal through a path that skips
  neutralization. Severity: high. Likelihood: medium. Mitigation: the
  `SafeText` type has one constructor for foreign text, the document writer
  accepts nothing else, and a whole-output property test checks the bytes.
- R4. Fluent inserts U+2068 and U+2069 around inserted values, breaking ASCII
  fallback and width measurement. Severity: medium. Likelihood: certain.
  Mitigation: one shared helper removes them; a property test covers it.
- R5. Presentation settings share files and the `WEAVER_` prefix with `Config`;
  a later key could collide. Severity: medium. Likelihood: low. Mitigation:
  both structs live in `weaver-config`, with a test for disjoint field names.
- R6. Compiler slots are shared across the host, so gates are slow. Severity:
  low. Likelihood: high. Mitigation: start each gate once and wait.
- R7. Pager tests depend on host tools. Severity: medium. Likelihood: medium.
  Mitigation: tests supply a purpose-built script through `PAGER`; none requires
  `less`.
- R8. A width measured per character differs from a terminal that shapes
  joined emoji. Severity: low. Likelihood: medium. Mitigation: documented
  limit; the caret and column bounds are stated over measured cells.
- R9. The upgrade to 0.11.0 disturbs other consumers. Severity: medium.
  Likelihood: low; upstream documents one source break, the `renderer` field.
  Mitigation: `EP-M1` is separate and validated alone.
- R10. Colours are unreadable on some themes. Severity: low. Likelihood:
  medium. Mitigation: use only the 16-colour palette's bold, cyan, and green,
  and never a background colour.

## Progress

- [x] (2026-11-24) Reconnaissance by a five-agent Wyvern team completed.
- [x] (2026-11-24) Initial draft completed.
- [ ] Community-of-experts design review applied.
- [ ] Plan approved by the user.
- [ ] `EP-M0` baseline gates recorded.
- [ ] `EP-M1` `ortho_config` 0.11.0 adopted.
- [ ] `EP-M2` safe text, document, and column-fitting kernel.
- [ ] `EP-M3` localized, width-aware `definitions get` layouts.
- [ ] `EP-M4` `--plain`, `--color`, `--no-pager`, and `--width` with layering.
- [ ] `EP-M5` pager and progress, proved in real terminal contexts.
- [ ] `EP-M6` renderer metadata, documentation, and roadmap closure.

## Surprises & discoveries

- Observation: `--locale` and `WEAVER_LOCALE` are documented and validated but
  have no effect. Evidence: `build_localizer()` in
  `crates/weaver-cli/src/localizer.rs` pins `en-US` and runs before
  configuration loads; `Config::locale()` has no caller outside `weaver-config`
  tests. Impact: localization must start by connecting it.
- Observation: OrthoConfig 7.2.2 shipped in 0.11.0 as descriptive metadata
  only. Evidence: upstream ADR-0016 lists "Implement terminal output, a
  rendering engine, or environment probing" as a non-goal. Impact: Weaver owns
  the runtime controls permanently, and the boundary-matrix wording for 13.2.1
  ("reusable human-renderer controls") can never be met as written (D15).
- Observation: the present renderer copies source lines verbatim. Evidence:
  `render_context` in `crates/weaver-cli/src/output/render.rs`. Impact: escape
  bytes in a source file reach the terminal, so colour controls alone could not
  satisfy the success criterion.
- Observation: tab characters have no measured width, so the caret is
  misplaced on tab-indented lines. Evidence: `caret_display_offset` sums
  `UnicodeWidthChar::width(ch).unwrap_or(0)`.
- Observation: payloads are rendered without regard to the stream they came
  from. Evidence: `process_message` in `crates/weaver-cli/src/daemon_output.rs`
  calls `render_stream_payload` before consulting `stream`.
- Observation: the build script watches a path that does not exist. Evidence:
  `crates/weaver-cli/build.rs` names `src/locales/en-US/messages.ftl`; the file
  is `locales/en-US/messages.ftl`.
- Observation: configuration flags given to the CLI are forwarded verbatim to
  `weaverd`. Evidence: `crates/weaver-cli/src/lifecycle/spawning.rs`. Impact:
  presentation settings must stay out of `CONFIG_CLI_FLAGS` (D3).

## Decision log

Decisions D1 to D19 were made by the planning agent on 2026-11-24 and take
effect on user approval.

- D1. One renderer serves both `definitions get` and the legacy
  `observe get-definition`; the per-invocation flags exist only on
  `definitions get`. Rationale: both reach the daemon as the same operation.
  Keeping a second definitions renderer for the legacy spelling would be a dual
  implementation (C13). The legacy spelling forwards unrecognized flags to the
  daemon, so it takes presentation settings only from files and the environment.
- D2. `--output auto` keeps its present meaning, except that naming any of
  the four flags on the command line selects human output. Rationale: ADR 0007
  wants human output by default with `--json` as the only machine switch, but
  changing the redirected default before `--json` exists would leave pipelines
  with no equivalent. A person who types `--plain > file.log` has asked for
  human output. An explicit `--output json` still wins and leaves the flags
  inert. Values from files and the environment never change the format.
- D3. Presentation settings live in a new
  `weaver_config::PresentationConfig`, separate from `Config`. Rationale:
  adding fields to `Config` would put a display preference in the daemon's
  schema, forward it to `weaverd`, and make an invalid colour prevent
  `weaver daemon start`. The struct uses the same prefix and discovery, so
  users write `color = "never"` in the same file. It is loaded only when human
  output has been selected, and before connecting to the daemon, so a mistake
  fails fast without starting anything.
- D4. `plain` removes all styling, whatever `color` says. Otherwise a resolved
  `color` of `always` or `never` decides; `auto` requires a terminal on
  standard output, an empty or absent `NO_COLOR`, and `TERM` other than `dumb`.
  Rationale: <https://no-color.org/> says a user's configuration file and flags
  may override the variable, and that an empty value counts as unset. Design
  §2.1.3 permits forced styling.
- D5. Nothing forces a pager or animation outside a terminal context.
  Rationale: the roadmap's success criterion is absolute, and narrower than
  design §2.1.3's "unless explicitly forced"; the design is amended to match.
- D6. The pager command comes only from the process environment: `PAGER`, or
  `less` when unset. It is split on ASCII whitespace and run without a shell.
  An empty value or `cat` disables paging. When `LESS` is unset the child
  receives `LESS=FRX`. A pager that cannot start is reported through `tracing`
  and the output is written directly. Rationale: C7, and <https://clig.dev/>
  recommends paging only on a terminal and those options.
- D7. Progress has three modes, chosen from standard error: animated on a
  capable terminal, static text otherwise, and none in JSON mode beyond the
  existing line. Rationale: `docs/ui-gap-analysis.md` says progress belongs on
  standard error only when it is a terminal, but a thirty-second silent wait in
  a log is worse than one line of plain text. The static line is the existing
  `Waiting for daemon start...` message, which has no control flow. The
  analysis is reworded to say "animated progress".
- D8. Unicode decoration is used only when standard output is a terminal, the
  locale variables declare UTF-8, `TERM` is not `dumb`, and `plain` is off.
  Everything else uses ASCII. Rationale: ADR 0007 asks for ASCII fallbacks for
  Unicode decoration; design §2.1.10 shows ASCII. This satisfies both. The rule
  governs decoration only; message text follows the catalogue's language.
- D9. Width comes from `--width`, then the settings layers, then the terminal,
  then `COLUMNS`, then 80. Outside a terminal, with no explicit width, output
  is unbounded. Explicit widths are 20 to 65,535; a detected terminal narrower
  than 20 is treated as 20. Rationale: the floor guarantees that a line-number
  gutter of up to 13 cells leaves room for any two-cell character, which makes
  the width bound absolute (`INV-WIDTH`).
- D10. A summary table with headings precedes the source-context blocks. If
  the natural table is wider than the limit, labelled blocks replace it.
  `--plain` always uses labelled blocks and shows the target line as a labelled
  field. Rationale: the roadmap asks for table headings and narrow labelled
  blocks, while design §2.1.10 requires context blocks; both are met.
- D11. Foreign text becomes `SafeText` by neutralization. Control characters
  (Unicode category Cc), the twelve Bidi_Control characters, and U+2028 and
  U+2029 become `<U+XXXX>`. Tabs expand to the next multiple of four cells.
  Rationale: structural safety rather than call-site discipline. The change
  also covers the reference and diagnostic renderers that share
  `render_locations`.
- D12. The renderer's locale is `Config::locale`, resolved after configuration
  loads. Localizing `--help` and clap errors before configuration loads stays
  with roadmap 13.3.3. Only `en-US` ships; tests use a pseudo-locale.
  Rationale: the pseudo-locale proves the mechanism without publishing an
  unreviewed translation. See `Questions for the approver`.
- D13. No Verus or Kani milestone. Rationale: see
  `Formal verification decision`.
- D14. The OrthoConfig upgrade is its own milestone and commit.
- D15. The boundary row becomes `consumes`, `shipped_in = "v0.11.0"`, for
  renderer metadata. ADR 0007 records that runtime renderer controls are
  Weaver-owned.
- D16. Two new dependencies: `terminal_size` 0.4 at run time and `rustix` 1
  for tests. Both are already in the dependency graph's style and licence
  range; see `Interfaces and dependencies`.
- D17. In human mode the standard-output payload for this command is buffered
  until the daemon's stream ends. Rationale: paging needs the whole document.
  The buffer is always written, including when the stream fails.
- D18. `IoStreams::new` takes a `TerminalContext` instead of a boolean.
  Fourteen call sites change together (C13).
- D19. Animation during the request uses a short read timeout before the first
  response byte and no thread. Rationale: waiting on a filled buffer consumes
  nothing, so no partial line can be lost, and standard error need not be
  shared across threads.

## Outcomes & retrospective

To be completed during implementation.

## Context and orientation

Inspected base revision: `fe56d226`. Line numbers below refer to it; relocate
symbols with `grep -rn "<symbol>" crates/`.

### How the command works today

`crates/weaver-cli/src/main.rs` tests whether standard output is a terminal and
builds `IoStreams`. `run` in `crates/weaver-cli/src/lib.rs` builds a localizer
and calls `run_with_handler`, which:

1. splits leading configuration flags from the rest with
   `split_config_arguments` (`config.rs`), stopping at the first other token;
2. parses the remainder with clap into `Cli` (`cli.rs`);
3. runs `handle_preflight`, then loads `weaver_config::Config`;
4. resolves `--output` with `OutputFormat::resolve`; and
5. calls `execute_daemon_command` (`runner_glue.rs`).

`execute_daemon_command` connects, starting the daemon if necessary
(`lifecycle/utils.rs`, `try_auto_start_daemon`), writes one JSON line, and calls
`read_daemon_messages` (`daemon_output.rs`). Each response line is either a
stream message (`stdout` or `stderr` plus text) or an exit status. In human
mode each payload passes through `render_human_output` (`output/mod.rs`), which
dispatches on the legacy operation and returns `None` for anything it does not
recognize, in which case raw text is written.

A definitions payload is a JSON array without a trailing newline, as built by
`crates/weaverd/src/dispatch/observe/get_definition.rs`:

```json
[{"uri":"file:///work/src/config.rs","line":42,"column":8}]
```

Lines and columns start at one; columns count UTF-16 code units. Daemon
failures for this operation are plain text on standard error.

`output/source.rs` turns a URI into a path. `output/render.rs` reads the file
through `cap_std` and draws the context block. The same `render_locations`
function serves references, diagnostics, and verification failures.

### Command metadata

`crates/weaver-cli/src/command_surface/tree.rs` is the hand-written command
tree. `command_ir/mod.rs` projects it into `ortho_config::docs::DocMetadata`,
from which help and the manual page are built. The independent expectation is
`command_ir/tests/canonical_projection.rs`. Both construct `DocMetadata` by
struct literal, so 0.11.0's new `renderer` field touches exactly those two
files. `help_output.rs` checks that tree arguments appear in help, but nothing
detects a flag present only in clap.

### Localization

`ortho_config::Localizer` is the lookup interface:
`message(id, args, fallback)`. `FluentLocalizer` wraps Fluent. It layers one
locale's catalogue over embedded defaults that exist only for English and
Japanese, so any other locale must be built with `disable_defaults()`. Fluent
surrounds inserted values with the invisible characters U+2068 and U+2069;
`strip_bidi_isolates` in `discoverability.rs` removes them and is private.

### Test facilities

`rstest` 0.28, `rstest-bdd` 0.5, `insta`, `proptest`, `mockable`, and `mockall`
are workspace dependencies. `googletest` 0.14.3 and `pretty_assertions` 1.4.1
are in the workspace but only `weaver-config` uses them. Behavioural steps for
the CLI are in `src/tests/behaviour.rs`, with features under
`crates/weaver-cli/tests/features/`; each scenario needs an explicit, named
binding. `crates/weaver-e2e` runs the real binary against a fake TCP daemon
(`daemon_harness::FakeDaemon`). There is no pseudo-terminal harness.

### Gates

| Command             | What it runs                                                                  |
| ------------------- | ----------------------------------------------------------------------------- |
| `make check-fmt`    | `cargo fmt --all -- --check`, then `mdtablefix --check`                       |
| `make lint`         | `cargo doc`, `cargo clippy` with warnings denied, `whitaker --all`, docstring |
| `make test`         | `cargo nextest run` over the workspace, then doctests                         |
| `make markdownlint` | `make spelling`, then `markdownlint-cli2`                                     |
| `make nixie`        | Mermaid validation                                                            |
| `make fmt`          | Formats Rust and Markdown                                                     |

_Table 2: Repository gates._

Clippy limits: four parameters, 70 lines, and four nesting levels per function.
Whitaker limits cognitive complexity to nine. The boundary-manifest gate is
`cargo test -p weaver-docs-gate --test boundary_manifest`.

Installed on the host but not configured by the repository: Kani 0.67 and
`cargo-mutants`. Verus is not installed. The CodeGraph server was unavailable
during planning.

### Budget evidence

| Plan                       | Limit set             | Outcome                       |
| -------------------------- | --------------------- | ----------------------------- |
| 12.1.2                     | 25 files, 1,200 lines | 116 files, +7,797 −2,336      |
| Adopt `ortho_config` 0.9.0 | 12 files, 500 lines   | Paused on unhealthy baseline  |
| Issue 368, 0.10.0          | None numeric          | Code +487 −125; guides +2,024 |

_Table 3: Recorded budgets and outcomes of comparable plans._

Plan 12.1.2 missed tests and documentation in its estimate and recorded the
breach afterwards. The 0.9.0 plan shows that a red baseline must be recorded
before starting. Estimates here count tests as code and separate the verbatim
upstream copies: the users' guide differs from 0.11.0 by 263 lines and the new
migration guide is 291.

### Documents and skills to consult

Documents: `docs/weaver-design.md` §§2.1.3, 2.1.7, 2.1.10, 2.1.11, and 2.3.1;
`docs/adrs/adr-0007-agent-native-command-surface.md`;
`docs/adrs/adr-0005-verification-trust-boundary.md`;
`docs/formal-verification-methods-in-weaver.md`; `docs/ui-gap-analysis.md`;
`docs/ortho-config-users-guide.md`; `docs/rust-testing-with-rstest-fixtures.md`;
`docs/rstest-bdd-users-guide.md`; `docs/rust-doctest-dry-guide.md`;
`docs/reliable-testing-in-rust-via-dependency-injection.md`;
`docs/complexity-antipatterns-and-refactoring-strategies.md`;
`docs/documentation-style-guide.md`; and `docs/developers-guide.md` §§15 and 21.

Skills: `execplans`, `sdlc-implementation`, `rust-router`, then
`domain-cli-and-daemons`, `rust-types-and-apis`, `rust-errors`,
`rust-unit-testing`, `proptest`, and `rust-verification`;
`hexagonal-architecture`; `arch-decision-records`; `localization-docs`;
`addressing-whitaker-findings`; `nextest`; `en-gb-oxendict-style`;
`commit-message`; `pr-creation`; and `pr-babysitting`.

## Conformance basis

There is no Terms of Reference. Upstream artefacts, all at `fe56d226` except
where stated: `docs/roadmap.md` 13.2.1; `docs/weaver-design.md` §§2.1.3,
2.1.10, and 2.1.11; ADR 0007 (status Proposed, "Human renderer contract"); ADR
0005; `docs/orthoconfig-consumer-boundary.toml` row 13.2.1; and OrthoConfig
v0.11.0 ADR-0016.

Requirements:

- `REQ-FLAGS`: `definitions get` accepts the four flags with ADR 0007 spellings.
- `REQ-LAYERED`: those settings layer through OrthoConfig.
- `REQ-L10N`: human prose resolves through Fluent using the configured locale,
  falling back deterministically.
- `REQ-TABLE`: tables have headings.
- `REQ-NARROW`: narrow output uses labelled blocks.
- `REQ-ASCII`: Unicode decoration has an ASCII fallback.
- `REQ-COLOUR`: meaning never depends on colour alone.
- `REQ-CONTEXT`: no pager or spinner control flow outside terminal contexts.
- `REQ-STREAMS`: results go to standard output; progress and diagnostics go to
  standard error.
- `REQ-BLOCKS`: code locations show source context, grouped by file.
- `REQ-META`: the renderer is declared through OrthoConfig 7.2.2 metadata.

Gaps between the base revision and the target:

- `GAP-LOCALE`: configured locale is ignored.
- `GAP-ESCAPE`: foreign control characters reach the terminal.
- `GAP-WIDTH`: no width awareness.
- `GAP-PTY`: no terminal test harness.
- `GAP-DEP`: `ortho_config` is 0.10.0.

```plaintext
GAP-DEP     -> EP-M1 -> weaver-config and command_ir suites on 0.11.0
GAP-ESCAPE  -> EP-M2 -> output::text property tests
REQ-COLOUR  -> EP-M2 -> output::document erasure property
REQ-TABLE, REQ-NARROW, REQ-ASCII, REQ-BLOCKS, GAP-WIDTH
            -> EP-M3 -> output::definitions snapshots and properties
REQ-L10N, GAP-LOCALE
            -> EP-M3 -> localizer tests and BDD locale scenarios
REQ-FLAGS, REQ-LAYERED
            -> EP-M4 -> presentation_precedence tests and BDD flag scenarios
REQ-CONTEXT, REQ-STREAMS, GAP-PTY
            -> EP-M5 -> weaver-e2e terminal_contexts
REQ-META    -> EP-M6 -> command_ir canonical projection
```

## Verification plan

The renderer is co-designed with its obligations. Writing the claims first
changed the design in five ways: output became a typed document with colour
applied last, so that "colour carries no meaning" is an erasure property;
foreign text became a distinct type; explicit widths gained a floor, so the
width bound has no exceptions; neutralized escapes became single-cell pieces,
so they can be wrapped; and the decision "may page" was separated from "is long
enough to page", so the safety claim is independent of content.

### Abstract model

Settings and facts are finite apart from two widths and the pager command:

```plaintext
Settings = { plain: Bool, colour: Auto|Always|Never, no_pager: Bool,
             width: None | 20..=65535 }
Facts    = { out_tty: Bool, err_tty: Bool, dumb: Bool, no_color: Bool,
             utf8: Bool, columns: None|1..=65535, rows: None|1..=65535,
             pager: None | Command }
Policy   = { styled: Bool, glyphs: Unicode|Ascii, plain: Bool,
             width: Unbounded | Bounded(20..=65535),
             pager: None | Command, progress: Animated|Static }
resolve  : Settings x Facts -> Policy        (total, pure)
```

Foreign text is a sequence of Unicode scalar values. A _piece_ is the smallest
unit the layout may move: a display width of 0, 1, or 2 cells and the range of
source UTF-16 units it represents. `neutralize` maps text to pieces. A document
is a sequence of lines, each a sequence of spans pairing a role with safe text.
`paint(document, styled)` produces characters.

Malformed inputs: text that is not valid UTF-8 cannot be read and produces an
explanatory note; a payload that does not parse is written as neutralized text;
a width outside the accepted range is rejected before connecting.

### Invariants and lemmas

- `INV-POLICY`: for all settings and facts,
  - styled if and only if not plain, and either colour is always, or colour is
    auto and out_tty, not no_color, and not dumb;
  - a pager only if out_tty, not plain, not no_pager, not dumb, and a command
    exists;
  - animated if and only if err_tty, not plain, and not dumb;
  - Unicode only if out_tty, utf8, not plain, and not dumb.
- `INV-INDEPENDENT`: styling, glyphs, width, and pager do not depend on
  err_tty; progress depends on no standard-output fact; pager and glyphs do not
  depend on colour or no_color.
- `INV-SAFE`: whatever the payload, source bytes, catalogue, settings, and
  facts, human output contains no character from the unsafe set except line
  feed and, when styled, the fixed SGR sequences Weaver defines.
- `INV-ERASE`: removing SGR sequences from styled output yields exactly the
  unstyled output. Hence no information is carried by colour.
- `INV-WIDTH`: with a bounded width, every output line's display width is at
  most that bound.
- `INV-CONSERVE`: for every definition, in payload order, the table, labelled,
  and plain layouts each contain its full path, line, and column; joining
  wrapped fragments recovers each value exactly.
- `INV-SELECT`: the table is used if and only if not plain and its natural
  width is within the bound; a table always has its heading row.
- `INV-CARET`: on a target line the caret's display column equals that of the
  first piece representing the target UTF-16 column, or the end of the line
  when the column is beyond it, and lies inside the visible window.
- `INV-LAYER`: for each setting independently, the effective value is that of
  the highest layer supplying it.
- `INV-LOCALE`: every message resolves in every catalogue, falling back to
  `en-US` and then to the built-in English; changing the locale never changes a
  protocol value.
- `INV-JSON`: JSON-mode bytes do not depend on settings, locale, or facts.
- `INV-PAGER-TRUST`: no file layer influences which program is executed.
- `INV-FLUSH`: buffered standard output is delivered exactly once on success,
  daemon failure, malformed response, missing exit, or pager failure.

Supporting lemmas, stated as hypotheses about the eventual argument:

- `L-POINT`: for every scalar value, its neutralization contains no unsafe
  scalar.
- `L-CONCAT`: neutralizing a concatenation equals concatenating
  neutralizations, apart from tab stops, which emit only spaces. With `L-POINT`
  and the document type, this yields `INV-SAFE`.
- `L-PARTITION`: `wrap(widths, limit)` returns consecutive, non-empty ranges
  covering the input once, each summing to at most `limit`, when every width is
  at most `limit`. Yields the wrapping halves of `INV-WIDTH` and `INV-CONSERVE`.
- `L-WINDOW`: `window(widths, focus, limit)` returns a range containing
  `focus`, summing to at most `limit`, under the same precondition. Yields
  `INV-CARET` and the source-line half of `INV-WIDTH`.
- `L-GUTTER`: a gutter has at most 13 cells, and the floor of 20 leaves at
  least 7, so every piece fits.

### Axioms (trusted, not verified here)

- `AXIOM-TTY`: `std::io::IsTerminal` and `terminal_size` report the stream's
  state.
- `AXIOM-WIDTH`: `unicode-width` gives terminal cell widths. Shaped emoji and
  ambiguous-width settings are outside the claim (R8).
- `AXIOM-FLUENT`: Fluent formats a message without altering its argument
  values beyond isolation characters.
- `AXIOM-MERGE`: `ortho_config` composes layers in documented order. The
  repository-owned command-line layer is tested against the real crate.
- `AXIOM-PAGER`: a pager displays its standard input and emits its own control
  sequences only to its terminal.

### Methods per obligation

- `INV-POLICY`, `INV-INDEPENDENT`.
  Method: exhaustive enumeration. Domain: 2 × 3 × 2 settings, 2^5 facts, and
  representative width and pager classes (absent, below floor, floor, typical,
  maximum), about 20,000 cases. Oracle: the formulae above written separately
  in the test as predicates, not calls to `resolve`. Artefact:
  `crates/weaver-cli/src/presentation/tests.rs`. Rationale: the domain is
  finite, so enumeration is a complete proof. Non-vacuity: count and assert
  that each outcome occurs; a negative control with `out_tty` dropped from the
  pager rule must fail.
- `L-POINT`, `L-CONCAT`.
  Method: exhaustive loop over all 1,112,064 scalar values, plus `proptest` for
  concatenation. Oracle: Unicode category ranges written as literals. Artefact:
  `crates/weaver-cli/src/output/text/tests.rs`. Non-vacuity: assert the exact
  count of neutralized scalars; removing U+202E must fail.
- `L-PARTITION`, `L-WINDOW`, `L-GUTTER`.
  Method: bounded exhaustive enumeration of every width sequence up to length
  eight over {0, 1, 2}, every focus, and every limit from 2 to 12, followed by
  `proptest` to length 2,000. Artefact:
  `crates/weaver-cli/src/output/text/fit_tests.rs`. Non-vacuity: require
  multi-range, single-range, and empty results; an off-by-one limit must fail.
- `INV-SAFE`, `INV-ERASE`, `INV-WIDTH`, `INV-CONSERVE`, `INV-SELECT`,
  `INV-CARET`. Method: `proptest` over payloads, source text, settings, and
  facts, with an independent SGR remover and width summation in the test.
  Generators must include escape sequences, C1 controls, bidi controls, tabs,
  wide characters, combining marks, empty files, out-of-range positions, and
  paths longer than the width; `prop_assume!` is not used, so rejection is
  zero. Artefact: `crates/weaver-cli/src/output/definitions/properties.rs`.
  Non-vacuity: deterministic witness tests show each class is generated; seeded
  faults (colour applied before layout, heading omitted, caret measured in
  bytes) must fail.
- `INV-LAYER`.
  Method: `rstest` table over each setting and each adjacent pair of layers
  using `MapEnv` and temporary files. Artefact:
  `crates/weaver-config/tests/presentation_precedence.rs`. Non-vacuity: every
  case uses distinct values per layer; reversing two layers must fail.
- `INV-LOCALE`.
  Method: parameterized test over message identifiers and catalogues, and
  property test over arguments. Artefact:
  `crates/weaver-cli/src/output/messages/tests.rs`. Non-vacuity: the
  pseudo-locale differs from English in every message, so a lookup that ignores
  the locale fails.
- `INV-JSON`.
  Method: `rstest` matrix of settings, locales, and terminal facts, comparing
  bytes with a fixed fixture. Artefact: `crates/weaver-cli/src/tests/unit/`.
  Non-vacuity: the same matrix in human mode must produce differing output.
- `REQ-CONTEXT`, `INV-PAGER-TRUST`.
  Method: end-to-end tests of the real binary with pipes and with a
  pseudo-terminal, pairing each absence with a presence. Artefact:
  `crates/weaver-e2e/tests/terminal_contexts.rs`. Non-vacuity: the probe pager
  records a marker file; the terminal case requires it, the pipe and `pager =`
  file cases forbid it.
- `INV-FLUSH`.
  Method: `rstest` table of stream endings with a fake connection and a
  `mockall` pager. Artefact: `crates/weaver-cli/src/daemon_output_tests.rs`.

Record `proptest` regression seeds in version control beside the test that
found them. Revert each seeded fault immediately and record the rejection.

### Formal verification decision

Neither Verus nor Kani is introduced. The claims that matter most are finite:
policy resolution and per-character neutralization are proved outright by
enumeration, with no tool in the trusted base. The remaining unbounded claims,
`L-PARTITION` and `L-WINDOW`, concern display layout: a fault is visible,
harmless to data, and well within reach of bounded enumeration plus generated
tests.

Verus would add a guarantee for sequences of every length. Its cost here is
disproportionate: it is not installed or pinned, roadmap 19.3.1 owns that work
and depends on 16.1.1, and `docs/formal-verification-methods-in-weaver.md`
reserves it for transaction and routing kernels that have stopped moving. Kani
over the same bounds would repeat the enumeration.

The lemma statements above are retained as the proof sketch. If `wrap` and
`window` are later reused by other renderers, they are suitable for roadmap
19.3.4 because they are pure functions over slices. No `assume`, `admit`, or
trusted proof body is introduced.

### Suggested BDD features

Proposed feature file:
`crates/weaver-cli/tests/features/weaver_cli_human_renderer.feature`, run by
`cargo nextest run -p weaver-cli behaviour`. Links: `REQ-FLAGS`, `REQ-TABLE`,
`REQ-NARROW`, `REQ-ASCII`, `REQ-COLOUR`, `REQ-L10N`; milestones `EP-M3` and
`EP-M4`.

```gherkin
Feature: Localized human output for definitions get

  Scenario: A wide terminal shows a headed table and source context
    Given a running fake daemon emitting two definitions
    And standard output is a terminal 100 columns wide
    When the operator runs "definitions get"
    Then stdout contains a table heading naming file, line, and column
    And stdout contains "^ definition"

  Scenario: A narrow terminal shows labelled blocks
    Given a running fake daemon emitting two definitions
    And standard output is a terminal 24 columns wide
    When the operator runs "definitions get"
    Then stdout contains "Definition 1 of 2"
    And no stdout line is wider than 24 columns

  Scenario: Plain output is labelled text without decoration
    Given a running fake daemon emitting two definitions
    And standard output is a terminal 100 columns wide
    When the operator runs "definitions get --plain --color always"
    Then stdout is ASCII and contains no escape character
    And stdout contains "Line: 2"

  Scenario: The configured locale selects the prose but not the values
    Given a running fake daemon emitting two definitions
    And the pseudo-locale is configured
    When the operator runs "definitions get --plain"
    Then stdout contains the pseudo-localized line label
    And stdout contains the unmodified definition path

  Scenario: No definitions were found
    Given a running fake daemon emitting no definitions
    When the operator runs "definitions get --plain"
    Then stdout contains "No definitions found."
    And the CLI exits with code 0

  Scenario: Hostile source text is made visible
    Given a source file containing terminal escape sequences
    And standard output is a terminal 100 columns wide
    When the operator runs "definitions get --color never"
    Then stdout contains "<U+001B>"
    And stdout contains no escape character

  Scenario: An invalid width is rejected before connecting
    When the operator runs "definitions get --width 5"
    Then stderr names the accepted range for "--width"
    And no daemon connection is attempted

  Scenario: JSON output ignores presentation settings
    Given a running fake daemon emitting two definitions
    When the operator runs "--output json definitions get --plain"
    Then stdout is the daemon payload unchanged
```

## Plan of work

### `EP-M0` baseline (no code)

Run the gates on the unmodified branch and record results. If a gate fails,
record the log and ask before proceeding, as the 0.9.0 plan required.

### `EP-M1` adopt `ortho_config` 0.11.0

Discharges `GAP-DEP`.

Raise the requirement in the root `Cargo.toml` and update the lock file for the
three OrthoConfig crates only. In `command_ir/mod.rs` and
`command_ir/tests/canonical_projection.rs`, name `renderer: None`. Add
`renderer` to the equality checks in `assert_canonical_options`, so `EP-M6`'s
declaration cannot be missed. Correct the `rerun-if-changed` path in `build.rs`.

Replace `docs/ortho-config-users-guide.md` with the 0.11.0 text from tag
`v0.11.0`, and add `docs/ortho-config-v0-11-0-migration-guide.md`. Record the
SHA-256 of each in `Artefacts and notes`. Update the version statements in
`docs/developers-guide.md` §15.1 and §15.2, `docs/weaver-design.md` §2.3.1,
`docs/rust-extricate-actuator-plugin-technical-design.md`, and
`docs/contents.md`.

Red: setting the version alone fails to compile with `missing field renderer`.
Green: add the field. No behaviour changes.

### `EP-M2` safe text, document, and column-fitting kernel

Discharges `GAP-ESCAPE`; establishes `REQ-COLOUR`.

Create under `crates/weaver-cli/src/output/`:

- `text/mod.rs`: `SafeText`, `Piece`, and `neutralize`.
- `text/fit.rs`: `wrap` and `window` over slices of widths.
- `document.rs`: `Role`, `Span`, `Line`, `Document`, `Glyphs`, and `paint`.

Rework `render.rs` to build a `Document` and paint it unstyled with ASCII
glyphs. References, diagnostics, and verification failures keep their layout;
the only changes they show are neutralized characters and correct caret
placement after tabs. Neutralize daemon text that is forwarded without
rendering in human mode.

Red: tests named for `L-POINT`, `L-PARTITION`, `L-WINDOW`, and `INV-ERASE`,
plus one regression test proving that an escape byte in a source file reaches
standard output today. Green: the modules above.

### `EP-M3` localized, width-aware `definitions get` layouts

Discharges `REQ-L10N`, `REQ-TABLE`, `REQ-NARROW`, `REQ-ASCII`, `REQ-BLOCKS`,
`GAP-LOCALE`, and `GAP-WIDTH`.

Move `strip_bidi_isolates` to `localizer.rs`. Add `localizer_for(&Locale)`,
which selects an embedded catalogue by exact tag, then by language, then
`en-US`, and returns a `FallbackLocalizer` that consults the selected catalogue
before `en-US`. Formatting issues are reported with `tracing::warn!`. After
configuration loads, `run_with_handler` builds this localizer and passes it to
`execute_daemon_command`.

Add `output/messages.rs`: one enumeration of renderer messages, each with a
Fluent identifier and an English fallback, and arguments passed only as
`SafeText` or numbers. Add the matching entries to
`locales/en-US/messages.ftl`, using plural selection for counts. The test-only
pseudo-locale lives in `crates/weaver-cli/src/tests/locales/`.

Add `output/definitions/`: `mod.rs` selects a layout, `table.rs`, `labelled.rs`,
`plain.rs`, and `context.rs` build documents, and `SourceProvider` is the port
through which source text is read, with the existing `cap_std` reader as its
adapter.

Introduce `presentation/mod.rs` with `TerminalContext`, `Settings`,
`PresentationPolicy`, and `resolve`, and `presentation/terminal.rs`, which
probes the process once in `main.rs` through `mockable::Env` and
`terminal_size`. `IoStreams::new` takes the `TerminalContext`. At this plateau
settings are the defaults, so the layout, width, and glyph choices follow the
terminal and output is unstyled.

Red: snapshot tests for table, labelled, plain, empty, single, grouped,
unreadable-source, non-file, and out-of-range cases at 20, 24, 60, and 100
columns in both glyph sets; property tests; the first, second, fourth, and
fifth BDD scenarios. Green: the modules above.

### `EP-M4` flags and layered settings

Discharges `REQ-FLAGS`, `REQ-LAYERED`, and `GAP` items left by `EP-M3`.

Add `crates/weaver-config/src/presentation.rs` with `PresentationConfig`,
`ColorChoice`, `OutputColumns`, and `PresentationOverrides`. Loading composes
the file and environment layers with
`PresentationConfig::compose_layers_from_iter_with_env`, appends
`MergeLayer::cli` holding the parsed overrides, and merges with
`merge_from_layers`. Only `--config-path` is passed as an argument, extracted
from the configuration split by a helper in `crates/weaver-cli/src/config.rs`.

Add `HumanRenderArgs` to `cli.rs`, flattened into `DefinitionGetArgs`. The two
booleans accept an attached value, so `--plain=false` can clear a configured
`true`, matching OrthoConfig's generated flags. Add the four arguments to
`tree.rs` and the canonical projection, with a new test that the clap arguments
of each structured command equal the tree's arguments in both directions.

Apply D2 when resolving the format, and D4 when resolving styling. Wire
`paint(styled)`.

Red: precedence table, policy enumeration, `INV-JSON` matrix, help snapshot for
`definitions get`, and the remaining BDD scenarios. Green: the above.

### `EP-M5` pager and progress in real terminal contexts

Discharges `REQ-CONTEXT`, `REQ-STREAMS`, and `GAP-PTY`.

Add `presentation/pager.rs`: the `Pager` port and a `ProcessPager` adapter.
After the stream ends, the document is paged only if the policy has a command
and the document has more lines than the terminal has rows. A pager closing its
input early is success. Standard output is flushed before the child starts.

Add `presentation/progress.rs`: a `Progress` port with `start`, `tick`, and
`finish`, whose animated adapter writes to standard error and erases its line
on finish, and whose static adapter writes one localized line. `wait_for_ready`
calls `tick` from its existing poll loop. Before the first response byte, the
connection uses a 120-millisecond read timeout and ticks on each expiry; the
timeout is cleared once data arrives. No animation appears unless the wait
exceeds 300 milliseconds. `finish` runs before any other write to standard
error, on every path.

Add `crates/weaver-e2e/tests/support/pty.rs`, using `rustix` to open a
pseudo-terminal, set its size, and collect output with a deadline. Add
`terminal_contexts.rs` with paired tests:

| Claim                               | Terminal case       | Outside a terminal        |
| ----------------------------------- | ------------------- | ------------------------- |
| Pager runs only on a terminal       | marker file present | marker absent, pipe/file  |
| `--no-pager` and `--plain` suppress | marker absent       | not applicable            |
| File-supplied `pager` is ignored    | marker absent       | marker absent             |
| Colour follows D4                   | SGR present         | SGR absent; `always` adds |
| Animation only on terminal stderr   | erase sequence seen | no escape byte            |
| `TERM=dumb` disables both           | no escape byte      | not applicable            |

_Table 4: Paired terminal-context tests._

### `EP-M6` metadata, documentation, and closure

Discharges `REQ-META`.

Declare a `RendererMetadata` for `definitions get` in the projection: human
supported, `tty` adapts, `closed_stdin` ignored, colour and progress and pager
auto, emoji never, width terminal, accessibility text_equivalent, plain
available, stdout text, stderr text, and fields `path`, `line`, and `column`
non-localized with `label` localized. The machine record stays undeclared until
13.2.2. The canonical projection test holds an independent expectation.

Write `docs/adrs/adr-0014-human-renderer-presentation-policy.md` for D3 to D11
and the proof-driven interface decisions. Amend ADR 0007 (D15),
`docs/weaver-design.md` §§2.1.3, 2.1.10, 2.1.11, and 2.3.1,
`docs/ui-gap-analysis.md`, `docs/users-guide.md` (flags, settings, environment,
samples, accessibility, pager), and `docs/developers-guide.md` (module map, the
`SafeText` rule, reuse policy, adding a message or locale, the pseudo-terminal
harness). Update `docs/contents.md` and `docs/repository-layout.md`. Update the
boundary TOML and regenerate the matrix. Mark 13.2.1 done in `docs/roadmap.md`.

## Milestones and plateaus

For every milestone, the conformance check is: assigned requirements met;
design still followed; no upstream assumption falsified; no unapproved
interface, dependency, trust boundary, or persisted format; trace links
current. Compatibility decision for every milestone: none, because Weaver is
pre-0.1.0 and no consumer is committed to the affected surfaces. Recovery for
every milestone: revert its commits; no data is migrated.

- `EP-M1`: 0.11.0 builds and behaves as before. Remaining: everything else.
- `EP-M2`: all human output is free of foreign control characters and carets
  are correct. Useful alone. Remaining: layouts, flags, pager.
- `EP-M3`: `definitions get` is localized and width-aware by default.
  Remaining: flags, colour, pager, progress.
- `EP-M4`: all four flags and layers work. `--no-pager` is accepted and
  trivially true. Remaining: pager and animation.
- `EP-M5`: pager and progress work and their absence is proved.
- `EP-M6`: metadata and documentation agree; roadmap closed.

## Concrete steps

Working directory: the repository root. Delegate full gate runs to the
`scrutineer` agent. Run them in this order, sequentially:

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

Focused commands:

```sh
cargo nextest run -p weaver-config presentation
cargo nextest run -p weaver-cli presentation
cargo nextest run -p weaver-cli output::
cargo nextest run -p weaver-cli behaviour
cargo nextest run -p weaver-e2e --test terminal_contexts
cargo test -p weaver-docs-gate --test boundary_manifest
cargo run -p weaver-docs-gate --example render_boundary_matrix -- \
  docs/orthoconfig-consumer-boundary.toml docs/orthoconfig-consumer-boundary.md
```

Set `RUSTFLAGS` as the Makefile does for focused runs, otherwise Cargo rebuilds
everything: `-D warnings -Zthreads=8 -Clink-arg=-fuse-ld=mold`.

## Validation and acceptance

`EP-M1` through `EP-M6` each require `make check-fmt`, `make lint`, and
`make test` to pass, plus `make markdownlint` and `make nixie` when Markdown
changes. Record for each milestone the red command and its failure, the green
command and its pass, and the refactor run.

Manual acceptance after `EP-M5`, with a running daemon, in a terminal:

```sh
weaver definitions get --uri "$U" --position 10:5            # table, context
weaver definitions get --uri "$U" --position 10:5 --width 24 # labelled blocks
weaver definitions get --uri "$U" --position 10:5 --plain    # plain text
NO_COLOR=1 weaver definitions get --uri "$U" --position 10:5 # no colour
weaver definitions get --uri "$U" --position 10:5 --plain | cat -v
weaver --output json definitions get --uri "$U" --position 10:5
```

Expect `cat -v` to show no `^[`, and the last command to print the JSON array
unchanged.

Done means: every requirement traces to passing evidence; every invariant and
lemma is discharged with its recorded negative control; gates pass; `INV-JSON`
holds; documentation and metadata agree; the roadmap entry is marked done.

## Idempotence and recovery

Every step can be repeated. `cargo insta` snapshots are accepted only after
reading the diff. Pseudo-terminal tests kill their child on deadline. The
boundary matrix is regenerated, never edited. Reverting a milestone's commits
restores the prior plateau.

## Artefacts and notes

Reconnaissance evidence, obtained during planning:

```plaintext
$ cargo info ortho_config
version: 0.10.0 (latest 0.11.0)
rust-version: 1.89.0
```

```plaintext
fluent-bundle-0.16.0/src/bundle.rs:641:            use_isolating: true,
```

External guidance consulted: <https://no-color.org/> (an unset or empty
variable does not disable colour; flags and user configuration may override),
<https://clig.dev/> (colour, animation, pager, plain output, and stream
advice), and OrthoConfig v0.11.0's ADR-0016, migration guide, and agent-native
CLI design.

Questions for the approver:

1. Should a second translation ship with this work (D12), or is the mechanism
   plus `en-US` sufficient until 13.3.3?
2. Is the D2 rule, that naming a human flag selects human output when
   redirected, acceptable before `--json` exists?
3. Are the two dependencies in D16 acceptable?
4. Should the milestones be delivered as stacked pull requests or as one?

## Interfaces and dependencies

Layering follows ports and adapters. `presentation` (policy) and `output`
(documents) are pure and import no adapter. `Pager`, `Progress`,
`SourceProvider`, and `ortho_config::Localizer` are ports. `terminal.rs`,
`ProcessPager`, the progress adapters, and the `cap_std` reader are adapters,
chosen only in `lib.rs` and `main.rs`.

In `weaver-config` (public):

```rust
pub enum ColorChoice { Auto, Always, Never }
pub struct OutputColumns(std::num::NonZeroU16); // 20..=65535
pub struct PresentationConfig {
    pub plain: bool,
    pub color: ColorChoice,
    pub no_pager: bool,
    pub width: Option<OutputColumns>,
}
pub struct PresentationOverrides { /* each field Option */ }
impl PresentationConfig {
    pub fn load_layered<I, T, E>(
        arguments: I,
        environment: std::sync::Arc<E>,
        overrides: &PresentationOverrides,
    ) -> ortho_config::OrthoResult<Self>;
}
```

In `weaver-cli`:

```rust
// presentation
pub struct TerminalContext { /* Facts */ }
pub(crate) struct PresentationPolicy { /* Policy */ }
pub(crate) fn resolve(
    settings: &PresentationConfig,
    terminal: &TerminalContext,
) -> PresentationPolicy;
pub(crate) trait Pager {
    fn page(&mut self, command: &PagerCommand, text: &str) -> std::io::Result<()>;
}
pub(crate) trait Progress {
    fn start(&mut self, message: &SafeText) -> std::io::Result<()>;
    fn tick(&mut self) -> std::io::Result<()>;
    fn finish(&mut self) -> std::io::Result<()>;
}

// output
pub(crate) struct SafeText(/* private */);
pub(crate) fn neutralize(text: &str) -> SafeText;
pub(crate) fn wrap(widths: &[usize], limit: usize) -> Vec<std::ops::Range<usize>>;
pub(crate) fn window(
    widths: &[usize], focus: usize, limit: usize,
) -> std::ops::Range<usize>;
pub(crate) fn paint(document: &Document, styled: bool) -> String;
pub(crate) trait SourceProvider {
    fn read(&self, path: &std::path::Path) -> std::io::Result<String>;
}

// changed
pub const fn IoStreams::new(stdin, stdout, stderr, terminal: TerminalContext);
```

Errors: an invalid layer is `AppError::LoadPresentation(Arc<OrthoError>)`; an
unreadable source is a localized note, never a failure; a pager failure is
logged and bypassed.

Observability: `tracing::debug!` for the resolved policy, with enumerated
fields only; `warn!` for pager failure, locale fallback, and an unrecognized
payload. No source text or path is logged. No metrics are added.

Reuse policy. `output::text` and `output::document` are the only route by which
human renderers in `weaver-cli` may write text; other modules must not strip or
escape characters themselves. `presentation::resolve` is the only place that
decides styling, glyphs, width, pager, or progress. Later resource commands
flatten `HumanRenderArgs` and reuse `PresentationConfig`; they add layouts under
`output/<resource>/`. None of these modules is exported from the crate.

Dependencies:

| Crate               | Requirement | Where                    | Why                             |
| ------------------- | ----------- | ------------------------ | ------------------------------- |
| `ortho_config`      | `0.11.0`    | workspace                | C8                              |
| `terminal_size`     | `0.4.4`     | `weaver-cli`, run time   | Window size on Unix and Windows |
| `rustix`            | `1.1`       | `weaver-e2e`, Unix tests | Pseudo-terminal, already locked |
| `googletest`        | workspace   | `weaver-cli`, tests      | Authorized by the task          |
| `pretty_assertions` | workspace   | `weaver-cli`, tests      | Authorized by the task          |
| `mockall`           | workspace   | `weaver-cli`, tests      | Port doubles                    |

_Table 5: Dependencies._

No table, pager, spinner, or colour crate is added: the safety claims depend on
Weaver owning every byte written.

## Revision note

- 2026-11-24: initial draft. Awaiting design review.
