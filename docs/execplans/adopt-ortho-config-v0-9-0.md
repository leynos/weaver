# Adopt `ortho-config` v0.9.0 across Weaver

This ExecPlan (execution plan) is a living document. The sections `Constraints`,
`Tolerances`, `Risks`, `Progress`, `Surprises & Discoveries`, `Decision Log`,
and `Outcomes & Retrospective` must be kept up to date as work proceeds.

Status: IN PROGRESS — remediating prerequisite Whitaker test-support debt

This document must be maintained in accordance with `AGENTS.md` at the
repository root. Drafting this plan does not authorize implementation. Await
explicit approval before changing production code, tests, dependencies, or
documentation beyond this plan.

## Purpose / big picture

After this change, Weaver builds against the published `ortho_config` v0.9.0
release instead of the temporary v0.8.0 Git revision. Operators retain the same
configuration names and precedence contract: built-in defaults are overridden
by files, then environment variables, then command-line flags. Malformed
discovered files continue to fail closed rather than being mistaken for absent
configuration.

The migration also makes two release-level effects explicit. Weaver's minimum
supported Rust version (MSRV) rises from 1.88 to 1.89 because the published
`ortho_config` v0.9.0 crate requires Rust 1.89. Missing `extends` parents are
reported with the resolved absolute missing path and the configuration file
that referenced it, giving operators enough information to repair the file.

Observable success means all of the following are true:

- `Cargo.toml` declares the caret requirement `ortho_config = "0.9.0"` and
  `rust-version = "1.89"`.
- `Cargo.lock` resolves both `ortho_config` and its transitive
  `ortho_config_macros` dependency to v0.9.0 from the crates.io registry, with
  no remaining `4339a6f3...` Git source for either package.
- `weaver` and `weaverd` keep their existing configuration flags, environment
  variable names, discovery filenames, defaults, and precedence order.
- A missing `extends` target produces a non-zero result whose diagnostic names
  both the referencing file and the resolved missing path.
- Unit and behavioural tests exercise defaults, layered precedence, malformed
  input, missing inheritance targets, and duplicate capability-override
  normalization without mutating the test process environment.
- A binary-level test proves that child-process environment configuration and
  CLI override precedence still pass through the real `weaver` composition root.
- `make check-fmt`, `make typecheck`, `make lint`, and `make test` pass after
  each completed major milestone. Documentation milestones additionally pass
  `make markdownlint` and `make nixie`.

## Constraints

1. Preserve Weaver's public configuration contract. Do not rename
   `--config-path`, `--daemon-socket`, `--log-filter`, `--log-format`,
   `--capability-overrides`, or `--locale`; do not rename the corresponding
   `WEAVER_*` variables or discovery files; and do not change the precedence
   order.
2. Keep configuration policy in `crates/weaver-config`. The CLI and daemon may
   adapt and invoke that policy through their existing `ConfigLoader` traits,
   but domain values must not acquire CLI, process, or filesystem concerns. Do
   not transplant a generic hexagonal directory layout into the repository.
3. Before adding any helper, port, or abstraction, sweep the repository with
   `leta grep`, `leta refs`, and scoped `rg` searches. If a new abstraction is
   still required, document its ownership, permitted call sites, and
   composition rules in `docs/developers-guide.md` before implementation.
4. Do not mutate the test runner's environment with `std::env::set_var` or
   `std::env::remove_var`. Use declarative `MergeComposer` layers for policy
   tests, `MapEnv` only for discovery inputs it can actually model, and
   `assert_cmd::Command::env` or `env_clear` for child-process end-to-end tests.
5. Keep `ortho_config` and the transitively resolved `ortho_config_macros`
   package on v0.9.0 together. Do not add a direct macro dependency unless
   application source imports it, which the current audit says it does not.
6. Use caret requirements for every added dependency. Add `googletest` and
   `pretty_assertions` as development-only dependencies; do not add runtime
   dependencies for test ergonomics.
7. Preserve semantic error handling. Continue exposing `OrthoError` through
   existing typed configuration boundaries and mapping it to application errors
   at CLI and daemon adapters. Do not parse human-readable error text in
   production code.
8. Treat v0.9.0 YAML parsing changes as non-applicable unless Weaver enables
   OrthoConfig's `yaml` feature. The unrelated `sempai-yaml` crate does not
   make Weaver's configuration format YAML-aware.
9. Do not adopt optional v0.9.0 metrics, agent-context commands, dependency
   aliases, or recursive subcommand documentation merely because the release
   provides them. Each requires a present Weaver consumer and must remain
   within ADR 007 and the OrthoConfig consumer-boundary matrix.
10. Use Red-Green-Refactor for behavioural changes. An expected focused red
    check may fail within a milestone, but the milestone is not complete until
    the focused checks are green and all four required Make gates pass.
11. Keep code files below 400 lines. If a touched file would exceed the limit,
    make a cohesive test-module extraction as a separate refactor after the
    migration behaviour is green.
12. Use en-GB-oxendict prose, wrap Markdown paragraphs and bullets at 80
    columns, and preserve generated-file boundaries. Update
    `docs/orthoconfig-consumer-boundary.toml` before regenerating its Markdown
    counterpart; never edit the generated table by hand.
13. Historical artefacts remain historical. Do not rewrite the v0.6.0 or
    v0.8.0 migration guides, completed ExecPlans, or archived roadmap entries
    merely to replace their period-accurate version numbers.

## Tolerances

- Scope: if production Rust changes are required outside
  `crates/weaver-config`, `crates/weaver-cli`, `crates/weaverd`, and their
  directly related tests, stop and record the compiler or runtime evidence
  before proceeding.
- Interface: if a public flag, environment variable, file name, default,
  precedence rule, error enum variant, or public Rust signature must change,
  stop and present the alternatives and compatibility consequences.
- Architecture: if satisfying the upgrade appears to require a new domain
  port, a repository-wide directory restructure, or direct calls between
  adapters, stop. Record the proposed abstraction and its reuse policy in the
  `Decision Log` and request approval.
- Dependencies: if any new runtime dependency or OrthoConfig feature is
  required beyond the v0.9.0 default features, stop and explain why. The planned
  `googletest = "0.14.3"` and `pretty_assertions = "1.4.1"` development
  dependencies are within tolerance.
- Upstream contracts: if the v0.9.0 audit proves that an ADR 007 temporary
  adapter's full removal gate is satisfied and replacing it would exceed this
  dependency migration, stop and propose a separately reviewable follow-up.
- Size: if the migration needs more than 12 non-generated files or 500 net
  lines outside documentation, lockfile, snapshots, and test fixtures, stop and
  re-scope before continuing.
