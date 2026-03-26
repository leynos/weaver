# Weaver developer's guide

This guide documents internal development concerns: toolchain baselines,
configuration framework internals, and implementation details that contributors
need but operators do not. For user-facing behaviour see the
[user's guide](users-guide.md).

## Workspace baseline

The workspace targets `ortho_config` v0.8.0 and Rust 1.88.

## Configuration framework internals

### `ortho_config` v0.8.0 integration

`weaver_config::Config` declares its discovery policy inline through the
`#[ortho_config(discovery(...))]` attribute. The app name, dotfile, project
file, and `--config-path` flag are all defined next to the struct, so every
consumer shares the same generated loader without bespoke builders.

The `ortho_config` v0.8.0 loader preserves the stricter discovery and parsing
model adopted in earlier releases: if any discovered configuration file fails
to parse, `ConfigDiscovery::load_first` returns an aggregated `OrthoError`.
Both the CLI and daemon bubble that error to the user instead of quietly
falling back to defaults, making misconfigurations immediately visible.

Configuration is layered with `ortho_config`, producing the precedence order
`defaults < files < environment < CLI`. File discovery honours `--config-path`
alongside the standard XDG locations, ensuring the CLI and daemon resolve
identical results regardless of which component loads the settings.

### Dependency-graph resolution

The loader uses a dependency-graph model for layered configuration sources.
Sources are merged in precedence order: built-in defaults are overridden by
discovered files, which are overridden by environment variables, which are in
turn overridden by CLI flags. When multiple configuration files are discovered,
they are merged in the order `--config-path` first, then XDG locations in
standard search order. Later sources override earlier ones field-by-field.

### TOML parsing semantics

All configuration inputs are parsed per TOML v1 rules. Anchors and tags are not
applicable (those are YAML concepts); TOML scalars are strongly typed and
preserve their declared type without implicit coercion. Boolean values must be
`true` or `false` (the string `"yes"` is rejected as an invalid boolean, as
shown in the user-facing error example).
