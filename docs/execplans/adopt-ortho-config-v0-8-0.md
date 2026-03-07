# Adopt `ortho-config` v0.8.0 Across Weaver

This ExecPlan (execution plan) is a living document. The sections
`Constraints`, `Tolerances`, `Risks`, `Progress`, `Surprises & Discoveries`,
`Decision Log`, and `Outcomes & Retrospective` must be kept up to date as work
proceeds.

Status: COMPLETE

This document must be maintained in accordance with `AGENTS.md` at the
repository root.

## Purpose / big picture

After this change, the Weaver workspace builds against `ortho_config` v0.8.0,
advertises Rust 1.88 or newer consistently, and keeps its configuration
behaviour unchanged for operators. The shared config loader in
`crates/weaver-config/` must still discover `weaver.toml`, `.weaver.toml`,
`config.toml`, `WEAVER_*` environment variables, and the existing CLI flags
exactly as it does today; the migration is a dependency and tooling upgrade,
not a feature redesign.

Observable success after implementation:

- `Cargo.toml` declares `ortho_config = "0.8.0"` in
  `[workspace.dependencies]`.
- The workspace and member manifests surface Rust 1.88+ rather than the
  current Rust 1.85 floor.
- `cargo` resolves both `ortho_config` and `ortho_config_macros` to
  `0.8.0` in `Cargo.lock`.
- `make check-fmt`, `make lint`, `make test`, `make markdownlint`,
  `make fmt`, and `make nixie` all pass when run through `tee` with
  `set -o pipefail`.
- Operator-facing documentation no longer claims that Weaver uses
  `ortho-config` v0.6.0 or Rust 1.85.
- The migration notes that do not apply to Weaver
  (`crate =` aliasing, `cli_default_as_absent`, generated `orthohelp` artefacts
  unless proven otherwise) are documented explicitly so future maintainers do
  not re-audit the same questions from scratch.

## Constraints

1. Preserve the current configuration contract. `weaver-config::Config`
   must keep the current discovery rules, precedence order, environment
   variable names, CLI flag names, and default values unless a migration note
   requires a source-compatible rewrite.
2. Keep `ortho_config` and `ortho_config_macros` in lockstep at `0.8.0`.
   The macro crate is not declared directly today, so lockfile inspection is
   required after the upgrade.
3. Raise the Rust floor to 1.88 or newer in a way Cargo will actually
   surface. Updating only `[workspace.package].rust-version` is insufficient
   because many member crates currently omit `rust-version.workspace = true`.
4. Do not introduce direct dependencies on `figment`, `uncased`, or `xdg`
   unless a source file genuinely needs them for non-generated code.
   Derive-adjacent imports and examples should prefer the `ortho_config::...`
   re-exports.
5. Preserve the existing localisation behaviour in
   `crates/weaver-cli/src/localizer.rs`. The Fluent-backed localiser and
   English fallbacks must keep working after the dependency bump.
6. Treat the `orthohelp` metadata flow as conditional. The current
   repository generates CLI man pages via build scripts, but the audit found no
   `OrthoConfigDocs`, `cargo orthohelp`, or `[package.metadata.ortho_config]`
   wiring. Do not add speculative metadata unless a concrete documentation
   artefact in this repository needs it.
7. Historical documents should remain truthful. Do not silently rewrite
   the versioned v0.6.0 migration guide to pretend it was always about v0.8.0.
   If new versioned migration guidance is needed, add a new guide.
8. Use the repository quality gates and the Makefile targets where they
   exist. Long-running commands must use `tee` and `set -o pipefail`.
9. Comments and documentation must use en-GB-oxendict spelling and remain
   wrapped to 80 columns.

## Tolerances

- If the upgrade changes any public CLI flag name, environment variable
  name, config file name, or precedence rule, stop and escalate before
  proceeding.
- If `ortho_config` v0.8.0 requires source edits outside the workspace
  manifests, `crates/weaver-config/`, `crates/weaver-cli/`, `crates/weaverd/`,
  `README.md`, and the configuration-related docs, stop and re-evaluate the
  scope.
- If a new runtime dependency is required beyond the `ortho_config`
  version change, stop and escalate.
- If `cargo orthohelp` is absent and a repository artefact truly depends on
  it, stop and ask whether installing an additional Cargo subcommand is
  acceptable.
- If `make test` reproduces the known pre-existing hang in
  `weaver_cli::tests::unit::auto_start::auto_start_succeeds_and_proceeds`
  before the ortho-config-specific diffs are complete, record that as a blocker
  with log evidence instead of treating it as a migration failure.