- Iterations: if a focused test or any required gate remains broken after
  three distinct repair attempts, stop and record the failing command, the last
  relevant output, and the attempted remedies.
- Baseline: if any of the four required Make gates fails before migration
  changes, record it as a pre-existing blocker. Do not weaken, silence, or
  casually absorb the failure into this migration.
- Ambiguity: this plan interprets the word "netsuke" in the request as a
  carry-over name because the requested destination and current worktree are
  Weaver-specific and contain no Netsuke component. If the intended target is
  the separate Netsuke repository, stop before implementation and create a
  repository-specific plan there instead.

## Risks

- Risk: the MSRV uplift may expose new compiler or Clippy diagnostics unrelated
  to OrthoConfig. Severity: medium. Likelihood: medium. Mitigation: capture a
  green baseline, change the dependency and MSRV in one milestone, and accept
  only minimal compiler-required fixes in a separate commit with focused
  validation.

- Risk: the current configuration behavioural tests mutate global process
  environment and can interfere with parallel tests. Severity: high.
  Likelihood: certain. Mitigation: replace policy-level environment mutation
  with OrthoConfig `MergeComposer` environment layers, and retain one
  real-wiring end-to-end check that sets only a child process's environment.

- Risk: `MapEnv` can be misapplied as a complete environment-injection port.
  Severity: medium. Likelihood: medium. Mitigation: use it only when testing
  `ConfigDiscovery` selectors, XDG paths, or home lookup. The v0.9.0 guide
  explicitly says it does not replace the `WEAVER_*` configuration-value merge
  layer.

- Risk: v0.9.0's optional recursive documentation and agent-context types may
  look like permission to broaden the migration into roadmap work governed by
  ADR 007. Severity: high. Likelihood: medium. Mitigation: audit the boundary
  matrix, update only release evidence that is actually consumed, and leave
  command-surface replacement to a separate plan unless every existing removal
  condition is demonstrably met and the user approves the expansion.

- Risk: diagnostics contain temporary absolute paths, making snapshots
  host-specific. Severity: medium. Likelihood: high. Mitigation: normalize only
  the temporary-root prefix before snapshotting; keep filenames, ordering,
  error classes, and surrounding wording visible.

- Risk: moving from a Git revision to crates.io may change transitive versions
  beyond the two OrthoConfig packages. Severity: medium. Likelihood: medium.
  Mitigation: inspect `git diff -- Cargo.lock` and `cargo tree -i` output, and
  escalate if unrelated transitive churn is not required by v0.9.0 resolution.

## Progress

- [x] (2026-08-12 17:24Z) Read the repository instructions, documentation
  index, layout, v0.9.0 migration guide, v0.9.0 user's guide, relevant design
  and testing guidance, and existing v0.8.0 ExecPlan.
- [x] (2026-08-12 17:24Z) Audit the current dependency, configuration code,
  CLI/daemon adapter seams, tests, documentation references, and boundary
  manifest.
- [x] (2026-08-12 17:24Z) Confirm that the v0.9.0 Git tag and crates.io runtime
  and macro packages exist, and that the runtime package declares Rust 1.89.
- [x] (2026-08-12 17:24Z) Draft this ExecPlan without implementing it.
- [x] (2026-08-12 17:35Z) Validate the draft with targeted Markdownlint and
  Typos checks and with `make nixie`; the first full `make markdownlint` run
  exposed generated spelling-configuration drift.
- [x] (2026-08-12 17:37Z) Add the existing inline-code spelling exception to
  its authoritative local overlay, regenerate `typos.toml`, and pass the full
  `make markdownlint nixie` gate.
- [x] (2026-08-12 18:52Z) Obtain explicit approval for this draft through the
  implementation request.
- [ ] Capture and record the pre-migration gate baseline (blocked: `make test`
  generated 17 unaccepted `insta` snapshots, and `make lint` rejects existing
  `rstest-bdd` step-function assertions outside migration scope).
- [ ] (2026-08-12 18:52Z) Reconcile the pre-existing snapshot baseline before
  making further migration changes; this requires a scope decision because the
  snapshots are outside this plan's configured crates.
- [ ] (2026-08-12 19:14Z) Reconcile the pre-existing Whitaker BDD-step lint
  failures before further migration changes; an isolated `origin/main` worktree
  reproduces the same `weaverd` 89-error failure, and remediation spans
  unrelated crates beyond this plan's scope tolerance.
- [ ] (2026-08-12 21:05Z) Remediate the prerequisite Whitaker
  `no_expect_outside_tests` failures as a separate fallibility pass, beginning
  with `weaver-cards` graph-slice steps and helpers. Each affected test-support
  function returns a typed or opaque `Result`; only recognized test bodies may
  retain assertion panic boundaries. Re-run Dylint after each crate because its
  failure reporting is wave-based.
- [x] (2026-08-12 21:32Z) Convert `weaver-cards` graph-slice, get-card, cache
  BDD support, snapshot serialization, and extraction-boundary helpers to
  fallible results. `cargo test -p weaver-cards --all-features` (195 tests and
  19 doctests), strict Clippy, and targeted Whitaker pass after converting the
  two remaining shared extraction helpers.
- [x] (2026-08-12 22:06Z) Remove direct mutex-poison panics from the daemon
  test reporter and shutdown signal, and make health-status extraction total.
  `cargo test -p weaverd --lib --all-features` (290 tests) and the targeted
  Whitaker run now pass.
- [x] (2026-08-12 22:28Z) Convert `sempai-yaml` BDD steps to return errors for
  absent scenario state, unexpected diagnostics, and invalid feature data. Its
  34 tests, two doctests, strict Clippy, and targeted Whitaker run pass after
  its shared parser-test helpers began returning `Result` values.
- [x] (2026-08-12 23:04Z) Convert `weaver-plugin-rope` BDD steps, contract
  checks, and stdin/stdout dispatch helpers to return errors. Its 23 tests,
  strict Clippy, and targeted Whitaker run pass.
- [x] (2026-08-12 23:16Z) Convert `weaver-graph` BDD steps and call-hierarchy
  test builders to return errors. Its 23 tests, strict Clippy, and targeted
  Whitaker run pass.
- [x] (2026-08-12 23:31Z) Convert Sempai's engine BDD steps, diagnostic
  snapshot serializer, integration compiler helper, and tracing mutex handling
  to fallible forms. Continue with the engine and normalization-constraint
  helpers, reducing targeted Whitaker findings from 40 to 11. Its normal test
  suite still exposes a missing semantic-validation debug-event assertion that
  requires separate behavioural investigation.
