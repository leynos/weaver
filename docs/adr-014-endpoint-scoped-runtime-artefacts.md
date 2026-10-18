# Architectural decision record (ADR) 014: Endpoint-scoped daemon runtime artefacts

## Status

Accepted.

## Date

2026-10-18.

## Context and problem statement

Daemon lock, PID, and health files used fixed filenames within a runtime
directory. Two configured endpoints could therefore contend for or read the
same process state, even though the endpoints were distinct.

Older versions wrote those shared files without recording the configured
endpoint. After filenames become endpoint-scoped, a newer CLI cannot safely
associate an older PID or health snapshot with a particular non-default
endpoint. A reachable socket does not prove that the process named by a shared
PID file owns that endpoint.

## Decision drivers

- Keep default endpoint filenames stable for existing installations.
- Give each non-default endpoint a deterministic, filesystem-safe set of
  runtime artefacts.
- Preserve exclusive daemon startup across supported operating systems.
- Never signal a process unless the configured endpoint's PID file identifies
  it.
- Make the upgrade requirement for already-running older daemons explicit.

## Options considered

### Option A: Continue using shared filenames

Keep one lock, PID, and health file per runtime directory. This preserves
existing lifecycle behaviour but prevents independent daemons from safely
sharing a runtime directory.

### Option B: Infer old endpoint ownership during upgrade

Read a legacy PID or health snapshot and use socket reachability to decide
whether to signal that process. The legacy files contain no endpoint identity,
so this could stop a different daemon sharing the runtime directory.

### Option C: Scope names to the endpoint and require older daemons to stop

Retain the old names for the default endpoint and derive all three names for
other endpoints from the complete configured endpoint. Do not adopt legacy
files for a non-default endpoint; require a running daemon from an older
version to stop before upgrading.

| Topic               | Shared filenames | Infer ownership | Endpoint-scoped names |
| ------------------- | ---------------- | --------------- | --------------------- |
| Endpoint isolation  | None             | Unverified      | Deterministic         |
| Existing default    | Preserved        | Preserved       | Preserved             |
| Old custom daemon   | Controllable     | Risky           | Stop before upgrade   |
| Safe process signal | By convention    | Not guaranteed  | Scoped PID file       |

_Table 1: Runtime artefact options._

## Decision statement

In the context of multiple configured endpoints sharing a runtime directory,
facing collisions among fixed lock, PID, and health filenames and legacy files
that do not identify their endpoint, Weaver decides to preserve the fixed names
for the default endpoint and derive non-default names from the complete
endpoint, while requiring daemons from older versions with non-default
endpoints to stop before upgrade, to isolate process state without guessing
which process a shared PID belongs to, accepting a one-time manual stop during
upgrade.

## Decision outcome and proposed direction

The default endpoint continues to use `weaverd.lock`, `weaverd.pid`, and
`weaverd.health`. For other endpoints, Weaver hashes the endpoint kind and its
complete address with SHA-256 and uses the resulting lowercase hexadecimal
identifier in each filename.

The daemon opens the lock file through its capability directory and holds an
exclusive file lock on that handle for its lifetime. Unix uses `flock`, except
Solaris, where `fcntl` locking is paired with a process-local guard because
those locks do not exclude threads in the same process. After daemonisation,
the child reacquires the process-associated lock before publishing its PID; the
parent retains the lock until it exits, so this handoff cannot open a gap.
Windows uses `LockFileEx` on the same capability-opened handle. The lock file
remains after shutdown; PID and health files are removed. A newer CLI uses only
the configured endpoint's runtime filenames. If its PID file is missing while
the socket is reachable, it reports the condition and refuses to signal an
unverified process.

## Consequences

- Distinct endpoints can use separate lock, PID, and health files in a shared
  runtime directory.
- Operators upgrading from a version with shared filenames must stop any
  running daemon configured with a non-default endpoint before upgrading.
- Lifecycle commands cannot safely recover ownership from legacy files because
  those files contain no endpoint identity.
- Persistent lock files are expected and can be reused after normal shutdown
  or an abandoned startup.

## Known risks and limitations

- The one-time stop requirement interrupts an older non-default daemon during
  upgrade.
- The naming scheme distinguishes endpoint strings exactly; aliases for the
  same network listener remain distinct configuration values.

## Outstanding decisions

- None.
