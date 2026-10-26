# Migrate Weaver to OrthoConfig v0.10.0

## Why this work matters

Weaver currently resolves configuration through the published `ortho_config`
v0.9.0 API. Upgrading the consumer to v0.10.0 must preserve the configuration
values users observe while adopting the changed documentation IR, Fluent
identifiers, and explicit-command-line precedence. Success is observable when
the v0.10.0 dependency graph builds, configuration discovery still fails
visibly on invalid files, and tests prove defaults < files < environment < CLI,
including a CLI value that repeats its built-in default.

The user explicitly requested implementation, validation, frequent commits,
CodeRabbit review after each major milestone, an SSH push, and a draft PR for
issue #368. That request is standing approval to execute this plan.

## Conformance basis

The primary requirements are issue #368 and the immutable OrthoConfig v0.10.0
documentation recorded below. Weaver's current configuration contract is
specified by `docs/orthoconfig-consumer-boundary.toml`, ADR 0007, the active
configuration sections of `docs/developers-guide.md` and
`docs/weaver-design.md`, and the existing `weaver-config` tests. No separate
technical design or terms-of-reference document governs this dependency
migration.

## Immutable upstream documentation

The authoritative source is `leynos/ortho-config`, tag `v0.10.0`, fetched and
resolved before any dependency, source, or test edits. The fetched tag object
was `996e5a1c9f22ace4f55bf5dcc196d2c0524262bf`; it peels to commit
`38812c563e3953a490e3efc202609f37e21a0517`, which matches the surveyed commit.
Both files below were imported from that immutable commit and read before
implementation.

| Upstream path                     | Local path                                     | SHA-256                                                            |
| --------------------------------- | ---------------------------------------------- | ------------------------------------------------------------------ |
| `docs/users-guide.md`             | `docs/ortho-config-users-guide.md`             | `84d1c97b77024224dc9220a12d4b749205d9bb2db54849185da21c969dda5575` |
| `docs/v0-10-0-migration-guide.md` | `docs/ortho-config-v0-10-0-migration-guide.md` | `b5331aa6cfcc07b3dcd188c620db98d0a1a1f13b0c8ef96d88913d701bca25bf` |

The local v0.10.0 guide replaces the previously imported OrthoConfig user's
guide. The v0.6.0, v0.8.0, and v0.9.0 migration guides remain historical files.
Local adoption notes belong in this plan and other Weaver documentation, never
inside either upstream copy. Their relative links remain byte-for-byte as
published. Markdown resolves those links against the local paths, so some
targets collide with Weaver files or are absent. The following map is the
authoritative way to locate the upstream target or its local imported copy; it
does not rewrite the source links.

| Imported source                                | Literal relative target                                     | Upstream target at the pinned commit                          | Local mapped target                                                      |
| ---------------------------------------------- | ----------------------------------------------------------- | ------------------------------------------------------------- | ------------------------------------------------------------------------ |
| `docs/ortho-config-v0-10-0-migration-guide.md` | `users-guide.md#<anchor>`                                   | `docs/users-guide.md#<anchor>`                                | `docs/ortho-config-users-guide.md#<anchor>`                              |
| `docs/ortho-config-users-guide.md`             | `v0-10-0-migration-guide.md#<anchor>`                       | `docs/v0-10-0-migration-guide.md#<anchor>`                    | `docs/ortho-config-v0-10-0-migration-guide.md#<anchor>`                  |
| `docs/ortho-config-users-guide.md`             | `v0-9-0-migration-guide.md#<anchor>`                        | `docs/v0-9-0-migration-guide.md#<anchor>`                     | `docs/ortho-config-v0-9-0-migration-guide.md#<anchor>`                   |
| `docs/ortho-config-v0-10-0-migration-guide.md` | `../README.md`, `../CHANGELOG.md`                           | Root `README.md` and `CHANGELOG.md`                           | Same upstream paths at commit `38812c563e3953a490e3efc202609f37e21a0517` |
| `docs/ortho-config-v0-10-0-migration-guide.md` | `../examples/hello_world/`, `../ortho_l10n/README.md`       | Same paths in the upstream repository                         | Same upstream paths at the pinned commit                                 |
| `docs/ortho-config-v0-10-0-migration-guide.md` | `adrs/adr-0004-cargo-external-subcommand-entry-point.md`    | `docs/adrs/adr-0004-cargo-external-subcommand-entry-point.md` | Same upstream path at the pinned commit                                  |
| `docs/ortho-config-v0-10-0-migration-guide.md` | `cargo-orthohelp-design.md`, `cargo-orthohelp-api-guide.md` | Same paths under upstream `docs/`                             | Same upstream paths at the pinned commit                                 |
| Either imported guide                          | `#<anchor>`                                                 | Anchor in the current upstream page                           | Same anchor in the byte-identical local copy                             |

The first row is the important path collision: the literal local link resolves
to Weaver's `docs/users-guide.md`, not the OrthoConfig guide. Do not rewrite
the imported link to hide that fact; use the explicit mapping above when
following the upstream document.