- [x] (2026-08-12 20:42Z) Complete the daemon fallibility pass. Socket,
  apply-patch, process, dispatch, get-card, safety-harness, and configuration
  helpers now propagate operational failures; all 290 daemon library tests and
  targeted daemon Whitaker pass. The workspace gate now exposes independent,
  untouched `sempai-core`, `weaver-plugin-rust-analyzer`, and CLI test-support
  debt.
- [x] (2026-08-12 20:12Z) Add and run the focused missing-`extends`
  characterization test, prove the old dependency-source assertion red, then
  replace the Git pin with published v0.9.0, raise the MSRV, and verify the
  registry runtime and macro packages through `cargo tree`.
- [x] (2026-08-12 20:35Z) Replace global-environment test mutation, extend
  unit and BDD coverage, and add real binary-level configuration coverage.
- [x] (2026-08-12 20:48Z) Update active architecture, developer, user,
  contents, and boundary documentation; regenerate the boundary matrix and pass
  its dedicated manifest test.
- [ ] Complete final validation, review the diff for scope and architecture
  drift, and update the living sections and retrospective.

## Surprises & discoveries

- Observation: `make markdownlint` currently stops in its `spelling-config`
  prerequisite because regenerating `typos.toml` removes the tracked
  inline-code ignore expression and the target rejects the resulting diff.
  Evidence: `typos.toml` contained `` `[^`\n]+` ``, but `typos.local.toml` did
  not. Impact: record the existing policy in the local overlay rather than
  editing generated output. Regeneration is now idempotent, and the full
  Markdown and Mermaid gate passes.

- Observation: the request says Netsuke, but the requested file path is inside
  a Weaver worktree and every relevant source and design document describes
  Weaver. Evidence: the workspace members include `weaver-config`,
  `weaver-cli`, and `weaverd`; scoped searches find no Netsuke component.
  Impact: this plan targets Weaver and makes the interpretation an explicit
  tolerance rather than silently planning against another repository.

- Observation: Weaver does not currently use the published v0.8.0 crate
  requirement. It uses the v0.8.0 package from Git revision
  `4339a6f3c61dc4fed86493d99ffb05230bee2a1b`. Evidence: `Cargo.toml` and both
  OrthoConfig entries in `Cargo.lock` name that revision. Git history says it
  was a temporary pin until v0.9.0 settled. Impact: the migration must prove
  removal of the Git source, not merely a package-version change in the
  lockfile.

- Observation: published `ortho_config` v0.9.0 requires Rust 1.89, while the
  Weaver workspace declares Rust 1.88. Evidence:
  `cargo info ortho_config@0.9.0` reports `rust-version: 1.89.0`;
  `[workspace.package]` reports `1.88`. Impact: the MSRV uplift is a required
  part of the migration, and active developer/design documentation must change
  with it.

- Observation: Weaver already uses v0.8's declarative discovery attribute,
  `OrthoConfigDocs` metadata, runtime tracing, typed errors, and a canonical
  dependency name. It has no direct `ConfigDiscovery::load_first`, YAML
  feature, dependency alias, or direct macro dependency. Evidence: scoped
  source and manifest searches plus `leta` symbol inspection. Impact: most
  v0.9.0 migration-guide items are already satisfied or non-applicable;
  production changes should remain narrow.

- Observation: the pinned revision already contains the clearer missing-
  `extends` path diagnostic, recursive subcommand documentation, and agent
  context types even though its package metadata still says v0.8.0. Evidence:
  the cached source at revision `4339a6f3...` formats the missing path and
  referencing file in `file/path.rs` and exports the v0.9-era types. Impact: a
  missing-`extends` test is a characterization test that should pass both
  before and after the release migration. The honest red step is a
  dependency-source assertion that rejects the old Git v0.8.0 package.

- Observation: `crates/weaver-config/tests/configuration_precedence.rs` and
  `configuration_failfast.rs` directly mutate the process environment despite
  current repository policy forbidding that pattern. Evidence: both files call
  `std::env::set_var` and `remove_var` behind custom guards and locks. Impact:
  touched configuration tests must move to declarative layers and child-process
  environment injection; retaining the guards is not acceptable.

- Observation: the OrthoConfig v0.9.0 guide is present, but
  `docs/contents.md` still indexes only the v0.8.0 and v0.6.0 migration guides.
  Evidence: direct inspection of the documentation index. Impact: the
  documentation milestone must make the new guide discoverable while preserving
  the older guides as history.

- Observation: the mandatory pre-migration `make test` gate produced 17
  unaccepted `insta` snapshot candidates in
  `crates/weaver-e2e/tests/snapshots/graph_slice_python_*.snap.new`. Evidence:
  the clean starting worktree gained those files while `make test` ran, so the
  test gate was not green even though its captured output was truncated.
  Impact: the baseline tolerance applies. Do not alter or accept those
  unrelated snapshots as part of this configuration migration without explicit
  direction.

- Observation: full lockfile regeneration updated 807 lines of `Cargo.lock`,
  including many unrelated SemVer-compatible packages. A selective update now
  changes only 59 added and 39 removed lockfile lines for `ortho_config`,
  `ortho_config_macros`, and the requested test-only dependencies. Evidence:
  the focused `cargo check -p weaver-config --all-targets --all-features` run
  locked four test packages and resolved both OrthoConfig packages from the
  registry. Impact: retain the selective lockfile result; broad resolution
  churn is not part of this migration.

- Observation: `make lint` rejects existing `expect` calls in `rstest-bdd`
  step functions and their test-support helpers in `weaver-plugin-rope`,
  `sempai-yaml`, `sempai`, and `weaver-cards`. Evidence: Whitaker reports
  `no_expect_outside_tests` for functions such as `when_execute`,
  `then_parse_succeeds`, and `then_edge_types_are`. Its remediation guidance
  deliberately treats fixtures, steps, and file-backed helper functions as
  fallible support code rather than test bodies. Impact: this is real baseline
  debt requiring a separate, repository-wide fallibility refactor; do not
  weaken the lint policy as part of this migration.

- Observation: an isolated `origin/main` worktree reproduces the current
  `make lint` failure, including 89 `no_expect_outside_tests` diagnostics in
  `crates/weaverd/src/tests/support`. Evidence: `origin/main` at
  `a7655e817aec1bc364a68a7fc736f132dd3ebf0a` exits 2 using the same local
  Whitaker Dylint library. Impact: the migration did not introduce the lint
  failure; retain the tooling/repository-wide remediation blocker.

- Observation: rebuilding Whitaker's local Dylint library and reading its
  current remediation guidance confirms that file-backed test-module ancestry
  deliberately does not authorize `.expect()` in helpers. A targeted
  `cargo dylint --all -- -p weaver-cards --all-targets --all-features` reports
  39 existing support-code assertions. Impact: convert helpers and BDD steps to
  fallible results in a separate repository-wide remediation; this is not an
  OrthoConfig migration change.

- Observation: a `cfg_attr(test, allow(...))` experiment at a crate root also
  leaves the Dylint diagnostics enabled. Evidence: targeted standard Clippy
  passes but targeted Dylint continues to report all 39 pre-existing
  `weaver-cards` test assertions. Impact: remove the experiment and retain the
  repository-wide fallibility remediation as the blocker.

- Observation: Whitaker's documented `additional_test_attributes` configuration
  does not recognize `given`, `when`, `then`, `scenario`, or `rstest` here.
  Evidence: a targeted `weaver-cards` Dylint run reports the same 39 failures
  after adding those names; remove the no-op configuration. Impact: there is no
  supported repository-local configuration repair for this rule behaviour.

- Observation: returning `Result` from `rstest-bdd` step functions works with
  the existing harness and reports step setup errors without panicking.
  Evidence: the converted graph-slice, get-card, and cache scenarios pass all
  `weaver-cards` tests. Impact: use this as the standard remediation pattern
  for BDD world access and feature-data parsing.

## Decision log

- Decision: target Weaver, not a separate Netsuke checkout.
  Rationale: the destination, current worktree, source modules, and supplied
  design documents are all Weaver-specific; no Netsuke component exists here.
  Date/Author: 2026-08-12, Codex.

- Decision: replace the temporary Git revision with the published caret
  requirement `ortho_config = "0.9.0"`. Rationale: v0.9.0 is available on
  crates.io, Cargo's caret semantics match repository dependency policy, and
  Git history identifies the current SHA as temporary until the v0.9.0 release
  settled. Date/Author: 2026-08-12, Codex.

- Decision: raise Weaver's MSRV from 1.88 to 1.89 in the same dependency
  milestone. Rationale: Cargo cannot truthfully advertise compatibility below
  the runtime dependency's declared Rust 1.89 floor. Date/Author: 2026-08-12,
  Codex.

- Decision: preserve the existing feature-oriented boundaries rather than
  introduce a new port or directory hierarchy. Rationale:
  `weaver-config::Config` owns configuration policy, while the existing CLI and
  daemon `ConfigLoader` traits already isolate composition and allow
  application tests to substitute loading. The migration adds no new external
  system that needs another port. Date/Author: 2026-08-12, Codex.

- Decision: test precedence policy with `MergeComposer` and production wiring
  with a child process. Rationale: this separates pure layer policy from
  process-environment adaptation, removes forbidden global mutation, and still
  proves the real `weaver` composition root end to end. Date/Author:
  2026-08-12, Codex.

- Decision: use the resolved dependency source/version as the red-green
  migration contract, and use behavioural tests for pre/post characterization.
  Rationale: the current Git revision already contains the user-visible v0.9.0
  behaviour relevant to Weaver, so claiming that a diagnostic test fails on the
  baseline would manufacture evidence. A shell assertion on
  `cargo tree -i ortho_config` fails before the manifest change and passes
  afterwards, directly expressing the actual migration outcome. Date/Author:
  2026-08-12, Codex.

- Decision: do not create a new ADR for the dependency bump or test-harness
  correction. Rationale: ADR 007 already governs the durable Weaver/OrthoConfig
  ownership boundary. The selected changes are narrow and reversible. Record
  them in `docs/weaver-design.md` and `docs/developers-guide.md`; create a new
  ADR only if implementation requires a new public contract, port, or permanent
  divergence. Date/Author: 2026-08-12, Codex.

- Decision: do not add new Proptest, Kani, or Verus artefacts for the version
  bump. Rationale: the migration introduces no new invariant over a range of
  states, unsafe boundary, transition system, lemma, or contractual business
  logic. Existing property tests for CLI configuration-argument ordering remain
  relevant and run under `make test`. Reconsider this decision if production
  logic expands beyond compatibility fixes. Date/Author: 2026-08-12, Codex.

- Decision: retain the selective lockfile resolution and pause before
  Milestone 1 gates or CodeRabbit review. Rationale: the selective lockfile
  preserves dependency scope, but the baseline tolerance prohibits absorbing
  existing `make test` and `make lint` failures. The migration's scoped files
  are not yet ready for a deterministic green gate or meaningful review.
  Date/Author: 2026-08-12, Codex.

- Decision: use OrthoConfig's `post_merge_hook` on `Config` to normalize
  capability directives after every merge path. Rationale: direct declarative
  policy tests and generated production loading must share the same domain
  invariant. The hook belongs to `weaver-config`; CLI and daemon adapters do
  not acquire policy and no new port is introduced. Date/Author: 2026-08-12,
  Codex.

- Decision: do not ask CodeRabbit to review while the global deterministic
  gates remain red. Rationale: the user requires a clean deterministic gate
  before each review, and the recorded Whitaker and unaccepted E2E snapshot
  failures prevent a meaningful major-milestone review. Date/Author:
  2026-08-12, Codex.

- Decision: treat the repeated post-turn quality-gate request as authorization
  to repair the existing Whitaker failures that prevent completion. Rationale:
  the lint violations are deliberate house policy, cannot be configured away,
  and reproduce on `origin/main`; changing test support to return `Result` is
  the smallest durable repair. Keep this prerequisite as a distinct pass and do
  not weaken the lint or conflate it with OrthoConfig behaviour. Date/Author:
  2026-08-12, Codex.

## Outcomes & retrospective

The dependency, hermetic test, and documentation changes are implemented and
their focused tests pass. The boundary-manifest test, formatting, type
checking, and focused Clippy checks also pass. No milestone can be closed,
committed, or sent to CodeRabbit while `make test` produces unrelated E2E
snapshot candidates and `make lint` fails on existing Whitaker test-context
diagnostics. Resolve those global baseline failures outside this migration,
then rerun all gates and the required review before completing the
retrospective.

## Context and orientation

The workspace is a Rust 2024 multi-crate project. `Cargo.toml` is the source of
truth for shared package metadata and dependencies. It now declares Rust 1.89
and resolves `ortho_config` from the published v0.9.0 registry release. Every
workspace member inherits `rust-version.workspace = true`, so the root value
publishes the new MSRV consistently.

`crates/weaver-config/src/lib.rs` owns the `Config` domain type. Its
`#[derive(OrthoConfig)]` and inline `discovery(...)` attribute define the
`WEAVER` prefix, discovery filenames, `--config-path`, field defaults, CLI
names, and collection merge strategy. Its post-merge hook normalizes duplicate
capability directives for both generated and declarative merges. This is
policy, not a CLI or daemon adapter.

