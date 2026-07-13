# Weaver

A human-friendly, agent-native semantic command-line tool.

Weaver is your AI agent's best friend when it comes to understanding and
modifying code. We believe the shell should speak the language of semantics—not
just bytes and lines—so we've built a tool that brings deep code intelligence
to your terminal.

## What is Weaver?

Weaver is a Rust-based CLI tool that provides semantic operations on codebases
through a simple, composable interface. It follows the UNIX philosophy: small,
focused tools that communicate clearly and play nicely with existing shell
utilities like `jq`, `xargs`, and `find`.

Under the bonnet, Weaver runs a daemon (`weaverd`) that orchestrates language
servers, syntax analysers, and specialized plugins—all sandboxed for
safety—while the lightweight CLI (`weaver`) lets you issue commands and stream
results. Whether you're an AI agent planning a refactor or a human debugging a
tricky rename, Weaver gives you the semantic primitives you need.

## Status

Weaver is in active early development before the first v0.1.0 release. The
repository already contains the CLI/daemon architecture, shared configuration,
Language Server Protocol (LSP) hosting, Tree-sitter syntax support, graph and
card foundations, plugin infrastructure, sandboxing, and write-operation safety
harness work. The public command surface is now being reset around
[ADR 007](docs/adr-007-agent-native-command-surface.md), which makes the 0.1.0
target human-friendly and agent-native without preserving compatibility with
the prototype `observe` / `act` / `verify` grammar.

Check the [roadmap](docs/roadmap.md) for the current build sequence and the
[user's guide](docs/users-guide.md) for the split between current prototype
commands and the 0.1.0 target contract.

## Key features

- **Semantic operations as shell verbs** — Current prototype commands expose
  code intelligence today; the 0.1.0 target moves to resource-first commands
  such as `definitions get`, `references list`, and `symbols rename`.
- **Agent-native output contracts** — The target surface keeps readable human
  defaults while making `--json` the stable machine-readable path.
- **Multi-layer fusion** — Combines Language Server Protocol (LSP), Tree-sitter
  parsing, and call graph analysis for comprehensive code understanding.
- **Zero-trust sandboxing** — External tools run in isolated environments using
  Linux namespaces and seccomp-bpf filters.
- **Graceful degradation** — When a capability isn't available, Weaver tells you
  what's missing and suggests alternatives.

## Components

Weaver is organized as a Cargo workspace with dedicated crates for the CLI,
daemon, configuration, LSP hosting, syntax parsing, graph support, cards,
plugins, sandboxing, Sempai query planning, build utilities, and end-to-end
tests. See [the repository layout](docs/repository-layout.md) for the current
crate map and planned component ownership.

## Getting started

Here's the quickest path to your first Weaver command:

```sh
# Start the daemon
weaver daemon start

# Query a symbol definition
weaver definitions get --uri file:///path/to/main.rs --position 42:17

# Check daemon status
weaver daemon status

# Stop the daemon when you're done
weaver daemon stop
```

For full installation, configuration, and usage instructions, please see the
[User's Guide](docs/users-guide.md). It covers daemon lifecycle management,
configuration layering, and the complete command reference.

## Building from source

Weaver requires the pinned **Nightly Rust toolchain `nightly-2026-03-26`** for
local builds. The workspace `.cargo/config.toml` uses options that require the
Nightly toolchain, so stable Rust is not sufficient for local Cargo builds in
this checkout. To build:

```sh
cargo +nightly-2026-03-26 build --release
```

To run the test suite:

```sh
cargo +nightly-2026-03-26 test --workspace
```

### Toolchain prerequisites

The workspace `.cargo/config.toml` enables Nightly-only build settings for the
Cranelift codegen backend in development builds. Install the pinned toolchain
and component with:

```sh
rustup toolchain install nightly-2026-03-26
rustup component add rustc-codegen-cranelift --toolchain nightly-2026-03-26
```

Set a local override so Cargo uses that pinned Nightly automatically in this
checkout:

```sh
rustup override set nightly-2026-03-26
```

If any of these prerequisites are missing, the failure mode is often opaque:
Cargo may report unstable `-Z` option errors or missing
`rustc-codegen-cranelift`. When that happens, verify the pinned Nightly
toolchain and the Cranelift component first.

If local builds fail, verify the pinned Nightly override first, then confirm
the Cranelift component is installed before investigating the workspace itself.

## Documentation

- [User's Guide](docs/users-guide.md) — Configuration, daemon lifecycle, and
  command reference
- [Design Document](docs/weaver-design.md) — Architecture, philosophy, and
  technical deep-dive
- [Roadmap](docs/roadmap.md) — Development phases and upcoming features

## Licence

Weaver is released under the [ISC License](LICENSE).
