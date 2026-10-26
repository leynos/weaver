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
inside either upstream copy. Relative links in the imported files retain their
upstream text; their targets map as follows:

| Upstream target                                                                                                                      | Local or immutable target                                                                |
| ------------------------------------------------------------------------------------------------------------------------------------ | ---------------------------------------------------------------------------------------- |
| `users-guide.md`                                                                                                                     | `docs/ortho-config-users-guide.md`                                                       |
| `v0-9-0-migration-guide.md`                                                                                                          | `docs/ortho-config-v0-9-0-migration-guide.md`                                            |
| `README.md`, `CHANGELOG.md`, `examples/hello_world/`                                                                                 | Same paths at commit `38812c563e3953a490e3efc202609f37e21a0517` in `leynos/ortho-config` |
| `ortho_l10n/README.md`                                                                                                               | Same path at that commit                                                                 |
| `docs/adrs/adr-0004-cargo-external-subcommand-entry-point.md`, `docs/cargo-orthohelp-design.md`, `docs/cargo-orthohelp-api-guide.md` | Same paths at that commit                                                                |

The guides describe v0.10.0 only. Planned or “not yet available” items are not
available APIs and are excluded from this migration.

## Baseline and repository inventory

The requested survey baseline is `41cab0d660e0a8d75216c63207f5fbf6fa0a185d`.
The actual clean source checkout before this task was
`5efe82fc4bc844102b0c93a78e34abd7199e0730`, four commits later on
`origin/main`. The only pre-implementation worktree changes are the two
imported documents. The compiler reports
`rustc 1.101.0-nightly (76456a688bc0ed897fa23a677d4ddb5b3fdb575a 2026-10-25)`,
LLVM 23.1.4, from the repository pin `nightly-2026-10-26`. The workspace
already declares Rust 1.89.0, which is the v0.10.0 floor.

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
| `DocMetadata` adds `profiles`                                         | Applicable: name `profiles: None` in the library-side projection literals                                                              | Command IR construction and canonical projection tests                                     |
| Boolean flags accept `--flag=false`; spaced false remains invalid     | Applicable to the derived boolean test fixture and metadata projection; no production OrthoConfig bool flag needs a user-facing change | Help parser tests for attached false and rejected spaced false                             |
| Named profiles                                                        | Not adopted: no product requirement; would add a new configuration tier and reserve names                                              | Keep the existing precedence contract                                                      |
| Stacked discovery scopes                                              | Not adopted: first-wins discovery is part of current behavior                                                                          | Leave `configuration_failfast.rs` unchanged                                                |
| Injected environment sources with `MapEnv`                            | Adopt for the precedence test harness because v0.10.0 exposes source-aware loaders                                                     | No process environment mutation in the touched test; source-layer and discovery tests pass |
| Infer string defaults with `cli_default_as_absent`                    | Not applicable: Weaver has no derived field that duplicates a clap string default                                                      | Field-attribute inventory                                                                  |
| Agent-native behaviour metadata and policy checks                     | Not adopted: no `cargo-orthohelp` consumer or package policy table                                                                     | Manifest, build-script, and CI inventory                                                   |
| Cargo external-subcommand helper                                      | Not adopted: Weaver does not provide a Cargo external subcommand                                                                       | CLI entry-point inventory                                                                  |
| Standalone `ortho_l10n` runtime                                       | Not adopted: localization is used through the `ortho_config` facade                                                                    | Existing `FluentLocalizer` imports and dependency inventory                                |
| `cargo-orthohelp` stdout, policy report, and bridge build environment | Inapplicable: Weaver does not install or invoke that tool                                                                              | CI, Makefile, build-script, and metadata inventory                                         |
| Missing, malformed, and unreadable files remain distinct              | Preserve existing fail-fast behavior; do not change the dedicated fail-fast test                                                       | `configuration_failfast.rs` remains byte-for-byte unchanged and passes                     |
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
logs are the evidence for the current plan and index edits:

```plaintext
/tmp/check-fmt-82e5abc1-2df2-47e5-89b8-49d2b584e7d8-issue-368-migrate-from-ortho-config-v0-9-0-to-v0-10-0-2.out
/tmp/markdownlint-82e5abc1-2df2-47e5-89b8-49d2b584e7d8-issue-368-migrate-from-ortho-config-v0-9-0-to-v0-10-0-2.out
/tmp/nixie-82e5abc1-2df2-47e5-89b8-49d2b584e7d8-issue-368-migrate-from-ortho-config-v0-9-0-to-v0-10-0-2.out
```

## Decision log

- **2026-10-26 — Import immutable documentation first.** The upstream tag and
  peeled commit match the values supplied in issue #368. Both imported files
  match the SHA-256 values above. No upstream version discrepancy was found.
- **2026-10-26 — Keep the Rust floor at 1.89.0 unless dependency resolution
  proves it insufficient.** The workspace already declares the v0.10.0 minimum;
  no automatic MSRV bump is justified by the version alone.
- **2026-10-26 — Preserve the baseline Cranelift failure as separate evidence.**
  The workspace's configured development backend fails in unrelated
  `weaver-cards` tests before migration. Do not suppress, skip, or attribute
  those failures to OrthoConfig. Investigate supported compiler/backend choices
  and keep the plan's validation result explicit.

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
- [ ] Commit EP-M1 and request CodeRabbit review; clear concerns before source
  changes.
- [ ] Complete EP-M2 through EP-M5 with focused tests, sequential gates,
  CodeRabbit review, SSH push, and draft PR readback.

## Acceptance criteria

The migration is complete when the lockfile resolves the intended published
OrthoConfig v0.10.0 packages, the configuration and help contracts above are
covered by passing focused tests, the imported documents remain byte-identical
to the pinned commit, current documentation and the generated boundary matrix
agree with the code, deterministic gates and CodeRabbit concerns are
reconciled, and the requested draft PR is created and read back. Any
unavailable or failing gate must remain identified with its exact cause and
log; it must not be reported as passing.