`crates/weaver-cli/src/config.rs` is the driving CLI adapter. Its private
`ConfigLoader` trait lets `CliRunner` load a `weaver_config::Config` without
depending on a concrete loader in unit tests. `OrthoConfigLoader` is the
production implementation. `crates/weaverd/src/bootstrap.rs` has an analogous
daemon-side `ConfigLoader` with `SystemConfigLoader` and `StaticConfigLoader`.
These existing seams already protect the application boundary.

`crates/weaver-cli/src/help.rs` reads `Config::get_doc_metadata()` through
`OrthoConfigDocs` to add configuration flags to a help-only clap command. The
runtime parser remains separate because configuration flags only take effect
before a command domain. `crates/weaver-cli/src/localizer.rs` uses
`FluentLocalizer`, `Localizer`, and `NoOpLocalizer`; the custom preflight and
argument-splitting flow means adopting v0.9.0's combined `LocalizedParse` API
is not a mechanical substitution.

`crates/weaver-config/tests/configuration_precedence.rs` binds
`tests/features/configuration_precedence.feature` with `rstest-bdd`. It now
uses OrthoConfig's `declarative::MergeComposer` to represent defaults, file,
environment, and CLI layers as owned data.
`crates/weaver-cli/tests/main_entry.rs` uses `assert_cmd` against the compiled
`weaver` binary for the real child-process environment contract.

