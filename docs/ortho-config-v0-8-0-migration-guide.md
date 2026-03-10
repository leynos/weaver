# Migration guide: Weaver adoption of `ortho-config` v0.8.0

## Introduction

This guide records how the Weaver workspace adopts `ortho_config` v0.8.0 from
the previous v0.7.0 baseline. It combines the upstream migration notes with the
repository-specific audit performed during the upgrade so future maintainers
can see both what changed and what was intentionally left alone.

The authoritative upstream user guide for v0.8.0 now replaces the local copy at
`docs/ortho-config-users-guide.md`. Weaver-specific behaviour remains
documented in `docs/users-guide.md` and `docs/weaver-design.md`.

## Starting point

Before this migration, the workspace state was:

- `[workspace.dependencies]` pinned `ortho_config = "0.7.0"` in
  `Cargo.toml`.
- `[workspace.package]` advertised `rust-version = "1.85"`.
- Several member crates inherited `edition` and `version` from the workspace
  but did not inherit `rust-version.workspace = true`, so the Rust floor was
  not surfaced consistently across the workspace.
- `crates/weaver-config/src/lib.rs` was the primary derive site, using
  `#[derive(OrthoConfig)]` with a declarative `#[ortho_config(discovery(...))]`
  attribute.
- `crates/weaver-cli/src/localizer.rs` consumed the localisation APIs added in
  v0.7.0 (`FluentLocalizer`, `Localizer`, and `NoOpLocalizer`).
- The repository still described the configuration stack as
  `ortho-config` v0.6.0 in several docs.

## Required migration steps

### 1. Update dependency versions and toolchain floor

Change every `ortho_config` and `ortho_config_macros` dependency to `0.8.0` and
ensure the workspace requires Rust 1.88 or newer.

For Weaver, that means:

- updating `[workspace.package].rust-version` to `1.88`,
- updating `[workspace.dependencies].ortho_config` to `0.8.0`,
- regenerating `Cargo.lock` so both `ortho_config` and
  `ortho_config_macros` resolve to `0.8.0`, and
- adding `rust-version.workspace = true` to the member manifests that
  previously omitted it.

### 2. Crate aliasing is not used in Weaver

Upstream v0.8.0 requires `#[ortho_config(crate = "...")]` whenever the runtime
crate is aliased in `Cargo.toml`, and the same rule applies to
`SelectedSubcommandMerge`.

The Weaver audit found no aliased `ortho_config` dependency and no in-repo use
of `SelectedSubcommandMerge`, so this note is currently non-applicable here. If
either pattern is introduced later, the derive attributes must be updated at
the same time.

### 3. `cli_default_as_absent` is not used in Weaver

Upstream v0.8.0 now rejects inference from stringly `default_value` metadata
when `cli_default_as_absent` is active; callers must use typed clap defaults
such as `default_value_t` or `default_values_t`.

The Weaver audit found no `cli_default_as_absent` attributes in the source, so
no code changes were required for this note. Future uses of
`cli_default_as_absent` must follow the typed-default rule from the start.

### 4. YAML semantics changed, even though Weaver runtime config is TOML-first

Upstream YAML parsing now uses `serde-saphyr` with YAML 1.2 behaviour. Legacy
literals such as `yes`, `on`, and `off` must be quoted when they should remain
strings, and duplicate mapping keys are rejected.

Weaver's runtime configuration still uses TOML discovery, so no production code
path changed here. The documentation set still needs this note because the
local ortho-config user guide discusses optional YAML support in generic
examples.

### 5. Prefer `ortho_config` re-exports for derive-adjacent imports

Upstream v0.8.0 expects derive-generated code to use dependency re-exports from
`ortho_config::figment`, `ortho_config::uncased`, and `ortho_config::xdg`
unless the application imports those crates directly for its own purposes.

Weaver did not have direct source imports from `figment`, `uncased`, or `xdg`
that needed correction. The main impact was documentation: examples in the
in-repo ortho-config guide should reflect the re-exported import paths that the
upstream v0.8.0 guide now uses.

### 6. `cargo orthohelp` is not currently part of the Weaver build

Upstream v0.8.0 adds metadata for generated configuration-documentation
artefacts via `[package.metadata.ortho_config]` and `cargo orthohelp`.

The Weaver audit found:

- no `OrthoConfigDocs` metadata in the source tree,
- no `[package.metadata.ortho_config]` in any manifest,
- no `cargo orthohelp` usage in scripts or docs, and
- build-time documentation generation that is limited to CLI man pages via
  `clap_mangen`.

Because no concrete Weaver artefact depends on `orthohelp` today, this note is
non-applicable for the current migration. Revisit it only if the workspace
starts emitting ortho-config-specific documentation artefacts.

## Documentation policy for this migration

Weaver keeps two kinds of ortho-config documentation:

- a versioned migration record inside this repository, and
- the upstream ortho-config user's guide, copied locally for reference.

For v0.8.0, the local policy is:

- keep `docs/ortho-config-v0-6-0-migration-guide.md` as a truthful historical
  document,
- add this new v0.8.0 migration guide rather than rewriting the v0.6.0 file,
  and
- replace `docs/ortho-config-users-guide.md` with the upstream
  `docs/users-guide.md` from the `v0.8.0` tag, applying only the minimal local
  fixes needed for links and Markdown tooling inside Weaver.

## Weaver adoption checklist

After the migration is complete, verify the following:

- `Cargo.toml` advertises `ortho_config = "0.8.0"` and Rust `1.88`.
- Every member crate surfaces the shared Rust floor through
  `rust-version.workspace = true` or an explicit `rust-version`.
- `Cargo.lock` resolves `ortho_config` and `ortho_config_macros` to `0.8.0`.
- `docs/ortho-config-users-guide.md` matches the upstream v0.8.0 guide except
  for minimal repository-local fixes.
- `docs/users-guide.md`, `docs/weaver-design.md`, and `README.md` no longer
  describe Weaver as an `ortho-config` v0.6.0 / Rust 1.85 workspace.
- `make check-fmt`, `make lint`, `make test`, `make markdownlint`,
  `make fmt`, and `make nixie` pass.
