# Weaver repository layout

This document defines the current repository layout for Weaver and separates
implemented components from planned components.

System semantics and architecture are defined in
[weaver-design.md](weaver-design.md). Implementation sequencing is defined in
[roadmap.md](roadmap.md). Documentation conventions are defined in
[documentation-style-guide.md](documentation-style-guide.md) and `AGENTS.md`.

## Layout goals

The repository layout is designed to:

- keep runtime roles clear across command-line interface (CLI), daemon, and
  backend crates,
- separate security-critical concerns such as sandboxing and safety locks,
- support testability and deterministic quality gates, and
- make planned work discoverable without obscuring implemented ownership.

In this document, Language Server Protocol (LSP) and Continuous Integration
(CI) are expanded here and then used by acronym in later sections.

## Top-level structure

```plaintext
/
├── crates/
│   ├── weaver-build-util/
│   ├── weaver-cli/
│   ├── weaver-config/
│   ├── weaver-e2e/
│   ├── weaver-graph/
│   ├── weaver-lsp-host/
│   ├── weaver-plugin-rope/
│   ├── weaver-plugin-rust-analyzer/
│   ├── weaver-plugins/
│   ├── weaver-sandbox/
│   ├── weaver-syntax/
│   └── weaverd/
├── docs/
├── test_expect/
├── .github/workflows/
├── Cargo.toml
├── Cargo.lock
├── Makefile
├── rust-toolchain.toml
├── LICENSE
└── README.md
```

## Implemented components

### Core workspace crates

| Crate                         | Responsibility                                                                                       | Status      |
| ----------------------------- | ---------------------------------------------------------------------------------------------------- | ----------- |
| `weaver-cli`                  | CLI entrypoint, command parsing, daemon lifecycle commands, and JSON Lines (JSONL) request streaming | Implemented |
| `weaverd`                     | Daemon orchestration, command dispatch, and write-operation safety harness                           | Implemented |
| `weaver-config`               | Shared configuration schema and loading for client and daemon                                        | Implemented |
| `weaver-lsp-host`             | Language server lifecycle, capability detection, and semantic operations                             | Implemented |
| `weaver-syntax`               | Tree-sitter parsing and structural search or rewrite functionality                                   | Implemented |
| `weaver-graph`                | Relational graph layer with LSP-backed call hierarchy provider                                       | Implemented |
| `weaver-sandbox`              | Sandbox boundary for external tools and plugin execution                                             | Implemented |
| `weaver-plugins`              | Plugin protocol, lifecycle management, and broker integration                                        | Implemented |
| `weaver-plugin-rope`          | Python specialist plugin integration                                                                 | Implemented |
| `weaver-plugin-rust-analyzer` | Rust specialist plugin integration                                                                   | Implemented |
| `weaver-build-util`           | Shared build-time utilities used across crates                                                       | Implemented |
| `weaver-e2e`                  | End-to-end test support crate and integration scaffolding                                            | Implemented |

_Table 1: Implemented crate boundaries and responsibilities._

### Implemented shared directories

| Path                 | Purpose                                                                                               | Status      |
| -------------------- | ----------------------------------------------------------------------------------------------------- | ----------- |
| `docs/`              | Design docs, architectural decision records (ADRs), roadmap, migration guides, and reference material | Implemented |
| `test_expect/`       | Golden and expectation artefacts used by test suites                                                  | Implemented |
| `.github/workflows/` | CI workflows and automation policy                                                                    | Implemented |

_Table 2: Implemented shared directories and their roles._

## Planned components

Planned components are listed in `docs/roadmap.md` and are not yet fully
implemented in this repository snapshot.

| Planned component                                          | Intended location                                                        | Roadmap reference                                    |
| ---------------------------------------------------------- | ------------------------------------------------------------------------ | ---------------------------------------------------- |
| Rust `extricate-symbol` actuator flow                      | `crates/weaver-plugin-rust-analyzer/` and `crates/weaverd/`              | Proposed in Rust extricate actuator technical design |
| Rust extricate plugin overlay and RA orchestration modules | `crates/weaver-plugin-rust-analyzer/src/lsp/` and related plugin modules | Proposed in Rust extricate actuator technical design |
| Plugin capability metadata for extrication                 | `crates/weaver-plugins/src/manifest/mod.rs`                              | Proposed in Rust extricate actuator technical design |
| `srgn` specialist plugin                                   | `crates/weaver-plugin-srgn/` (expected new crate)                        | Phase 3, specialist actuator plugins                 |
| `jedi` specialist plugin                                   | `crates/weaver-plugin-jedi/` (expected new crate)                        | Phase 3, specialist sensor plugins                   |
| Static analysis provider for `weaver-graph`                | `crates/weaver-graph/` provider modules                                  | Phase 3, static analysis provider                    |
| `onboard-project` command flow                             | `crates/weaver-cli/` and `crates/weaverd/` command handlers              | Phase 4, advanced agent support                      |
| Interactive review mode for lock failures                  | `crates/weaver-cli/` plus daemon confirmation interfaces                 | Phase 4, human-in-the-loop mode                      |
| Dynamic analysis ingestion provider                        | `crates/weaver-graph/` provider modules                                  | Phase 4, dynamic analysis ingestion                  |

_Table 3: Planned components and their expected repository placement._

## Layout governance rules

- Add new implementation units to the crate that owns the domain behaviour,
  rather than by technical layer alone.
- Add new plugins as dedicated crates under `crates/` and register them through
  `weaver-plugins` and daemon runtime configuration.
- Keep `docs/repository-layout.md` and `docs/roadmap.md` synchronized when
  planned components move to implemented status.
- Keep `docs/contents.md` updated when new documentation artefacts are added.

## Relationship to implementation planning

`docs/roadmap.md` defines delivery order and scope. This file defines ownership
and expected locations for implementation artefacts. Any roadmap item that adds
or renames a crate should update this layout document in the same change.