`crates/weaver-config/tests/configuration_failfast.rs` uses `rstest` fixtures
and explicit paths for independent malformed-file and missing-inheritance
cases. It does not mutate the process environment.

The active documentation sources are `docs/weaver-design.md` for architecture
and design decisions, `docs/developers-guide.md` for internal configuration and
test practices, `docs/users-guide.md` for operator-visible behaviour, and
`docs/contents.md` for discovery. ADR 007 and
`docs/orthoconfig-consumer-boundary.toml` govern the division between reusable
OrthoConfig command-contract machinery and Weaver-specific semantic adapters.
`docs/orthoconfig-consumer-boundary.md` is generated from the TOML manifest.

## Documentation and skill signposts

An implementer must read the following before changing the corresponding area:

- `$execplans` at `/home/leynos/.codex/skills/execplans/SKILL.md` governs this
  living document, Red-Green-Refactor evidence, tolerances, and the approval
  gate.
- `$hexagonal-architecture` at
  `/home/leynos/.codex/skills/hexagonal-architecture/SKILL.md` governs inward
  dependencies and port/adapter isolation. Apply its invariants to Weaver's
  existing feature layout; do not impose its canonical folder sketch.
- `$leta` at `/home/leynos/.codex/skills/leta/SKILL.md` is the default for
  symbol, reference, implementation, and call-graph navigation.
- `$rust-router` at `/home/leynos/.codex/skills/rust-router/SKILL.md` routes
  any compile or API issue to the smallest follow-on skill.
- `$rust-unit-testing` at
  `/home/leynos/.codex/skills/rust-unit-testing/SKILL.md` governs `rstest`
  fixtures, rich assertion selection, and snapshots. Repository policy is
  stricter than that skill for environment mutation: use no direct mutation,
  even with `serial_test`.
- Load `$rust-errors` only if v0.9.0 changes a typed error boundary, and load
  `$rust-verification` followed by `$proptest`, `$kani`, or `$verus` only if
  implementation introduces an invariant or proof obligation not present in
  this draft.

Repository-relative documentation signposts are:

- `docs/ortho-config-v0-9-0-migration-guide.md` for required, recommended, and
  optional release changes;
- `docs/ortho-config-users-guide.md` for v0.9.0 APIs and operational examples;
- `docs/weaver-design.md` sections 2.1 and 2.3.1 for command/configuration
  ownership;
- `docs/rust-testing-with-rstest-fixtures.md` for fixtures and parameterized
  cases;
- `docs/rstest-bdd-users-guide.md` for feature files, step fixtures, and
  scenario binding;
- `docs/reliable-testing-in-rust-via-dependency-injection.md` for isolating
  external state;
- `docs/rust-doctest-dry-guide.md` if a changed public API needs a new worked
  Rustdoc example;
- `docs/complexity-antipatterns-and-refactoring-strategies.md` for keeping test
  harnesses cohesive rather than replacing one global-state tangle with many
  tiny helpers; and
- `docs/documentation-style-guide.md` for design decisions, ADR thresholds,
  links, formatting, and prose.

## Plan of work

### Milestone 0: capture the baseline

Record the starting dependency sources, MSRV, relevant APIs, and working-tree
state. Run all four required Make gates before editing code. This distinguishes
upgrade failures from existing branch failures. Also run the focused
configuration, CLI entrypoint, and boundary-manifest tests once so later
commands have a direct comparison.

Do not edit any file in this milestone. Record command results in `Progress`
and unexpected facts in `Surprises & Discoveries`. If the baseline is not
green, apply the baseline tolerance and stop.

Milestone acceptance: all four required gates exit zero, focused tests pass,
and the plan records the resolved v0.8.0 Git source and Rust 1.88 baseline.

### Milestone 1: prove and perform the compatibility upgrade

Start with a characterization test in
`crates/weaver-config/tests/configuration_failfast.rs`. Add an `rstest` case
that writes a `weaver.toml` containing `extends = "missing.toml"`, calls
`Config::load_from_iter` with an explicit `--config-path`, and uses
`googletest::contains_substring` to require both the referencing file and the
resolved missing target. Normalize the temporary directory prefix and capture
the stable diagnostic with `insta`. Run it on the Git-pinned baseline and
record that it passes; this proves the source migration preserves existing
operator-visible behaviour.

Then run a dependency-source assertion that requires the registry v0.9.0
package. It must fail on the baseline because `cargo tree` reports the Git
v0.8.0 package. This is the red step for a dependency-source migration whose
relevant behaviour is already present in the temporary pin.

Then update `[workspace.package].rust-version` in `Cargo.toml` from `1.88` to
`1.89`, replace the Git dependency with `ortho_config = "0.9.0"`, and add
workspace development dependencies `googletest = "0.14.3"` and
`pretty_assertions = "1.4.1"`. Add the three relevant test dependencies
(`googletest`, `pretty_assertions`, and the existing workspace `insta`) to
`crates/weaver-config/Cargo.toml`. Keep the runtime feature set at its current
default; do not enable YAML or metrics.