The guides describe v0.10.0 only. Planned or “not yet available” items are not
available APIs and are excluded from this migration.

## Baseline and repository inventory

The requested survey baseline is `41cab0d660e0a8d75216c63207f5fbf6fa0a185d`.
The actual clean source checkout before this task was
`5efe82fc4bc844102b0c93a78e34abd7199e0730`, four commits later on
`origin/main`. The only pre-implementation worktree changes are the two
imported documents. The compiler reports
`rustc 1.101.0-nightly (76456a688bc0ed897fa23a677d4ddb5b3fdb575a 2026-10-25)`,
LLVM 23.1.4, from the repository pin `nightly-2026-10-26`. The pre-migration
workspace declared Rust 1.89.0, matching the v0.10.0 floor; later graph checks
showed that the application runtime graph needs a higher floor.

There is one Cargo workspace, one maintained `Cargo.lock`, and no independent
standalone manifest or submodule. The workspace dependency
`ortho_config = "0.9.0"` is inherited by `weaver-config`, `weaverd`, and
`weaver-cli`, including the CLI build dependency. No direct macro or
`cargo-orthohelp` dependency is present. There are no direct Figment, `uncased`,
`xdg`, `serde_yaml`, `figment-json5`, or `fluent-bundle` dependencies in
Weaver manifests. The `serde-saphyr` dependency belongs to the separate
`sempai-yaml` crate and is not an OrthoConfig duplicate.

`Config` is the production derive in `weaver-config`; source loading occurs in
the CLI and daemon adapters. The configuration precedence integration test
currently mutates process environment under a lock; there is no `figment::Jail`.
`configuration_failfast.rs` tests discovery failure visibility and must remain
unmodified. Weaver has no `AgentContext` literal, no `cargo-orthohelp` call,
and no production OrthoConfig boolean field. The command IR and help renderer
do build metadata literals and must follow the v0.10.0 metadata shape.

## Migration applicability matrix

| v0.10.0 guide change                                                  | Weaver decision                                                                                                                        | Evidence or acceptance check                                                               |
| --------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------ |
| Update runtime, direct macro, and installed tooling together          | Required: update inherited `ortho_config` to `0.10.0`; no direct macro/tooling package exists                                          | Manifest audit, targeted lock update, `cargo tree -i ortho_config`                         |
| Explicit CLI value equal to a default now wins                        | Applicable: preserve the layer order and add a regression where CLI repeats `en-US` while lower layers differ                          | BDD test in `configuration_precedence.feature`                                             |
| Fluent identifiers and documentation IR 2.0                           | Applicable: re-key field-help identifiers and compare projected IR with `ORTHO_DOCS_IR_VERSION`                                        | Help/localization tests and command IR tests                                               |
| `AgentContext.profiles` uses `ProfilesDeclaration`                    | Inapplicable: no Weaver `AgentContext` literal                                                                                         | Repository symbol and literal inventory                                                    |
| `DocMetadata` adds behaviour and profiles fields                      | Applicable: name `behaviour: None` and `profiles: None` to preserve absent declarations                                                | Command IR construction and canonical projection tests                                     |
| Boolean flags accept `--flag=false`; spaced false remains invalid     | Applicable to the derived boolean test fixture and metadata projection; no production OrthoConfig bool flag needs a user-facing change | Help parser tests for attached false and rejected spaced false                             |
| Named profiles                                                        | Not adopted: no product requirement; would add a new configuration tier and reserve names                                              | Keep the existing precedence contract                                                      |
| Stacked discovery scopes                                              | Not adopted: first-wins discovery is part of current behaviour                                                                         | Leave `configuration_failfast.rs` unchanged                                                |
| Injected environment sources with `MapEnv`                            | Adopt for the precedence test harness because v0.10.0 exposes source-aware loaders                                                     | No process environment mutation in the touched test; source-layer and discovery tests pass |
| Infer string defaults with `cli_default_as_absent`                    | Not applicable: Weaver has no derived field that duplicates a clap string default                                                      | Field-attribute inventory                                                                  |
| Agent-native behaviour metadata and policy checks                     | Not adopted as a capability; keep the required `behaviour: None` IR field to preserve the absent declaration                           | Manifest, build-script, and CI inventory                                                   |
| Cargo external-subcommand helper                                      | Not adopted: Weaver does not provide a Cargo external subcommand                                                                       | CLI entry-point inventory                                                                  |
| Standalone `ortho_l10n` runtime                                       | Not adopted: localization is used through the `ortho_config` facade                                                                    | Existing `FluentLocalizer` imports and dependency inventory                                |
| `cargo-orthohelp` stdout, policy report, and bridge build environment | Inapplicable: Weaver does not install or invoke that tool                                                                              | CI, Makefile, build-script, and metadata inventory                                         |
| Missing, malformed, and unreadable files remain distinct              | Preserve existing fail-fast behaviour; do not change the dedicated fail-fast test                                                      | `configuration_failfast.rs` remains byte-for-byte unchanged and passes                     |
| Keep default features, including `serde_json`                         | Required: retain default features; do not add `default-features = false`                                                               | Manifest and resolved feature graph inspection                                             |