- If five successive fix attempts still leave the workspace failing to
  compile, lint, or test, stop and escalate with the captured logs.

## Risks

- Risk: Rust-version propagation is incomplete today. The root
  `Cargo.toml` sets `rust-version = "1.85"`, but these manifests currently do
  not inherit it: `crates/weaver-build-util/Cargo.toml`,
  `crates/weaver-cli/Cargo.toml`, `crates/weaver-e2e/Cargo.toml`,
  `crates/weaver-graph/Cargo.toml`, `crates/weaver-lsp-host/Cargo.toml`,
  `crates/weaver-plugin-rope/Cargo.toml`,
  `crates/weaver-plugin-rust-analyzer/Cargo.toml`,
  `crates/weaver-plugins/Cargo.toml`, `crates/weaver-sandbox/Cargo.toml`, and
  `crates/weaverd/Cargo.toml`. Severity: medium. Likelihood: certain.
  Mitigation: normalise every member manifest to
  `rust-version.workspace = true` as part of the migration.

- Risk: The repository still documents `ortho-config` v0.6.0 in multiple
  places, and one completed ExecPlan documents the v0.7.0 upgrade. Severity:
  medium. Likelihood: certain. Mitigation: update active operator/developer
  docs (`README.md`, `docs/users-guide.md`, `docs/weaver-design.md`,
  `docs/ortho-config-users-guide.md`, `docs/contents.md`, and
  `docs/roadmap.md`) while leaving historical execplans untouched.

- Risk: `weaver-config/src/lib.rs` wraps derive-generated loaders and
  currently carries `#[allow(unfulfilled_lint_expectations)]` around
  `#[expect(clippy::missing_panics_doc)]`. A newer toolchain or macro expansion
  could change the lint surface. Severity: medium. Likelihood: medium.
  Mitigation: re-run `make lint` after the version bump before making unrelated
  refactors, and only touch the suppression if the new expansion makes it
  necessary.

- Risk: The migration notes mention `cli_default_as_absent`, crate aliasing,
  and `SelectedSubcommandMerge`, none of which appear in the current source.
  Severity: low. Likelihood: low. Mitigation: keep the audit evidence in the
  migration guide so future maintainers know these notes were considered and
  found non-applicable.

- Risk: The repository documents YAML behaviour in its ortho-config guides
  even though Weaver itself currently uses TOML discovery. Severity: low.
  Likelihood: medium. Mitigation: update the generic documentation to match
  v0.8.0 semantics while clarifying that Weaver's runtime path still uses TOML
  by default.

## Progress

- [x] (2026-03-07 00:00 UTC) Audit the current workspace state and draft
  this ExecPlan.
- [x] (2026-03-07 10:40 UTC) Record the before-state. `make check-fmt`,
  `make lint`, and `make test` all passed on the pre-migration tree.
- [x] (2026-03-07 10:42 UTC) Update workspace and member manifests for
  Rust 1.88+ and `ortho_config` v0.8.0.
- [x] (2026-03-07 10:42 UTC) Regenerate `Cargo.lock` so `ortho_config` and
  `ortho_config_macros` both resolve to `0.8.0`.
- [x] (2026-03-07 10:44 UTC) Re-audit source for migration-note
  applicability and make the required code changes surfaced by the Rust 1.88
  lint set.
- [x] (2026-03-07 10:43 UTC) Refresh the configuration and migration
  documentation, including replacing the local ortho-config guide with the
  upstream v0.8.0 guide.
- [x] (2026-03-07 10:46 UTC) Run all relevant quality gates and capture the
  command logs.
- [x] (2026-03-07 10:46 UTC) Summarise the outcome and update this document's
  living sections.

## Surprises & Discoveries

- The live workspace is already on `ortho_config` v0.7.0, not v0.6.0.
  The next migration is therefore incremental, but the surrounding docs have
  not kept pace.
- `crates/weaver-config/src/lib.rs` is the primary derive site. It uses
  `#[derive(OrthoConfig)]` with a `#[ortho_config(discovery(...))]` attribute
  and does not alias the runtime crate.
- The CLI localiser in `crates/weaver-cli/src/localizer.rs` uses
  `FluentLocalizer`, `Localizer`, and `NoOpLocalizer` from `ortho_config`
  directly. This is the main non-config API surface exercised by the dependency
  today.
- The audit found no in-repo use of `SelectedSubcommandMerge`,
  `cli_default_as_absent`, `#[ortho_config(crate = "...")]`,
  `[package.metadata.ortho_config]`, `cargo orthohelp`, or `OrthoConfigDocs`.