Update the lockfile through Cargo, then verify runtime and macro versions and
sources. Compile the focused crates. Make only minimal source corrections
forced by the v0.9.0 API or Rust 1.89 floor, and preserve existing public
signatures and typed errors. Re-run the characterization test, require the
dependency-source assertion to turn green, review the snapshot deliberately,
then close the milestone with all four Make gates.

Milestone acceptance: the diagnostic characterization test passes before and
after the change, the dependency-source assertion changes from red to green,
the lockfile contains registry v0.9.0 entries for both OrthoConfig packages, no
OrthoConfig Git source remains, and all required gates exit zero.

### Milestone 2: make configuration tests hermetic and complete

Refactor `crates/weaver-config/tests/configuration_precedence.rs` so its
`rstest-bdd` world accumulates owned declarative layers rather than setting
process environment. Use `ortho_config::declarative::MergeComposer` for
defaults, file, environment, and CLI inputs, then call
`Config::merge_from_layers`. Keep setup, query, and assertion separate. Make
fallible fixtures or steps return `Result` and propagate with `?`; remove
`ENV_LOCK`, `EnvGuard`, unsafe environment calls, and panic-heavy setup.

Use `pretty_assertions::assert_eq` for full domain values and matrices, and use
GoogleTest matchers for option/result variants, collections, and diagnostic
substrings. Extend the feature file with the following synchronized
specification:

```gherkin
Feature: Configuration precedence under OrthoConfig v0.9.0

  Scenario: CLI overrides environment and configuration file values
    Given a configuration file setting the locale to "en-GB"
    And an environment layer setting the locale to "fr-FR"
    When a CLI layer sets the locale to "de-DE"
    Then the resolved locale is "de-DE"

  Scenario: Defaults apply when no external layers are present
    When the configuration layers are merged without overrides
    Then the built-in Weaver defaults are returned

  Scenario: Invalid higher-precedence input fails closed
    Given a configuration file setting the locale to "en-GB"
    When an environment layer sets the locale to "not_a_locale"
    Then configuration loading reports an invalid locale

  Scenario: The last duplicate capability directive wins
    Given lower layers allow the Rust rename capability
    When a CLI layer denies the Rust rename capability
    Then the resolved capability matrix denies the Rust rename capability
```

Refactor `configuration_failfast.rs` to remove `EnvOverride` and its mutex. Use
`rstest` fixtures for the temporary directory and explicit config paths. Retain
happy and unhappy typed-error checks, and snapshot normalized multi-variant
diagnostic output only where it improves format consistency.

Add end-to-end cases to `crates/weaver-cli/tests/main_entry.rs` using the
compiled `weaver` binary. Each command must call `env_clear()`, set an explicit
temporary home/XDG location as needed, and use child-only `.env(...)` values.
At minimum, prove that an invalid `WEAVER_LOCALE` fails
`weaver --capabilities`, that a valid `--locale` argument overrides the invalid
child environment and succeeds, and that a missing `extends` parent exits
non-zero with both paths in stderr. Use `rstest` for repeated cases and the
requested rich assertion crates for output semantics.

Do not add `MapEnv` merely to test the upstream type. If implementation finds a
Weaver-owned discovery-input test, use `MapEnv` there; otherwise record it as
non-applicable because the full `WEAVER_*` value layer is covered by
declarative policy tests plus a child-process adapter test.

Run focused unit, BDD, and binary tests after each refactor. Close the
milestone only after the four required Make gates pass.

Milestone acceptance: no test under `crates/weaver-config` directly mutates
process environment, the four Gherkin scenarios pass, the binary tests prove
the real composition root, existing argument-ordering Proptest cases still
pass, and all required gates exit zero.

### Milestone 3: synchronize design and operational documentation

Update `docs/contents.md` to index the v0.9.0 migration guide as current and
describe v0.8.0 and v0.6.0 guides as historical. Do not overwrite the supplied
v0.9.0 migration guide or user's guide unless repository-local link or format
validation requires a narrow correction.

Update `docs/weaver-design.md` configuration sections to say v0.9.0 and Rust
1.89, preserve the existing strict file-failure and precedence semantics, and
record the accepted migration decisions in its design decision log. State that
Weaver retains its existing policy and adapter boundaries, and that optional
agent-context, recursive command-metadata, localization, metrics, and YAML work
remains governed by ADR 007 and roadmap-specific removal gates.

Update `docs/developers-guide.md` workspace baseline and configuration
internals. Document the test boundary: `MergeComposer` exercises source-layer
policy without global state, while `assert_cmd` child processes prove the
production environment adapter. Explicitly document that `MapEnv` covers
discovery inputs but not `WEAVER_*` value merging. Update `docs/users-guide.md`
only for observable behaviour: add the clearer missing `extends` diagnostic and
the Rust 1.89 installation requirement if the guide owns installation
prerequisites. Do not invent a UI change.

Review active non-historical references found by:

```sh
rg -n 'v0\.8\.0|0\.8\.0|Rust 1\.88|4339a6f3' \
  README.md Cargo.toml docs crates \
  --glob '!docs/archive/**' \
  --glob '!docs/execplans/**' \
  --glob '!docs/ortho-config-v0-8-0-migration-guide.md' \
  --glob '!docs/ortho-config-v0-6-0-migration-guide.md'
```

Update `docs/rust-extricate-actuator-plugin-technical-design.md` where it
describes Weaver's current dependency, while retaining historical v0.8.0 links
that explain a past migration. In `docs/orthoconfig-consumer-boundary.toml`,
replace the temporary SHA evidence for the already-consumed boundary with
`v0.9.0` and update its review date only after confirming the release contains
that contract. Audit all `wraps` and `pending` rows against v0.9.0, but do not
claim consumption without a Weaver call site. Regenerate
`docs/orthoconfig-consumer-boundary.md` with the documented example command.

Run `make fmt` after documentation edits, inspect its complete changed-file
list, and retain only in-scope formatter changes. Run the four required Make
gates plus `make markdownlint` and `make nixie` before closing the milestone.

Milestone acceptance: active documentation names v0.9.0 and Rust 1.89 where
current state matters, user-visible missing-inheritance behaviour is accurate,
the generated boundary matrix matches its manifest, historical records remain
truthful, and all six gates exit zero.

### Milestone 4: final system proof and hand-off