## Constraints and tolerances

Keep public flags, `WEAVER_*` variables, discovery files, configuration
precedence, localized output, generated documentation, and exit-status/stream
contracts stable. Keep all production policy in `weaver-config` and preserve
the current adapter boundaries. Do not edit `configuration_failfast.rs`,
vendored code, historical imported migration guides, or the upstream copies. Do
not use roadmap-only APIs, suppress warnings, add ignores, or pass
`--ignore-rust-version`.

The normal implementation scope is the workspace manifest and lockfile,
`weaver-config`, `weaver-cli`, directly related tests, and current
documentation. If a change to public configuration names, semantics, errors, or
adapter ownership becomes necessary, record the evidence and compatibility
impact in the decision log before proceeding. Keep the imported document hashes
exact after any repository-wide formatter runs.

## Verification plan

The preserved invariant is that each field is resolved in the order built-in
defaults, discovered files, environment, then explicit CLI values. The finite
precedence partitions are covered by the existing feature scenarios and a new
case in which the CLI explicitly repeats the built-in locale. A false result is
observable if any lower layer wins. File discovery and error-class distinctions
are covered by the untouched `configuration_failfast.rs` test. The CLI and
daemon integration tests protect their output streams and exit statuses. No new
mathematical lemma, unbounded state invariant, or concurrency protocol is
introduced, so property testing, model checking, and formal proof are not
proportionate to this dependency/API migration.

| Obligation                                                                    | Evidence                                                                         | Discharge condition                                                                                           |
| ----------------------------------------------------------------------------- | -------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------- |
| Resolve the published v0.10.0 packages without duplicate OrthoConfig versions | Targeted `cargo update`, `cargo tree -i ortho_config`, lockfile diff             | All direct consumers resolve to v0.10.0; any older transitive version has an explained chain                  |
| Preserve configuration precedence and discovery                               | Focused unit/BDD tests, untouched fail-fast test, source-aware test harness      | Defaults < files < environment < CLI and each unhappy path retain expected result                             |
| Preserve localized CLI/help and generated docs                                | Help tests, snapshots, command IR tests, boundary-manifest test                  | Localized identifiers resolve; serialized IR version equals the dependency constant; reviewed snapshots match |
| Preserve CLI boolean parsing contract                                         | Derived fixture tests                                                            | `--force=false` succeeds and `--force false` remains rejected                                                 |
| Preserve compiler and quality policy                                          | Repository Make targets, current compiler identity, lockfile and manifest review | Required gates pass under the selected supported toolchain; failures are reported with logs                   |

The BDD feature contract is
`crates/weaver-config/tests/features/configuration_precedence.feature`:

```gherkin
Feature: OrthoConfig v0.10.0 configuration precedence
  Scenario: An explicit default value overrides lower layers
    Given a configuration file setting the locale to "ja-JP"
    And the environment overrides the locale to "fr-FR"
    When the CLI explicitly sets the locale to "en-US"
    Then the resolved locale is "en-US"
```

This scenario has witnesses for the file, environment, and CLI tiers and fails
if the old whole-object default comparison drops the explicit CLI layer.

## Milestones

1. **EP-M1 — Record provenance and scope.** Add this living plan, index the
   imported v0.10.0 guide, and verify both imported hashes. Run documentation
   gates, commit the documentation milestone, then request CodeRabbit review
   and clear its concerns before source changes.
2. **EP-M2 — Align dependencies and compiler.** Update the inherited runtime
   constraint and lockfile precisely, inspect the resolved feature graph and
   Rust floor, and decide from evidence whether the existing dated nightly pin
   needs a change. Do not widen features or change MSRV if the current floor
   already satisfies v0.10.0.
3. **EP-M3 — Adapt source and regression coverage.** Update IR literals and
   version assertions, Fluent field-help identifiers, boolean metadata/help
   rendering, and the source-aware configuration test harness. Add the
   explicit-default BDD case and keep fail-fast discovery untouched. Run
   focused red/green checks before the full sequential gates.
4. **EP-M4 — Reconcile active documentation and boundary evidence.** Update
   current-version references and user-facing precedence guidance, regenerate
   the consumer boundary matrix from its TOML source, and review the complete
   diff and lockfile graph. Run all applicable gates and CodeRabbit before
   delivery.
5. **EP-M5 — Push and draft PR.** Push over SSH, refresh and verify the remote
   tracking ref, create a draft PR titled with `(#368)`, state `Closes #368`,
   and end the description with a `## References` section containing the Lody
   session URL. Read the PR back; do not merge it.

## Baseline validation

The sequential baseline ran before implementation edits. `make check-fmt`,
`make typecheck`, `make lint`, `make markdownlint`, `make nixie`, and
`make test-workflow-contracts` passed. Type checking and lint reported existing
Cargo warnings about missing `[lints]` inheritance in several manifests and
unused dependencies in `weaver-docs-gate` and `weaver-e2e`.