- The repository does generate manual pages for `weaver` and `weaverd`
  during builds, but that mechanism is implemented with `clap_mangen` and a
  handwritten build script, not ortho-config documentation metadata.
- Raising the Rust floor from 1.85 to 1.88 surfaced four pre-existing Clippy
  findings under the stricter toolchain: three trivial accessors that now need
  `const fn` and one collapsible nested `if` in
  `crates/weaver-config/src/socket.rs`.
- Replacing the local ortho-config guide with the upstream v0.8.0 document
  required only two repository-local link fixes, both pointing README
  references at the tagged upstream GitHub URLs.

## Decision Log

- Decision: Treat this as a repo-wide migration of dependency version,
  toolchain floor, and documentation, not as a new feature. Rationale: the user
  asked to update usage to v0.8.0 using upstream migration notes, and the
  source audit shows the likely code delta is narrow while the documentation
  delta is broad. Date: 2026-03-07.

- Decision: Preserve versioned history by adding a new
  `docs/ortho-config-v0-8-0-migration-guide.md` rather than rewriting
  `docs/ortho-config-v0-6-0-migration-guide.md`. Rationale: the existing v0.6.0
  guide is still truthful historical documentation, and versioned file names
  should remain accurate. Date: 2026-03-07.

- Decision: Replace `docs/ortho-config-users-guide.md` with the upstream
  v0.8.0 guide from
  `https://raw.githubusercontent.com/leynos/ortho-config/refs/tags/v0.8.0/docs/users-guide.md`
   instead of hand-editing the in-repo copy. Rationale: the user explicitly
  requested the upstream guide as the replacement artefact, and the fetched
  file already captures the v0.8.0 material around layer composition,
  post-merge hooks, localisation, and dependency aliasing. Date: 2026-03-07.

- Decision: Only implement the `orthohelp` metadata flow if a concrete
  repository consumer is found during implementation. Rationale: the current
  tree has no `orthohelp` wiring, and adding speculative metadata would create
  maintenance burden without an observable payoff. Date: 2026-03-07.

- Decision: Normalise member manifests to
  `rust-version.workspace = true` while raising the workspace floor. Rationale:
  without that change, several crates would continue to omit an explicit Rust
  floor, undermining the requirement to ensure Rust 1.88 or newer. Date:
  2026-03-07.

- Decision: Accept the minimal Rust 1.88 follow-on fixes in
  `crates/sempai-core/src/diagnostic.rs`,
  `crates/weaver-syntax/src/pattern.rs`, and
  `crates/weaver-config/src/socket.rs` as part of this migration. Rationale:
  these were pre-existing code paths that only became lint failures after the
  toolchain uplift, and fixing them keeps the workspace green without changing
  behaviour. Date: 2026-03-07.

## Outcomes & Retrospective

The migration completed successfully. The workspace now resolves `ortho_config`
and `ortho_config_macros` to `0.8.0`, advertises Rust 1.88 at the workspace
root, and surfaces that floor consistently across member manifests by using
`rust-version.workspace = true` where it was previously missing.

The code impact was deliberately small. No aliasing, `SelectedSubcommandMerge`,
`cli_default_as_absent`, or `orthohelp` wiring was needed in Weaver. The only
code changes beyond manifests were the four toolchain-driven lint fixes noted
above. Behaviourally, the configuration contract for `weaver-config::Config`
and the CLI localiser remained unchanged.

Documentation now matches the upgraded state. The repository gained
`docs/ortho-config-v0-8-0-migration-guide.md`, the local
`docs/ortho-config-users-guide.md` was replaced with the upstream v0.8.0 guide
plus minimal local link fixes, and the active Weaver docs no longer describe
the project as an `ortho-config` v0.6.0 / Rust 1.85 workspace.

Validation completed successfully with captured logs:

- `make check-fmt`
- `make lint`
- `make test`
- `make fmt`
- `make markdownlint`
- `make nixie`

## Context and orientation

The upgrade is concentrated in a small set of files.

- `/home/user/project/Cargo.toml` holds the workspace dependency on
  `ortho_config = "0.7.0"` and the current Rust floor `rust-version = "1.85"`.
- `/home/user/project/crates/weaver-config/src/lib.rs` defines the shared
  `Config` struct and the derive-generated loading path used by both binaries.
- `/home/user/project/crates/weaver-cli/src/localizer.rs` exercises the
  Fluent localisation APIs added in v0.7.0 and must continue to compile after
  the v0.8.0 upgrade.
