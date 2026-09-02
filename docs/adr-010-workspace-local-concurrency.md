# Architectural decision record (ADR) 010: Workspace-local concurrency

## Status

Proposed.

## Date

2026-08-16.

## Context and problem statement

The daemon accepts connections on separate threads, but every routed request
holds one mutex around the complete backend set. Expensive semantic work,
read-only queries, and mutations therefore serialize across all languages and
repositories. When all handler slots are occupied, the listener drops newly
accepted connections.

Multi-workspace tenancy needs a concurrency model whose isolation unit matches
the state being protected. Mutation verification adds a stricter constraint: a
shared LSP session's `didChange` state is visible to every request using that
session, so read-only work must not observe staged mutation content.

[RFC 0002](rfcs/0002-multi-workspace-daemon.md) contains the full proposal and
rollout plan.

## Decision drivers

- Allow unrelated workspaces and languages to make independent progress.
- Keep registry critical sections short and free of external input/output.
- Serialize conflicting commits within one workspace.
- Prevent readers from observing staged LSP document state.
- Bound connections, in-flight operations, server queues, and wait time.
- Return structured, retryable overload diagnostics where possible.
- Preserve conservative correctness before optimizing same-workspace reads.

## Options considered

### Option A: Workspace-local coordination with exclusive mutation leases

Use a short registry lock, workspace-local read coordination, server-local
bounded queues, and one exclusive mutation lease per workspace. A mutation
holds the lease through staging, formatting, verification, commit or rollback,
and LSP state restoration.

### Option B: Keep one global backend mutex

Continue serializing every operation. This is easy to reason about but blocks
unrelated work and converts transport concurrency into threads waiting for one
lock.

### Option C: Run all requests without coordination

Rely only on content digests and server request identifiers. This allows races
in caches, server document views, configuration restart, and commit ordering.

### Option D: Require transaction-scoped shadow servers immediately

Let reads use the live server while every mutation starts an isolated server
against a shadow workspace. This offers more concurrency but adds another
expensive lifecycle path before the workspace boundary is proven.

| Topic                                | Workspace-local leases | Global mutex | No coordination | Shadow servers |
| ------------------------------------ | ---------------------- | ------------ | --------------- | -------------- |
| Cross-workspace concurrency          | Yes                    | No           | Yes             | Yes            |
| Staged-state isolation               | Yes                    | Yes          | No              | Yes            |
| First-slice complexity               | Moderate               | Low          | Low             | High           |
| Same-workspace reads during mutation | Wait                   | Wait         | Unsafe          | Concurrent     |
| Process cost                         | Normal                 | Normal       | Normal          | Higher         |

_Table 1: Concurrency and mutation-isolation alternatives._

## Decision statement

In the context of several agents issuing semantic and mutation requests through
one local daemon, facing a daemon-wide backend mutex, finite connection
handlers, and session-global staged LSP document state, Weaver decides for
short registry critical sections, bounded workspace- and server-local
coordination, and one exclusive mutation lease per workspace, and against
daemon-wide serialization, uncoordinated execution, or mandatory
transaction-scoped servers in the first slice, to achieve cross-workspace
progress, deterministic mutation ordering, and observable overload without
exposing staged content, accepting that read-only work in the same workspace
waits during mutation and that queue, cancellation, and fairness policies
become explicit.

## Decision outcome and proposed direction

Adopt option A.

The registry lock covers only lookup, single-flight creation, retirement, and
removal. An operation then coordinates through its selected `WorkspaceState`
and language-server lease.

Read-only operations may proceed concurrently when they observe the same
committed revision and their backend supports concurrent in-flight requests. A
mutation obtains an exclusive workspace lease before capturing its baseline and
holds it until commit or rollback plus shared LSP state restoration.

Admission is bounded at the daemon, workspace, and server scopes. Saturation
returns a stable retryable reason code and bounded guidance whenever the
transport can respond. Queue wait and execution time remain distinct
observability signals.

Transaction-scoped shadow servers remain a future optimization. They require a
follow-up decision if evidence shows that same-workspace read latency during
mutations is unacceptable.

## Consequences

- Python, Rust, and unrelated repositories no longer block each other through
  one backend mutex.
- Same-workspace readers may wait for a mutation, preserving a coherent live
  LSP view in the first implementation.
- Connection, workspace, server, queue, timeout, cancellation, and overload
  policies become bounded configuration and protocol concerns.

## Known risks and limitations

- Long mutations can increase same-workspace read latency.
- Multiple bounded queues can produce unfairness unless acquisition order and
  cancellation are designed carefully.
- A client disconnect must cancel queued work without abandoning a live commit
  or leaving a server lease stuck.
- Structured overload responses are not possible after every low-level
  transport failure; tracing must retain evidence for those cases.

## Outstanding decisions

- Define fair queue ordering, cancellation semantics, and retry guidance.
- Set default limits for connections, workspace work, and server requests.
- Define the evidence threshold for adopting transaction-scoped shadow
  semantic verification.