`make test` failed in the existing development-profile Cranelift backend: 531
tests passed, 8 `weaver-cards` tests aborted on
`llvm.x86.sse2.pmulhu.w is not yet supported`, 4 tests were skipped, and 1,019
were not run after nextest fail-fast. The doctest recipe did not run. This is a
pre-existing backend limitation, not evidence of an OrthoConfig regression. The
exact baseline logs are under `/tmp/`:

The durable baseline record is:

| Gate                           | Result                                                                             |
| ------------------------------ | ---------------------------------------------------------------------------------- |
| `make check-fmt`               | Passed; 102 files unchanged.                                                       |
| `make typecheck`               | Passed in 6m 27s; existing manifest and unused-dependency warnings noted above.    |
| `make lint`                    | Passed in 13m 35s; existing manifest and unused-dependency warnings noted above.   |
| `make test`                    | Failed (exit 100): 531 passed, 8 failed, 4 skipped; 1,019 not run after fail-fast. |
| `make markdownlint`            | Passed; 102 Markdown files, zero errors.                                           |
| `make nixie`                   | Passed; all diagrams validated.                                                    |
| `make test-workflow-contracts` | Passed; 271 tests.                                                                 |

The eight test aborts were the seven `weaver-cards` graph-slice snapshot tests
and one graph-slice behaviour test. Each hit the unsupported
`llvm.x86.sse2.pmulhu.w` intrinsic in the configured Cranelift backend. The
doctest recipe did not run. This record is retained here; the following log
paths are supplementary and may not persist beyond the work environment:

```plaintext
/tmp/check-fmt-82e5abc1-2df2-47e5-89b8-49d2b584e7d8-issue-368-migrate-from-ortho-config-v0-9-0-to-v0-10-0.out
/tmp/typecheck-82e5abc1-2df2-47e5-89b8-49d2b584e7d8-issue-368-migrate-from-ortho-config-v0-9-0-to-v0-10-0.out
/tmp/lint-82e5abc1-2df2-47e5-89b8-49d2b584e7d8-issue-368-migrate-from-ortho-config-v0-9-0-to-v0-10-0.out
/tmp/test-82e5abc1-2df2-47e5-89b8-49d2b584e7d8-issue-368-migrate-from-ortho-config-v0-9-0-to-v0-10-0.out
/tmp/markdownlint-82e5abc1-2df2-47e5-89b8-49d2b584e7d8-issue-368-migrate-from-ortho-config-v0-9-0-to-v0-10-0.out
/tmp/nixie-82e5abc1-2df2-47e5-89b8-49d2b584e7d8-issue-368-migrate-from-ortho-config-v0-9-0-to-v0-10-0.out
/tmp/test-workflow-contracts-82e5abc1-2df2-47e5-89b8-49d2b584e7d8-issue-368-migrate-from-ortho-config-v0-9-0-to-v0-10-0.out
```

The documentation/provenance milestone then passed `make check-fmt`,
`make markdownlint`, and `make nixie` sequentially. Both imported guide hashes
still match the immutable source commit and `git diff --check` is clean. These
durable results for the final docs-only candidate were: format check passed
with 103 files unchanged; Markdown lint passed with 103 files and zero errors;
all Mermaid diagrams validated; both source hashes matched; and
`git diff --check` passed. The log paths below are supplementary evidence:

```plaintext
/tmp/check-fmt-82e5abc1-2df2-47e5-89b8-49d2b584e7d8-issue-368-migrate-from-ortho-config-v0-9-0-to-v0-10-0-2.out
/tmp/markdownlint-82e5abc1-2df2-47e5-89b8-49d2b584e7d8-issue-368-migrate-from-ortho-config-v0-9-0-to-v0-10-0-2.out
/tmp/nixie-82e5abc1-2df2-47e5-89b8-49d2b584e7d8-issue-368-migrate-from-ortho-config-v0-9-0-to-v0-10-0-2.out
```

CodeRabbit reviewed the documentation commit at
`32fb1c8080573c3046d2923fed2ca340a4238118` and reported two minor findings
about those relative links. The source files remain exact upstream copies; the
plan now maps each affected literal link and its local reading target. The
first docs-only recheck caught two American spellings in the new prose; after
correcting them, all documentation gates passed again and both imported hashes
still match:

```plaintext
/tmp/coderabbit-82e5abc1-2df2-47e5-89b8-49d2b584e7d8-issue-368-migrate-from-ortho-config-v0-9-0-to-v0-10-0-3.out
/tmp/check-fmt-82e5abc1-2df2-47e5-89b8-49d2b584e7d8-issue-368-migrate-from-ortho-config-v0-9-0-to-v0-10-0-5.out
/tmp/markdownlint-82e5abc1-2df2-47e5-89b8-49d2b584e7d8-issue-368-migrate-from-ortho-config-v0-9-0-to-v0-10-0-5.out
/tmp/nixie-82e5abc1-2df2-47e5-89b8-49d2b584e7d8-issue-368-migrate-from-ortho-config-v0-9-0-to-v0-10-0-5.out
```

