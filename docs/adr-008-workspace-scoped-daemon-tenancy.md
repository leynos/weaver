# Architectural decision record (ADR) 008: Workspace-scoped daemon tenancy

## Status

Proposed.

## Date

2026-08-16.

## Context and problem statement

Weaver's default daemon endpoint is per user, but the daemon captures one
startup directory as its workspace root and retains that root for its lifetime.
Commands issued from another repository therefore reach the same socket while
the router and mutation handlers still operate against the first repository.

The daemon needs a tenancy boundary that supports several repositories without
turning a local process into a multi-user service. That boundary must precede
routing, cache access, language-server selection, and filesystem mutation.

[RFC 0002](rfcs/0002-multi-workspace-daemon.md) contains the full proposal and
migration analysis.

## Decision drivers

- Preserve one convenient per-user socket and daemon lifecycle.
- Make repository selection explicit and deterministic for every request.
- Prevent mutable caches or backend state from crossing repository roots.
- Keep workspace paths as untrusted request input until daemon validation.
- Use capability-oriented filesystem access after canonical resolution.
- Avoid holding a registry lock during routing or external input/output.
- Retain separate sockets as an operational escape hatch.

## Options considered

### Option A: Canonical workspace registry in one per-user daemon

Add a workspace locator to the request contract. Resolve it in the daemon to a
canonical `WorkspaceKey`, then obtain an `Arc<WorkspaceState>` from a registry.
Each workspace state owns all mutable state whose correctness depends on that
root.

### Option B: One daemon and socket per repository

Keep the fixed-root daemon and require clients to select a repository-specific
socket. This avoids an internal registry but moves lifecycle coordination into
every client and duplicates daemon-wide resource management.

### Option C: Continue using daemon startup state

Keep one captured root and document that the default socket belongs to the
first repository. This is simple but makes commands from other repositories
silently target the wrong workspace unless every client coordinates sockets.

### Option D: Trust a client-supplied canonical path

Let the CLI canonicalize the path and use that string directly as the registry
key. This avoids daemon work but treats untrusted input as authority and allows
client and daemon filesystem views to disagree.

| Topic                       | Canonical registry | Per-repository daemons | Startup root    | Client canonicalization |
| --------------------------- | ------------------ | ---------------------- | --------------- | ----------------------- |
| Default socket              | One per user       | One per repository     | One per user    | One per user            |
| Workspace isolation         | Explicit           | Process boundary       | Absent          | Client-dependent        |
| Central resource budget     | Yes                | No                     | Yes             | Yes                     |
| Client coordination         | Minimal            | High                   | Unsafe          | Medium                  |
| Daemon filesystem authority | Yes                | Yes                    | Only at startup | No                      |

_Table 1: Workspace tenancy alternatives._

## Decision statement

In the context of one local Weaver daemon serving commands from several
repositories, facing a per-user socket coupled to one bootstrap working
directory and one lifetime workspace root, Weaver decides for a daemon-resolved
canonical `WorkspaceKey` and workspace-owned `WorkspaceState` selected before
routing, and against repository-specific daemons as the primary model, ambient
process working directory, or client-authoritative canonical paths, to achieve
deterministic repository isolation with one convenient local service and one
bounded resource budget, accepting a versioned request-schema change plus
registry creation, retirement, and eviction complexity.

## Decision outcome and proposed direction

Adopt option A.

Every routable request carries a workspace locator. The daemon validates and
canonicalizes that locator, opens the corresponding capability directory, and
uses the canonical root as a `WorkspaceKey`. The key selects exactly one
`WorkspaceState` before domain routing begins.

`WorkspaceState` owns workspace-sensitive caches, language-server lifecycle,
revision state, and mutation coordination. Immutable registries whose values do
not depend on a workspace may remain daemon-global.

The registry protects only lookup, single-flight creation, retirement marking,
and removal. Routing holds an `Arc<WorkspaceState>` after releasing the
registry lock.

## Consequences

- The CLI-daemon request schema gains required workspace identity and explicit
  version-mismatch behaviour.
- The per-user socket remains the default, while configured separate sockets
  remain available for troubleshooting or stronger process isolation.
- Workspace discovery, canonicalization, nested repositories, capability
  access, lifecycle, and eviction become explicit daemon responsibilities.

## Known risks and limitations

- Canonical paths do not express every bind-mount or platform-specific
  filesystem identity edge case.
- Nested repositories and worktrees need a documented tenancy-root selection
  policy.
- A registry creates retained state that must be bounded and evicted safely.
- This decision does not make the daemon suitable for several operating-system
  users or remote clients.

## Outstanding decisions

- Define the ordinary CLI workspace-locator source and any explicit override.
- Define portable handling for aliases that canonical paths cannot distinguish.
- Set default idle and retained-workspace budgets through configuration work.
