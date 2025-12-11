# Weaver

A semantics-aware command-line tool for AI coding agents.

Weaver is your AI agent's best friend when it comes to understanding and
modifying code. We believe the shell should speak the language of semantics—not
just bytes and lines—so we've built a tool that brings deep code intelligence
to your terminal.

## What is Weaver?

Weaver is a Rust-based CLI tool that provides semantic operations on codebases
through a simple, composable interface. It follows the UNIX philosophy: small,
focused tools that communicate via JSON Lines (JSONL) and play nicely with your
existing shell utilities like `jq`, `xargs`, and `find`.

Under the hood, Weaver runs a daemon (`weaverd`) that orchestrates language
servers, syntax analysers, and specialised plugins—all sandboxed for
safety—while the lightweight CLI (`weaver`) lets you issue commands and stream
results. Whether you're an AI agent planning a refactor or a human debugging a
tricky rename, Weaver gives you the semantic primitives you need.

## Status

Weaver is in active early development (v0.1.0). We've completed the core
foundation—the CLI/daemon architecture, LSP integration for Rust, Python, and
TypeScript, and the security sandbox—but there's more to come. Check out our
[roadmap](docs/roadmap.md) to see what we're working on next.

> **Note:** The "Double-Lock" safety harness for write operations is still under
> development. We recommend caution when using `act` commands until this is
> complete.

## Key features

- **Semantic operations as shell verbs** — Commands like
  `observe get-definition`
  and `act rename-symbol` bring IDE-level intelligence to your terminal.
- **JSONL-native protocol** — Every request and response is a JSON object,
  making integration with other tools trivial.
- **Multi-layer fusion** — Combines Language Server Protocol (LSP), Tree-sitter
  parsing, and call graph analysis for comprehensive code understanding.
- **Zero-trust sandboxing** — External tools run in isolated environments using
  Linux namespaces and seccomp-bpf filters.
- **Graceful degradation** — When a capability isn't available, Weaver tells you
  what's missing and suggests alternatives.

## Components

Weaver is organised as a Cargo workspace with five crates:

| Crate             | Description                                                      |
| ----------------- | ---------------------------------------------------------------- |
| `weaver-cli`      | Thin CLI client that serialises commands into JSONL              |
| `weaverd`         | Daemon broker that orchestrates backends and verifies operations |
| `weaver-config`   | Shared configuration management via `ortho-config`               |
| `weaver-lsp-host` | Language Server Protocol host with capability detection          |
| `weaver-sandbox`  | Security sandbox wrapper around `birdcage`                       |

## Getting started

Here's the quickest path to your first Weaver command:

```sh
# Start the daemon
weaver daemon start

# Query a symbol definition
weaver observe get-definition --uri file:///path/to/main.rs --position 42:17

# Check daemon status
weaver daemon status

# Stop the daemon when you're done
weaver daemon stop
```

For full installation, configuration, and usage instructions, please see the
[User's Guide](docs/users-guide.md). It covers daemon lifecycle management,
configuration layering, and the complete command reference.

## Building from source

Weaver requires **Rust 1.85+** (edition 2024). To build:

```sh
cargo build --release
```

To run the test suite:

```sh
cargo test --workspace
```

## Documentation

- [User's Guide](docs/users-guide.md) — Configuration, daemon lifecycle, and
  command reference
- [Design Document](docs/weaver-design.md) — Architecture, philosophy, and
  technical deep-dive
- [Roadmap](docs/roadmap.md) — Development phases and upcoming features

## Licence

Weaver is released under the [MIT Licence](LICENSE).