After recording those results in this plan, the final candidate also passed
`make check-fmt`, `make markdownlint`, and `make nixie` sequentially:

```plaintext
/tmp/check-fmt-82e5abc1-2df2-47e5-89b8-49d2b584e7d8-issue-368-migrate-from-ortho-config-v0-9-0-to-v0-10-0-6.out
/tmp/markdownlint-82e5abc1-2df2-47e5-89b8-49d2b584e7d8-issue-368-migrate-from-ortho-config-v0-9-0-to-v0-10-0-6.out
/tmp/nixie-82e5abc1-2df2-47e5-89b8-49d2b584e7d8-issue-368-migrate-from-ortho-config-v0-9-0-to-v0-10-0-6.out
```

CodeRabbit completed review of `e66076bcab8b48f81eefac0b91e700eacd77fbb2` with
exit 0 in 458 seconds and reported five minor findings. The contents entry
incorrectly described the imported guide as Weaver adoption notes; this plan
update corrects it to “Published OrthoConfig v0.10.0 migration guide.” The plan
now records the baseline and documentation gate outcomes above, so they remain
available if temporary logs disappear. Three wording findings targeted the
byte-identical upstream guide copies (“the derive” → “the derive macro” and “If
application source imports” → “If the application source imports”). Those edits
are intentionally not applied because the user requires the imported text to
remain byte-for-byte intact. The verified SHA-256 values remain the integrity
check. Review output is supplementary at:

```plaintext
/tmp/coderabbit-82e5abc1-2df2-47e5-89b8-49d2b584e7d8-issue-368-migrate-from-ortho-config-v0-9-0-to-v0-10-0-5.out
```

After correcting the local index entry and recording these dispositions,
`make check-fmt` passed with 103 files unchanged, `make markdownlint` passed
with 103 files and zero errors, and `make nixie` validated all diagrams. Both
imported guide hashes and `git diff --check` passed. The per-gate output is
available at:

```plaintext
/tmp/check-fmt-82e5abc1-2df2-47e5-89b8-49d2b584e7d8-issue-368-migrate-from-ortho-config-v0-9-0-to-v0-10-0-8.out
/tmp/markdownlint-82e5abc1-2df2-47e5-89b8-49d2b584e7d8-issue-368-migrate-from-ortho-config-v0-9-0-to-v0-10-0-8.out
/tmp/nixie-82e5abc1-2df2-47e5-89b8-49d2b584e7d8-issue-368-migrate-from-ortho-config-v0-9-0-to-v0-10-0-8.out
```

## Implementation and candidate validation

The inherited workspace dependency and lockfile now resolve OrthoConfig v0.10.0
for `weaver-config`, `weaverd`, `weaver-cli`, and the CLI build dependency. The
lockfile contains the matching v0.10.0 macro and l10n packages. Default
features remain enabled, including `serde_json`.

| Constraint                     | Before                            | After                             |
| ------------------------------ | --------------------------------- | --------------------------------- |
| Workspace declaration          | `ortho_config = "0.9.0"` (caret)  | `ortho_config = "0.10.0"` (caret) |
| Resolved `ortho_config`        | 0.9.0                             | 0.10.0                            |
| Resolved `ortho_config_macros` | 0.9.0                             | 0.10.0                            |
| Resolved `ortho_l10n`          | Not present as a separate package | 0.10.0                            |

`cargo tree --locked -i ortho_config` shows a single resolved version shared by
every inherited consumer. `cargo tree --locked -e features -i ortho_config`
confirms the default, `serde_json`, and `toml` features remain active. The
remaining `figment` and `xdg` dependencies come through `ortho_config`, with
`uncased` below `figment`. `fluent-bundle` remains through `ortho_l10n` and the
existing `rstest-bdd`/`i18n-embed` test path; Weaver has no direct
`fluent-bundle` dependency to remove.