- `/home/user/project/crates/weaver-cli/src/config.rs` is a secondary audit
  point because it forwards CLI flags into `Config::load_from_iter`.
- `/home/user/project/README.md`,
  `/home/user/project/docs/users-guide.md`,
  `/home/user/project/docs/weaver-design.md`,
  `/home/user/project/docs/ortho-config-users-guide.md`,
  `/home/user/project/docs/contents.md`, and
  `/home/user/project/docs/roadmap.md` all describe the current configuration
  story and will become stale if the migration lands without documentation
  updates.

The current implementation does not appear to use the migration-note edge
cases. There is no alias for the `ortho_config` crate, no direct
`ortho_config_macros` dependency in workspace manifests, no
`SelectedSubcommandMerge`, and no `cli_default_as_absent` annotations in the
source audit. That means the code changes may be as small as a version bump,
Rust-floor uplift, and any compile fixes required by the new macro output. The
plan still includes explicit audits so those assumptions are verified rather
than guessed.

## Implementation plan

### Stage 1: Capture the baseline and prove the starting point

Start by recording the before-state so any later failure can be attributed
correctly.

1. Confirm the current dependency and toolchain state:

   ```sh
   rg -n 'ortho_config|rust-version' Cargo.toml crates/*/Cargo.toml
   rg -n 'name = "ortho_config"|name = "ortho_config_macros"|version = "0\\.(7|8)\\.0"' Cargo.lock
   migration_audit_pattern='SelectedSubcommandMerge|cli_default_as_absent|\\#\\[ortho_config\\(crate =|package.metadata.ortho_config|OrthoConfigDocs|orthohelp'
   rg -n "$migration_audit_pattern" crates docs
   ```

2. Run the current quality gates before any edits, using log capture:

   ```sh
   set -o pipefail; make check-fmt 2>&1 | tee /tmp/ortho-v0-8-check-fmt.before.log
   set -o pipefail; make lint 2>&1 | tee /tmp/ortho-v0-8-lint.before.log
   set -o pipefail; make test 2>&1 | tee /tmp/ortho-v0-8-test.before.log
   set -o pipefail; make markdownlint 2>&1 | tee /tmp/ortho-v0-8-markdownlint.before.log
   set -o pipefail; make nixie 2>&1 | tee /tmp/ortho-v0-8-nixie.before.log
   ```

3. If the known `make test` hang reproduces before any migration edits, do
   not continue blindly. Capture the tail of the test log in this plan and
   treat it as a pre-existing blocker that needs user direction.

Acceptance for Stage 1: the plan records whether the workspace was green before
the migration and whether any unrelated blockers were already present.

### Stage 2: Update manifests and the lockfile

Make the version and toolchain changes first, then let the compiler and tests
tell us whether any source edits are needed.

1. In `/home/user/project/Cargo.toml`, change the workspace Rust floor from
   `1.85` to `1.88` and change `ortho_config = "0.7.0"` to
   `ortho_config = "0.8.0"`.
2. Add `rust-version.workspace = true` to every member manifest that lacks
   it today: `crates/weaver-build-util/Cargo.toml`,
   `crates/weaver-cli/Cargo.toml`, `crates/weaver-e2e/Cargo.toml`,
   `crates/weaver-graph/Cargo.toml`, `crates/weaver-lsp-host/Cargo.toml`,
   `crates/weaver-plugin-rope/Cargo.toml`,
   `crates/weaver-plugin-rust-analyzer/Cargo.toml`,
   `crates/weaver-plugins/Cargo.toml`, `crates/weaver-sandbox/Cargo.toml`, and
   `crates/weaverd/Cargo.toml`.
3. Regenerate the lockfile entries. There is no Makefile target for this, so
   use Cargo directly:

   ```sh
   cargo update -p ortho_config --precise 0.8.0
   cargo update -p ortho_config_macros --precise 0.8.0
   ```

4. Verify that `Cargo.lock` now resolves both crates to `0.8.0`.

Acceptance for Stage 2: the workspace manifests declare the new Rust floor and
dependency version, and the lockfile resolves both runtime and macro crates to
`0.8.0`.

### Stage 3: Re-audit migration-note applicability and fix code

Only make source changes that the upgraded crate or the new Rust floor actually
require.

1. Re-run the source audit after Stage 2:

   ```sh
   migration_source_pattern='SelectedSubcommandMerge|cli_default_as_absent|\\#\\[ortho_config\\(crate =|default_value =|default_value_t|default_values_t'
   rg -n "$migration_source_pattern" crates
   rg -n 'use figment|use uncased|use xdg|figment::|uncased::|xdg::' crates
   ```

