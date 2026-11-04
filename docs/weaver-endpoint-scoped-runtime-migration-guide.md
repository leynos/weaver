# Weaver endpoint-scoped runtime artefact migration guide

## Who should read this

Read this guide before upgrading a running `weaverd` that was configured with a
non-default endpoint. A non-default endpoint is any Unix socket path or TCP
address that differs from the built-in default. Installations using the default
endpoint need no action.

The decision and its rationale are recorded in
[ADR 0014](adrs/adr-0014-endpoint-scoped-runtime-artefacts.md).

## What changed

Weaver now derives the daemon lock, PID, and health filenames from the complete
configured endpoint instead of using one fixed set of names per runtime
directory.

- The default endpoint keeps the legacy names: `weaverd.lock`, `weaverd.pid`,
  and `weaverd.health`.
- A non-default endpoint uses a stable identifier derived from SHA-256 over the
  endpoint kind and its complete address, for example
  `weaverd-<endpoint-hash>.pid`. The identifier is lowercase hexadecimal.

Two endpoints that share a runtime directory therefore no longer contend for,
or read, the same process state.

## Impact at a glance

| Priority                           | Area                   | What to do                                                    |
| ---------------------------------- | ---------------------- | ------------------------------------------------------------- |
| Required for non-default endpoints | Running daemon         | Stop the old daemon before upgrading.                         |
| None for default endpoints         | Existing installations | Upgrade normally.                                             |
| Recommended                        | Verification           | Confirm status and inspect the runtime directory after start. |

_Table 1: Obligations when adopting endpoint-scoped runtime artefacts._

## 1. Find the runtime directory and endpoint-specific files

The runtime directory depends on the endpoint:

- **Unix endpoint:** the parent directory of the configured socket path. For the
  default socket `$XDG_RUNTIME_DIR/weaver/weaverd.sock`, this is
  `$XDG_RUNTIME_DIR/weaver`. When `$XDG_RUNTIME_DIR` is unavailable, the
  fallback is a per-user directory under the system temporary directory, for
  example `/tmp/weaver/uid-<uid>`.
- **TCP endpoint:** the per-user runtime directory, `$XDG_RUNTIME_DIR/weaver`
  when available. Otherwise Weaver falls back to the system temporary
  directory, namespaced per user on Unix as `/tmp/weaver/uid-<uid>`, and
  `/tmp/weaver` on other platforms.

List the files after the new daemon starts. The listing shows the default names
or the endpoint-scoped names for the configured endpoint:

```sh
ls "$XDG_RUNTIME_DIR/weaver"
```

A non-default endpoint produces three matching files such as
`weaverd-<endpoint-hash>.lock`, `weaverd-<endpoint-hash>.pid`, and
`weaverd-<endpoint-hash>.health`, where `<endpoint-hash>` is the 64-character
SHA-256 identifier.

## 2. Stop the running daemon with the previous version

The current CLI cannot adopt the runtime files written by an older version for
a non-default endpoint. Legacy PID and health files contain no endpoint
identity, so the CLI cannot prove which process owns them. It refuses to signal
an unverified process even when the socket is reachable.

Stop the daemon before upgrading. Use the previous Weaver release with the same
configuration that started the daemon:

```sh
# Run with the previous release, before installing the new one.
weaver daemon stop
```

If `daemon stop` is unavailable, or the previous process cannot be reached that
way, read the PID recorded by the previous release and stop it directly. The
previous release wrote a shared `weaverd.pid` in the runtime directory
regardless of the configured endpoint:

```sh
# Read the PID written by the previous release.
cat "$XDG_RUNTIME_DIR/weaver/weaverd.pid"

# Replace <pid> with the value printed above.
kill <pid>
```

Replace `<pid>` with the numeric PID. Confirm the process has exited before
continuing.

## 3. Upgrade and start the new version

Install the new Weaver release, then start the daemon with the same endpoint
configuration:

```sh
weaver daemon start
```

Startup reports the runtime directory on standard error:

```text
runtime artefacts stored under /run/user/1000/weaver
```

## Verify the upgrade

Run:

```sh
weaver daemon status
```

A healthy daemon from the new release reports its state, PID, and endpoint:

```text
daemon status: ready (pid 12345) via unix:///run/user/1000/weaver/weaverd.sock
```

If the default endpoint is in use, the runtime directory continues to hold
`weaverd.lock`, `weaverd.pid`, and `weaverd.health`.

A stale legacy `weaverd.pid` or `weaverd.health` left by the previous release
is ignored for a non-default endpoint and is not reused. If the socket is
reachable but the endpoint-scoped PID file is missing, `weaver daemon status`
reports that endpoint-scoped runtime files are missing and advises stopping the
older daemon with the previous version or its known PID. Once no older daemon
can still be running, the leftover legacy file can be removed by hand.

## Normal cleanup semantics

On shutdown the daemon removes its PID and health files. The lock file is
intentionally retained and reused on the next start, so a persistent
`weaverd*.lock` file is expected rather than evidence of leakage. Do not delete
it to work around a startup problem; the lock protects against two daemons
running at once.

## What does not need to change

- Installations using the default endpoint keep the legacy filenames.
- The `weaver daemon start`, `weaver daemon stop`, and `weaver daemon status`
  command syntax is unchanged.
- Configuration keys and flags, including `--daemon-socket`, are unchanged.

## Migration checklist

- [ ] Determine whether the running daemon uses a non-default endpoint.
- [ ] Stop the running daemon with the previous release, or kill its known PID.
- [ ] Upgrade Weaver.
- [ ] Start the new daemon with the same endpoint configuration.
- [ ] Run `weaver daemon status` and confirm it reports a healthy state.
- [ ] Confirm the runtime directory holds the expected endpoint-scoped files.

## Common errors and fixes

- **Error:** the command reports that the socket is reachable but the
  endpoint-scoped PID file is missing, refuses to signal an unverified process,
  and advises stopping the older daemon with the previous version or its known
  PID before upgrading.
  - **Fix:** The running daemon predates endpoint-scoped filenames. Stop it with
    the previous release or its known PID, then start the new release.