The application workspace declares Rust 1.90 because `tree-sitter` 0.27.1
requires it. The separately consumable `weaver-config` package declares Rust
1.89; its runtime library check passes at that floor. The stricter
`weaver-config --all-targets` check is not supported at 1.89 because its
test-only `weaver-test-macros` package declares the workspace's 1.90 floor. The
all-target test graph also includes test-only `serial_test` 4.0.1, which
requires Rust 1.93.1. The root pin remains `nightly-2026-10-26` (Rust
1.101.0-nightly, LLVM 23.1.4) and already supplies the required compiler and
components; the
[pinned CI setup action](https://github.com/leynos/shared-actions/blob/cfcc379e29be516160f499864ae82720842359af/.github/actions/setup-rust/action.yml)
reads this repository toolchain file. No dated-nightly or CI pin change was
needed. The lockfile's targeted update adds `ortho_l10n` and `syn` 3.0.9 and
removes obsolete `dirs` 5, `dirs-sys` 0.4, Redox 0.4, and Windows 0.48
packages; the remaining graph changes are required by the published release.

The v0.10.0 source changes re-key field-help identifiers to the generated
hyphenated IDs, add the new documentation metadata fields and optional-value
metadata to IR projections, compare projected IR to the installed version
constant, and render optional boolean values as `--flag[=<BOOL>]`. The config
precedence integration test now uses `MapEnv` and source-aware loading rather
than process environment mutation. Its BDD scenario verifies that an explicit
CLI `en-US` overrides file and environment values even though it repeats the
built-in default. The untouched fail-fast discovery test still passes.

| Gate or focused validation                                                   | Result                                                                                                             |
| ---------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------ |
| `make check-fmt`                                                             | Passed; 103 files unchanged.                                                                                       |
| `make typecheck`                                                             | Passed; existing manifest warnings remain.                                                                         |
| `make lint`                                                                  | Passed, including Clippy, Rustdoc, and Whitaker; existing warnings remain.                                         |
| `make test`                                                                  | Failed after fail-fast: 552 passed, 8 failed, 4 skipped, 1,002 not run.                                            |
| `make markdownlint`                                                          | Passed; 103 files, zero errors.                                                                                    |
| `make nixie`                                                                 | Passed; all diagrams validated.                                                                                    |
| `make test-workflow-contracts`                                               | Passed; 271 tests.                                                                                                 |
| `cargo test -p weaver-docs-gate`                                             | Passed; 36 tests and 12 doctests.                                                                                  |
| `cargo test -p weaver-cli --lib`                                             | Passed; 200 tests, 3 ignored.                                                                                      |
| `cargo test -p weaver-config --features cli --lib`                           | Passed; 37 tests.                                                                                                  |
| `cargo test -p weaver-config --features cli --test configuration_precedence` | Passed; 1 test.                                                                                                    |
| `cargo test -p weaver-config --features cli --test configuration_failfast`   | Passed; 2 tests.                                                                                                   |
| Rust 1.90 release workspace check, all features                              | Passed; validates the application runtime floor.                                                                   |
| Rust 1.93.1 release workspace check, all targets and features                | Passed; validates the all-target test graph.                                                                       |
| Rust 1.93.1 stable release all-target tests                                  | 1,555 passed, 7 trybuild cases failed across 3 targets, 4 ignored; nested rustc receives repository `-Zthreads=8`. |
| Pinned Nightly release all-target tests, all features                        | Passed; 1,562 tests, 4 ignored.                                                                                    |
| Pinned Nightly release workspace doctests                                    | Passed; 93 tests, 9 ignored.                                                                                       |

After the final plan edits, `make fmt` and a sequential post-format check
passed. The Markdown, diagram, and documentation-gate checks were rerun; the
only output beyond passing results was the existing Cargo workspace lint
inheritance and unused-dependency warnings, plus the Node colour warning. Final
post-format logs:

```plaintext
/tmp/check-fmt-82e5abc1-2df2-47e5-89b8-49d2b584e7d8-issue-368-doc-freeze.out
/tmp/markdownlint-82e5abc1-2df2-47e5-89b8-49d2b584e7d8-issue-368-doc-freeze.out
/tmp/nixie-82e5abc1-2df2-47e5-89b8-49d2b584e7d8-issue-368-doc-freeze.out
/tmp/cargo-test-docs-gate-82e5abc1-2df2-47e5-89b8-49d2b584e7d8-issue-368-doc-freeze.out
```

The 8 canonical `make test` failures are the same `weaver-cards` Cranelift
aborts present in the baseline: `llvm.x86.sse2.pmulhu.w is not yet supported`.
The 1,562 executable-test inventory is four greater than the baseline's 1,558;
the four skipped tests are unchanged. The release-profile run on the pinned
Nightly uses LLVM and passes the full all-target suite. The stable 1.93.1 full
test run is supplemental only: its three trybuild failures come from
Nightly-only flags inherited by nested compilations, while the stable
all-target compile check passes.

The MapEnv discovery correction is committed as `b76ea992`. Its focused
precedence test and the untouched fail-fast discovery tests pass. A first
follow-up `make lint` stopped in the worktree-local Whitaker target after an
`autocfg` build script could not write a target artifact. Cleaning only
`target/dylint/target/nightly-2026-05-28` removed 416.8 MiB, and the single
retry passed. The subsequent assertion fix is committed as `0e16d97`; its
recursive canonical projection test now compares the IR 2.0 `behaviour` and
`profiles` fields. On that candidate, `make check-fmt`, `make typecheck`,
`make lint`, `cargo test -p weaver-cli --lib` (200 passed, 3 ignored), and the
pinned-Nightly release all-target suite (1,562 passed, 4 ignored) passed.
`make test` again stopped on the same eight Cranelift aborts, with 552 passed,
4 skipped, and 1,002 not run. CodeRabbit reviewed the earlier source candidate
`b76ea992` and found the missing `behaviour` and `profiles` comparisons. The
fix addresses that finding. The follow-up review was rate-limited, then
completed on `243d3c2` after the user-directed 63-minute cooldown. It found a
mislabelled link in the technical design: the “Weaver adoption” label pointed
to the imported upstream guide instead of the Weaver ExecPlan. The link now
points to the ExecPlan, with the upstream guide retained as a separate
reference; the imported copy remains unchanged. The final documentation gates
passed: 103 files formatted, 103 Markdown files with zero errors, all diagrams
valid, and 48 docs-gate tests passed. Cargo and Markdownlint reported only the
existing workspace warnings and colour-variable warning.

Follow-up gate and review logs:

```plaintext
/tmp/lint-82e5abc1-2df2-47e5-89b8-49d2b584e7d8-issue-368-followup-2.out
/tmp/check-fmt-82e5abc1-2df2-47e5-89b8-49d2b584e7d8-issue-368-review-fix.out
/tmp/typecheck-82e5abc1-2df2-47e5-89b8-49d2b584e7d8-issue-368-review-fix.out
/tmp/lint-82e5abc1-2df2-47e5-89b8-49d2b584e7d8-issue-368-review-fix.out
/tmp/test-82e5abc1-2df2-47e5-89b8-49d2b584e7d8-issue-368-review-fix.out
/tmp/test-weaver-cli-lib-82e5abc1-2df2-47e5-89b8-49d2b584e7d8-issue-368-review-fix.out
/tmp/test-release-all-targets-82e5abc1-2df2-47e5-89b8-49d2b584e7d8-issue-368-review-fix.out
/tmp/coderabbit-82e5abc1-2df2-47e5-89b8-49d2b584e7d8-issue-368-after-mapenv.out
/tmp/coderabbit-82e5abc1-2df2-47e5-89b8-49d2b584e7d8-issue-368-review-fix.out
/tmp/coderabbit-82e5abc1-2df2-47e5-89b8-49d2b584e7d8-issue-368-final-review.out
/tmp/check-fmt-82e5abc1-2df2-47e5-89b8-49d2b584e7d8-issue-368-final-exact.out
/tmp/markdownlint-82e5abc1-2df2-47e5-89b8-49d2b584e7d8-issue-368-final-exact.out
/tmp/nixie-82e5abc1-2df2-47e5-89b8-49d2b584e7d8-issue-368-final-exact.out
/tmp/test-docs-gate-82e5abc1-2df2-47e5-89b8-49d2b584e7d8-issue-368-final-exact.out
```

The required focused and full validation log paths follow:

```plaintext
/tmp/check-fmt-82e5abc1-2df2-47e5-89b8-49d2b584e7d8-issue-368-migrate-from-ortho-config-v0-9-0-to-v0-10-0-12.out
/tmp/typecheck-82e5abc1-2df2-47e5-89b8-49d2b584e7d8-issue-368-migrate-from-ortho-config-v0-9-0-to-v0-10-0-12.out
/tmp/lint-82e5abc1-2df2-47e5-89b8-49d2b584e7d8-issue-368-migrate-from-ortho-config-v0-9-0-to-v0-10-0-12.out
/tmp/test-82e5abc1-2df2-47e5-89b8-49d2b584e7d8-issue-368-migrate-from-ortho-config-v0-9-0-to-v0-10-0-12.out
/tmp/markdownlint-82e5abc1-2df2-47e5-89b8-49d2b584e7d8-issue-368-migrate-from-ortho-config-v0-9-0-to-v0-10-0-12.out
/tmp/nixie-82e5abc1-2df2-47e5-89b8-49d2b584e7d8-issue-368-migrate-from-ortho-config-v0-9-0-to-v0-10-0-12.out
/tmp/test-workflow-contracts-82e5abc1-2df2-47e5-89b8-49d2b584e7d8-issue-368-migrate-from-ortho-config-v0-9-0-to-v0-10-0-12.out
/tmp/cargo-test-docs-gate-82e5abc1-2df2-47e5-89b8-49d2b584e7d8-issue-368-migrate-from-ortho-config-v0-9-0-to-v0-10-0-12.out
/tmp/test-weaver-cli-lib-82e5abc1-2df2-47e5-89b8-49d2b584e7d8-issue-368-3.out
/tmp/test-weaver-config-lib-82e5abc1-2df2-47e5-89b8-49d2b584e7d8-issue-368.out
/tmp/test-configuration-precedence-cli-82e5abc1-2df2-47e5-89b8-49d2b584e7d8-issue-368-2.out
/tmp/cargo-test-weaver-config-failfast-82e5abc1-2df2-47e5-89b8-49d2b584e7d8-issue-368-migrate-from-ortho-config-v0-9-0-to-v0-10-0-12.out
/tmp/msrv-1.89-weaver-config-runtime-issue-368.out
/tmp/msrv-1.90.0-workspace-82e5abc1-2df2-47e5-89b8-49d2b584e7d8-issue-368-migrate-from-ortho-config-v0-9-0-to-v0-10-0-12.out
/tmp/msrv-1.93.1-test-graph-82e5abc1-2df2-47e5-89b8-49d2b584e7d8-issue-368-migrate-from-ortho-config-v0-9-0-to-v0-10-0-12.out
/tmp/test-1.93.1-stable-release-82e5abc1-2df2-47e5-89b8-49d2b584e7d8-issue-368-migrate-from-ortho-config-v0-9-0-to-v0-10-0-12.out
/tmp/test-nightly-release-82e5abc1-2df2-47e5-89b8-49d2b584e7d8-issue-368-migrate-from-ortho-config-v0-9-0-to-v0-10-0-12.out
/tmp/doc-test-nightly-release-82e5abc1-2df2-47e5-89b8-49d2b584e7d8-issue-368-migrate-from-ortho-config-v0-9-0-to-v0-10-0-12.out
```

## Decision log

- **2026-10-26 — Import immutable documentation first.** The upstream tag and
  peeled commit match the values supplied in issue #368. Both imported files
  match the SHA-256 values above. No upstream version discrepancy was found.
- **2026-10-26 — Set compiler floors from the resolved package graph.** The
  `weaver-config` runtime graph compiles on Rust 1.89.0, matching OrthoConfig
  v0.10.0, so that published library retains its explicit 1.89 floor. The
  workspace application graph includes `tree-sitter` 0.27.1, which requires
  Rust 1.90, so the workspace floor is raised to 1.90. The all-target test
  graph also includes test-only `serial_test` 4.0.1, which requires Rust
  1.93.1. The test-only `weaver-test-macros` crate also follows the workspace
  floor, so `weaver-config` all-target checks need Rust 1.90 even though its
  runtime library check passes on 1.89. CI and local repository builds use the
  pinned Nightly 2026-10-26 toolchain, which the pinned CI setup action reads
  from `rust-toolchain.toml`; that existing dated pin already provides the
  required components.
- **2026-10-26 — Preserve the baseline Cranelift failure as separate evidence.**
  The workspace's configured development backend fails in unrelated
  `weaver-cards` tests before migration. Do not suppress, skip, or attribute
  those failures to OrthoConfig. Investigate supported compiler/backend choices
  and keep the plan's validation result explicit.
- **2026-10-26 — Preserve upstream relative-link text.** CodeRabbit found that
  three imported guide links resolve differently under Weaver's local paths. Do
  not rewrite them because the user requires byte-identical upstream copies and
  explicitly permits a documented link map. Expand the map above to name each
  source link, immutable upstream target, and local reading target.
- **2026-10-26 — Keep imported guide wording immutable.** CodeRabbit identified
  grammar and terminology edits within the two v0.10.0 upstream copies. Do not
  make them because the required SHA-256 provenance depends on byte-identical
  imports. Correct local descriptions and persist the gate evidence in this
  plan instead.

## Progress

- [x] Read `AGENTS.md`, repository contents/layout, requested skills, and the
  v0.10.0 guides before dependency, source, or test edits.
- [x] Rename the local branch to
  `issue-368-migrate-from-ortho-config-v0-9-0-to-v0-10-0` and rename the Lody
  session to the requested title.
- [x] Fetch and verify the immutable upstream tag, import both guides, and
  record their provenance and checksums.
- [x] Inventory manifests, lockfiles, loaders, localization, IR literals,
  build scripts, compiler pins, CI gates, and existing migration PRs.
- [x] Capture the sequential pre-edit baseline and record its Cranelift test
  failure, skips, and not-run count.
- [x] Complete EP-M1 documentation, provenance, and sequential docs gates.
- [x] Run CodeRabbit on the documentation commit; review the two relative-link
  findings against the user's byte-identity and mapping requirements.
- [x] Clarify the upstream-link map without editing imported files, fix the
  spelling findings, and pass the documentation gates again.
- [x] Review exact candidate `e66076b`; fix the inaccurate local contents
  description and persist its reported validation results. Preserve the
  imported guide wording under the explicit byte-identity requirement.
- [x] Gate and commit the review follow-up, then request CodeRabbit review again
  before source changes.
- [x] Complete EP-M2 dependency alignment and compiler-floor verification.
- [x] Complete EP-M3 source changes and focused regression coverage.
- [x] Complete EP-M4 active documentation, full validation, and final
  post-format documentation gates.
- [ ] Complete EP-M5 CodeRabbit review, SSH push, and draft PR readback.

## Acceptance criteria

The migration is complete when the lockfile resolves the intended published
OrthoConfig v0.10.0 packages, the configuration and help contracts above are
covered by passing focused tests, the imported documents remain byte-identical
to the pinned commit, current documentation and the generated boundary matrix
agree with the code, deterministic gates and CodeRabbit concerns are
reconciled, and the requested draft PR is created and read back. Any
unavailable or failing gate must remain identified with its exact cause and
log; it must not be reported as passing.