2. Compile/lint and repair only the affected areas:
   `crates/weaver-config/src/lib.rs`, `crates/weaver-cli/src/localizer.rs`,
   `crates/weaver-cli/src/config.rs`, and any directly failing call site
   surfaced by the compiler.
3. If an alias for `ortho_config` is introduced during the migration,
   annotate every relevant derive with `#[ortho_config(crate = "...")]`,
   including any future `SelectedSubcommandMerge` derives. The audit suggests
   this should not be necessary in Weaver today.
4. If any `cli_default_as_absent` usage is discovered, replace stringly
   `default_value = ...` overrides with typed clap defaults (`default_value_t`
   or `default_values_t`) before proceeding.
5. Do not add direct `figment`, `uncased`, or `xdg` dependencies to make
   derive-generated code happy. Prefer the `ortho_config::...` re-export paths
   where source imports are needed.

Acceptance for Stage 3: the workspace compiles cleanly against `ortho_config`
v0.8.0, and every migration note has been either applied or marked
non-applicable with concrete evidence.

### Stage 4: Refresh documentation and record applicability

The code change is small; the documentation clean-up is not optional.

1. Add `/home/user/project/docs/ortho-config-v0-8-0-migration-guide.md`.
   This guide should: describe the Weaver-specific move from v0.7.0 to v0.8.0,
   list the upstream migration notes supplied in the user request, call out
   which notes apply here, and record the audit results for the notes that do
   not apply.
2. Update `/home/user/project/docs/contents.md` to include the new migration
   guide without deleting the v0.6.0 guide.
3. Replace `/home/user/project/docs/ortho-config-users-guide.md` with the
   upstream v0.8.0 guide from:

   ```text
   https://raw.githubusercontent.com/leynos/ortho-config/refs/tags/v0.8.0/docs/users-guide.md
   ```

   Treat that fetched document as the body of the local file. Only make
   repository-local follow-up edits when they are necessary to keep links,
   formatting, or Markdown tooling valid in this repository.
4. Update `/home/user/project/docs/users-guide.md` and
   `/home/user/project/docs/weaver-design.md` so they describe the current
   Weaver configuration story accurately. These files should say that Weaver
   now uses `ortho-config` v0.8.0 and Rust 1.88+, and they should not imply
   that YAML is part of Weaver's runtime config path unless it truly is.
5. Update `/home/user/project/docs/roadmap.md` item 3.2.5 so it points at
   the v0.8.0 guide and no longer asks for already-completed v0.6.0
   documentation work.
6. Update `/home/user/project/README.md` so the build requirements say
   Rust 1.88+ instead of Rust 1.85+.
7. Explicitly record the `orthohelp` conclusion in the new migration guide:
   either it is not applicable because Weaver does not generate ortho-config
   documentation artefacts today, or it was added because a concrete artefact
   required it.

Acceptance for Stage 4: the active repository documentation matches the new
dependency/toolchain reality, the in-tree `docs/ortho-config-users-guide.md`
matches the upstream v0.8.0 guide except for minimal repository-local fixes,
and the migration guide explains every upstream note in Weaver terms.

### Stage 5: Validate end to end

After all edits are in place, run every relevant gate and capture the logs.

1. Format first:

   ```sh
   set -o pipefail; make fmt 2>&1 | tee /tmp/ortho-v0-8-fmt.after.log
   ```

2. Then run the Rust and docs gates:

   ```sh
   set -o pipefail; make check-fmt 2>&1 | tee /tmp/ortho-v0-8-check-fmt.after.log
   set -o pipefail; make lint 2>&1 | tee /tmp/ortho-v0-8-lint.after.log
   set -o pipefail; make test 2>&1 | tee /tmp/ortho-v0-8-test.after.log
   set -o pipefail; make markdownlint 2>&1 | tee /tmp/ortho-v0-8-markdownlint.after.log
   set -o pipefail; make nixie 2>&1 | tee /tmp/ortho-v0-8-nixie.after.log
   ```

3. Record the key evidence in this plan:
   `Cargo.lock` versions, successful command completion, and any notable
   warnings or follow-up constraints.

Acceptance for Stage 5: all applicable gates pass, or any remaining failure is
clearly shown to be pre-existing and user-approved for follow-up.

## Approval gate

This document is the draft phase only. Do not start implementing the migration
until the user explicitly approves this plan or requests changes to it.