Inspect the final diff for dependency, architecture, and scope drift. Use
`cargo tree` to prove the resolved OrthoConfig packages, and use `git diff` to
confirm no optional v0.9.0 feature or unrelated dependency was enabled. Search
tests for forbidden environment mutation and source for stale Git pins.

Run focused configuration, BDD, binary, and docs-gate tests one final time,
then run every required repository gate. Update `Progress`,
`Surprises & Discoveries`, `Decision Log`, and `Outcomes & Retrospective` with
actual commands and concise evidence. Change status to `COMPLETE` only when no
required work remains.

If committing is requested, keep the history reviewable: first commit the
tested dependency/MSRV compatibility change, then commit hermetic test
refactoring only if it is materially separable, and finish with synchronized
documentation. Do not commit a milestone whose required gates fail.

Milestone acceptance: the stated observable outcomes all hold, every required
gate passes, the worktree contains only intentional changes, and the living
plan records the final evidence.

## Concrete steps

Run all commands from the repository root:
`/data/leynos/Projects/weaver.worktrees/adopt-ortho-config-v0-9-0`.

1. Capture baseline state and dependency evidence:

   ```sh
   git status --short --branch
   rustc --version
   cargo info ortho_config@0.9.0
   rg -n 'ortho_config|rust-version' Cargo.toml crates/*/Cargo.toml
   rg -n -A4 -B2 'name = "ortho_config"|name = "ortho_config_macros"' Cargo.lock
   cargo test -p weaver-config --all-features
   cargo test -p weaver-cli --test main_entry
   cargo test -p weaver-docs-gate --test boundary_manifest -- --nocapture
   ```

2. Run and record the baseline milestone gates separately so a failed command
   cannot be hidden by a later success:

   ```sh
   set -o pipefail; make check-fmt 2>&1 | tee /tmp/weaver-ortho-v0-9-check-fmt.baseline.log
   set -o pipefail; make typecheck 2>&1 | tee /tmp/weaver-ortho-v0-9-typecheck.baseline.log
   set -o pipefail; make lint 2>&1 | tee /tmp/weaver-ortho-v0-9-lint.baseline.log
   set -o pipefail; make test 2>&1 | tee /tmp/weaver-ortho-v0-9-test.baseline.log
   ```

3. Add and run the missing-`extends` characterization test before changing the
   dependency:

   ```sh
   cargo test -p weaver-config --test configuration_failfast \
     missing_extends_reports_referencing_and_resolved_paths -- --exact --nocapture
   ```

   Expect this test to pass because the temporary Git revision already contains
   the clearer diagnostic. Then prove the dependency-source contract red:

   ```sh
   cargo tree -i ortho_config | rg '^ortho_config v0\.9\.0 '
   ```

   Expected red evidence is a non-zero pipeline status because the baseline
   output resembles:

   ```plaintext
   ortho_config v0.8.0 (https://github.com/leynos/ortho-config.git?rev=4339a6f3...)
   ```

   Record the actual command and output; do not copy the illustrative text as
   if it were observed.

4. Edit the root manifest and relevant dev-dependencies, then update and inspect
   the lockfile:

   ```sh
   cargo update -p ortho_config
   cargo tree -i ortho_config
   cargo tree -i ortho_config_macros
   rg -n -A4 -B2 'name = "ortho_config"|name = "ortho_config_macros"' Cargo.lock
   rg -n '4339a6f3|git\+https://github.com/leynos/ortho-config' Cargo.toml Cargo.lock
   ```

   The last search must return no matches. The tree and lockfile output must
   show v0.9.0 for both packages.

5. Prove green and close Milestone 1:

   ```sh
   cargo test -p weaver-config --test configuration_failfast \
     missing_extends_reports_referencing_and_resolved_paths -- --exact --nocapture
   cargo tree -i ortho_config | rg '^ortho_config v0\.9\.0 '
   set -o pipefail; make check-fmt 2>&1 | tee /tmp/weaver-ortho-v0-9-check-fmt.m1.log
   set -o pipefail; make typecheck 2>&1 | tee /tmp/weaver-ortho-v0-9-typecheck.m1.log
   set -o pipefail; make lint 2>&1 | tee /tmp/weaver-ortho-v0-9-lint.m1.log
   set -o pipefail; make test 2>&1 | tee /tmp/weaver-ortho-v0-9-test.m1.log
   ```

6. Refactor and extend the tests, then run focused proof:

   ```sh
   cargo test -p weaver-config --all-features --test configuration_precedence -- --nocapture
   cargo test -p weaver-config --all-features --test configuration_failfast -- --nocapture
   cargo test -p weaver-cli --test main_entry configuration -- --nocapture
   rg -n 'std::env::(set_var|remove_var)' crates/weaver-config --glob '*.rs'
   ```

   The final search must return no matches. Then close Milestone 2 with the
   same four Make commands, writing logs with the `.m2.log` suffix.

7. Regenerate the governed boundary document after editing its TOML source:

   ```sh
   cargo run -p weaver-docs-gate --example render_boundary_matrix -- \
     docs/orthoconfig-consumer-boundary.toml \
     docs/orthoconfig-consumer-boundary.md
   cargo test -p weaver-docs-gate --test boundary_manifest -- --nocapture
   ```

8. Format and close the documentation milestone:

   ```sh
   make fmt
   git status --short
   git diff --check
   set -o pipefail; make check-fmt 2>&1 | tee /tmp/weaver-ortho-v0-9-check-fmt.m3.log
   set -o pipefail; make typecheck 2>&1 | tee /tmp/weaver-ortho-v0-9-typecheck.m3.log
   set -o pipefail; make lint 2>&1 | tee /tmp/weaver-ortho-v0-9-lint.m3.log
   set -o pipefail; make test 2>&1 | tee /tmp/weaver-ortho-v0-9-test.m3.log
   set -o pipefail; make markdownlint 2>&1 | tee /tmp/weaver-ortho-v0-9-markdownlint.m3.log
   set -o pipefail; make nixie 2>&1 | tee /tmp/weaver-ortho-v0-9-nixie.m3.log
   ```

9. Run final audits and gates:

   ```sh
   cargo tree -i ortho_config
   cargo tree -i ortho_config_macros
   rg -n 'std::env::(set_var|remove_var)' crates/weaver-config --glob '*.rs'
   rg -n '4339a6f3|git\+https://github.com/leynos/ortho-config' Cargo.toml Cargo.lock
   git diff --check
   git diff --stat
   git status --short --branch
   set -o pipefail; make check-fmt 2>&1 | tee /tmp/weaver-ortho-v0-9-check-fmt.final.log
   set -o pipefail; make typecheck 2>&1 | tee /tmp/weaver-ortho-v0-9-typecheck.final.log
   set -o pipefail; make lint 2>&1 | tee /tmp/weaver-ortho-v0-9-lint.final.log
   set -o pipefail; make test 2>&1 | tee /tmp/weaver-ortho-v0-9-test.final.log
   set -o pipefail; make markdownlint 2>&1 | tee /tmp/weaver-ortho-v0-9-markdownlint.final.log
   set -o pipefail; make nixie 2>&1 | tee /tmp/weaver-ortho-v0-9-nixie.final.log
   ```

