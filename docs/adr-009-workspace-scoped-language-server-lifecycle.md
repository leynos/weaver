# Architectural decision record (ADR) 009: Workspace-scoped language-server lifecycle

## Status

Proposed.

## Date

2026-08-16.

## Context and problem statement

`LspHost` currently stores one session per language. Production process
adapters use bare server commands, normally inherit the daemon's runtime
directory, and initialize without `rootUri` or `workspaceFolders`. That model
reuses processes but does not establish which repository, toolchain,
configuration, or environment owns a session.

Language Server Protocol (LSP) servers retain workspace configuration,
open-document contents, document versions, diagnostics, and build-system state.
Rustup also resolves toolchains from directory-sensitive overrides. A process
being alive is therefore insufficient evidence that it is correct for the next
request.

[RFC 0002](rfcs/0002-multi-workspace-daemon.md) contains the full proposal,
prior art, and migration analysis.

## Decision drivers

- Preserve server startup amortization within one workspace.
- Prevent open-document and diagnostic state from crossing workspaces.
- Set process and protocol workspace roots explicitly.
- Honour repository-specific Rust toolchains and adapter configuration.
- Restart servers when their execution identity changes.
- Bound warm processes, unhealthy restart loops, and idle retention.
- Keep server-specific root discovery behind adapter contracts.

## Options considered

### Option A: Workspace-owned pool keyed by execution identity

Each `WorkspaceState` owns a pool. Server identity includes language, command,
arguments, resolved executable or toolchain, configuration, and relevant
environment. The process starts in the workspace or adapter-selected project
root and initializes with matching LSP root fields.

### Option B: One daemon-global server per language

Reuse one Rust, Python, or TypeScript server for every repository. This
minimizes process count but combines unrelated workspace, toolchain,
configuration, and open-document state.

### Option C: One server process per request

Start and stop a server for every operation. This provides strong isolation but
discards persistent indexes and makes daemon amortization ineffective.

### Option D: Key only by workspace and language

Keep one server for each workspace-language pair until it exits. This misses
toolchain, executable, configuration, and environment changes while the process
remains alive.

| Topic                 | Execution identity | Global per language | Per request  | Workspace and language |
| --------------------- | ------------------ | ------------------- | ------------ | ---------------------- |
| Workspace isolation   | Yes                | No                  | Yes          | Yes                    |
| Toolchain correctness | Explicit           | No                  | Ambient      | Incomplete             |
| Startup amortization  | Yes                | Yes                 | No           | Yes                    |
| Change invalidation   | Structural         | Unsafe              | Automatic    | Incomplete             |
| Process budget needed | Yes                | Small               | Spawn budget | Yes                    |

_Table 1: Language-server lifecycle alternatives._

## Decision statement

In the context of semantic operations across repositories with different
language-server configurations and Rust toolchains, facing session-global
document state and directory-sensitive executable selection, Weaver decides for
workspace-owned language-server pools keyed by complete execution identity with
explicit process and LSP roots, and against one cross-repository server per
language, ambient proxy selection, one process per request, or
workspace-and-language-only keys, to achieve correct process reuse,
deterministic invalidation, and isolated semantic state, accepting additional
warm processes, identity resolution cost, restart coordination, and eviction
policy.

## Decision outcome and proposed direction

Adopt option A.

The workspace key is implicit in the pool owner. A language-server key includes
the language, configured command and arguments, resolved executable or
toolchain, server configuration fingerprint, and adapter-declared environment
fingerprint.

Every child starts with the selected workspace or project root as its current
directory. LSP initialization supplies `rootUri` and a matching
`workspaceFolders` entry when supported. Open-document state and document
versions remain session-local.

For rustup-managed Rust servers, Weaver resolves the active toolchain with the
workspace as `current_dir`, records that identity, and launches
`rustup run <toolchain> rust-analyzer`. A missing component is an explicit
`unavailable` result; Weaver does not fall back to a different toolchain.

Relevant input changes or bounded freshness expiry trigger identity
revalidation. A changed identity retires the old server before new leases use
the replacement.

## Consequences

- Unrelated repositories never share an LSP process merely because they use
  the same language or toolchain.
- Server lifecycle gains health checks, bounded restart, graceful retirement,
  identity diagnostics, process budgets, and idle eviction.
- Adapters must declare project-root selection, configuration inputs,
  environment inputs, and execution-identity resolution.

## Known risks and limitations

- Several active repositories may require several large language-server
  processes.
- Rustup directory overrides live outside the repository and require periodic
  or event-driven revalidation beyond local file watching.
- Some servers interpret `rootUri`, workspace folders, and nested project roots
  differently, so adapters need conformance tests.
- This decision does not require sharing indexes between separate processes.

## Outstanding decisions

- Define adapter-specific environment allowlists for identity fingerprints.
- Set process budgets, idle timeouts, restart backoff, and freshness defaults.
- Decide whether any supported server can safely host related nested project
  roots within one canonical workspace.