## Validation and acceptance

Red-Green-Refactor evidence is mandatory where behaviour changes. For this
source migration, record the nearest observable equivalent:

- Characterize: the focused missing-`extends` test passes on the Git-pinned
  baseline and records the operator-visible contract that must not regress.
- Red: `cargo tree -i ortho_config | rg '^ortho_config v0\.9\.0 '` returns
  non-zero while the workspace still resolves the Git v0.8.0 package.
- Green: after the manifest, lockfile, and MSRV changes, the same dependency
  assertion and the characterization test both pass.
- Refactor: the environment-mutating harnesses are replaced with declarative
  layer tests and child-process tests; focused suites and all four Make gates
  remain green.

The migration is accepted only when a reviewer can observe all of the following:

- `cargo tree -i ortho_config` and `cargo tree -i ortho_config_macros` show only
  v0.9.0 registry packages.
- `cargo metadata --locked --format-version 1` succeeds with Rust 1.89 as the
  workspace floor.
- The named `rstest` missing-inheritance case asserts both absolute paths and
  its reviewed `insta` snapshot contains no host-specific temporary root.
- The four named BDD scenarios pass through `rstest-bdd`.
- Binary tests execute `weaver --capabilities` with isolated child
  environments and prove both failure and CLI-override success paths.
- Existing CLI ordering property tests pass. No new property/model/proof suite
  is required unless production logic changes beyond this plan.
- `rg -n 'std::env::(set_var|remove_var)' crates/weaver-config --glob '*.rs'`
  returns no matches.
- `make check-fmt`, `make typecheck`, `make lint`, `make test`,
  `make markdownlint`, and `make nixie` all exit zero at final validation.

Expected final gate transcript shape is:

```plaintext
make check-fmt    exit 0
make typecheck    exit 0
make lint         exit 0
make test         exit 0
make markdownlint exit 0
make nixie        exit 0
```

Replace this illustrative block with concise actual evidence in
`Outcomes & Retrospective`; do not invent test counts.

## Idempotence and recovery

All inspection, focused test, formatting, and validation commands are safe to
repeat. Cargo lockfile regeneration is deterministic for the manifest
constraints; rerun `cargo update -p ortho_config` after correcting a partial
manifest edit. Review `Cargo.lock` rather than hand-editing package entries.

Test fixtures own temporary directories and child-process environments. They
must clean up through RAII and must not rely on restoration after a panic.
Snapshot tests normalize temporary roots before assertion; never accept
machine-specific paths into checked-in snapshots.

If the boundary matrix regeneration fails, correct
`docs/orthoconfig-consumer-boundary.toml` and rerun the generator. Do not edit
the generated Markdown table. If `make fmt` changes unrelated Markdown, inspect
the changed-file list and revert only those unrelated formatter changes with a
targeted patch; do not reset the worktree.

If a milestone fails after edits, preserve the failing test and logs, update
the living sections, and repair forward. Do not use `git reset --hard`, broad
checkout commands, or destructive cleanup. Temporary logs live under `/tmp` and
are not repository artefacts.

## Artefacts and notes

The implementation should leave these durable artefacts:

- updated `Cargo.toml` and `Cargo.lock` dependency/MSRV evidence;
- focused unit and `rstest-bdd` tests under `crates/weaver-config/tests/`;
- reviewed `insta` snapshots under the corresponding `snapshots/` directory;
- binary-level configuration tests in `crates/weaver-cli/tests/main_entry.rs`;
- updated `docs/contents.md`, `docs/weaver-design.md`,
  `docs/developers-guide.md`, and, when observable behaviour warrants it,
  `docs/users-guide.md`;
- updated boundary manifest and regenerated matrix; and
- this ExecPlan with actual progress, decisions, evidence, and retrospective.

Do not create a new ADR unless a tolerance is crossed. Do not create Kani/Verus
harnesses or new property tests merely to satisfy a tool checklist; the
verification technique must match an introduced invariant.

## Interfaces and dependencies

The final runtime dependency declaration must be:

```toml
[workspace.package]
rust-version = "1.89"

[workspace.dependencies]
ortho_config = "0.9.0"
```

The final shared test dependencies must include:

```toml
[workspace.dependencies]
googletest = "0.14.3"
insta = "1.41"
pretty_assertions = "1.4.1"
```

`crates/weaver-config` may consume those three only from `[dev-dependencies]`.
The existing `Config` public shape and methods remain:

```rust
pub struct Config {
    pub daemon_socket: SocketEndpoint,
    pub log_filter: String,
    pub log_format: LogFormat,
    pub capability_overrides: Vec<CapabilityDirective>,
    pub locale: Locale,
}

impl Config {
    pub fn load() -> ortho_config::OrthoResult<Self>;
    pub fn load_from_iter<I, T>(iter: I) -> ortho_config::OrthoResult<Self>
    where
        I: IntoIterator<Item = T>,
        T: Into<std::ffi::OsString> + Clone;
}
```

The existing adapter traits remain owned by their application crates:

```rust
// crates/weaver-cli/src/config.rs
pub(crate) trait ConfigLoader {
    fn load(&self, args: &[std::ffi::OsString]) -> Result<Config, AppError>;
}

// crates/weaverd/src/bootstrap.rs
pub trait ConfigLoader: Send + Sync {
    fn load(&self) -> Result<Config, std::sync::Arc<ortho_config::OrthoError>>;
}
```

No new domain port is planned. `MergeComposer` is test input construction, not
a production adapter. `assert_cmd` is the process-boundary adapter for end-to-
end tests. `MapEnv` is an optional discovery-input fake and must not be
presented as control over the full configuration-value environment layer.

## Revision note

This plan entered execution on 2026-08-12 after explicit approval. A selective
lockfile update replaced a broad regeneration attempt. The baseline gates still
expose unrelated E2E snapshots and BDD-step lint failures. The plan is paused
before a completed implementation milestone or CodeRabbit review until the user
decides whether those out-of-scope artefacts should be remediated, accepted, or
excluded from the migration baseline.
