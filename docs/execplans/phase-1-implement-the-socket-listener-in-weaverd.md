# Implement the weaverd socket listener (Phase 1)

This ExecPlan is a living document. The sections `Constraints`, `Tolerances`,
`Risks`, `Progress`, `Surprises & Discoveries`, `Decision Log`, and
`Outcomes & Retrospective` must be kept up to date as work proceeds.

Status: DRAFT

No `PLANS.md` file exists in the repository root, so this document stands on
its own.

## Purpose / Big Picture

We need the daemon to accept client connections over the configured transport
so the CLI can connect reliably and the rest of the JSONL protocol can be
layered on top in the next roadmap step. Success is observable when the daemon
binds to the socket declared in configuration, accepts multiple concurrent
connections without crashing, and keeps running even when individual
connections fail. Unit tests and behavioural tests must cover success and
failure paths using `rstest-bdd` v0.3.2.

## Constraints

- Use the existing `SocketEndpoint` configuration contract from
  `crates/weaver-config` without changing the schema.
- Support Unix domain sockets on Unix targets and TCP sockets on all targets;
  fail fast with a clear error if a Unix socket is requested on a non-Unix
  platform.
- Do not introduce a JSONL request loop or change CLI request/response
  semantics in this phase.
- Keep modules under 400 lines and add a `//!` module-level comment to any new
  module.
- Use `rstest-bdd` v0.3.2 for behavioural tests; update the workspace
  dependency if required.
- Do not add new runtime dependencies beyond the `rstest-bdd` version bump
  unless explicitly approved.

## Tolerances (Exception Triggers)

- Scope: if the work requires touching more than 12 files or exceeds 500 net
  new/changed lines, stop and ask for guidance.
- Dependencies: if any new runtime dependency is required (other than
  updating `rstest-bdd`/`rstest-bdd-macros`), stop and ask for approval.
- Interfaces: if the CLI JSONL envelope, `weaver-config` schema, or public CLI
  flags must change, stop and ask for direction.
- Architecture: if an async runtime (Tokio, async-std) appears necessary to
  meet the acceptance criteria, stop and ask before proceeding.
- Tests: if `make test` still fails after two fix attempts, stop and report
  the failing cases.

## Risks

- Risk: Unix socket files can linger and cause bind failures after crashes.
  Severity: medium. Likelihood: medium.
  Mitigation: attempt safe cleanup of stale socket paths when no listener is
  active and log when cleanup is skipped.
- Risk: Non-blocking accept loops can spin if errors are not throttled.
  Severity: medium. Likelihood: medium.
  Mitigation: add bounded sleep/backoff on repeated accept errors.
- Risk: Updating `rstest-bdd` to 0.3.2 could break existing scenarios across
  crates. Severity: high. Likelihood: medium.
  Mitigation: update the workspace dependency first, run the full suite, and
  adjust any failing scenarios before implementing new steps.

## Progress

- [x] 2026-01-10 Drafted ExecPlan for the daemon socket listener.
- [ ] Inventory current daemon launch flow and socket-related helpers.
- [ ] Update workspace `rstest-bdd` dependencies to 0.3.2 (if not already) and
      confirm existing tests still compile.
- [ ] Add a socket listener module with a testable connection handler.
- [ ] Integrate the listener into `run_daemon_with` and graceful shutdown.
- [ ] Add unit tests for binding, cleanup, and error handling.
- [ ] Add `rstest-bdd` scenarios covering happy/unhappy connection paths.
- [ ] Update `docs/weaver-design.md` and `docs/users-guide.md` with design and
      behaviour notes.
- [ ] Mark the Phase 1 socket listener roadmap entry as done.
- [ ] Run `make check-fmt`, `make lint`, and `make test` successfully.

## Surprises & Discoveries

- None yet.

## Decision Log

- Decision: Pending — record the listener concurrency and cleanup strategy
  once the implementation approach is finalised.
  Date/Author: 2026-01-10 / plan author.

## Outcomes & Retrospective

Not started yet.

## Context and Orientation

`weaverd` currently boots, prepares runtime files, and then blocks on the
shutdown signal without binding any socket. The daemon launch flow is in
`crates/weaverd/src/process/launch.rs`, which loads configuration, prepares
socket directories, acquires the process lock, and then calls
`bootstrap_with`. The transport contract lives in
`crates/weaver-config/src/socket.rs` via `SocketEndpoint`, which distinguishes
Unix domain sockets (filesystem paths) from TCP sockets (host/port). The CLI
connects using `crates/weaver-cli/src/transport.rs` and expects the daemon to
accept either Unix or TCP connections.

A Unix domain socket is a local, filesystem-backed socket used for
inter-process communication on Unix-like systems. TCP sockets are network
sockets addressed by host and port; they work on all platforms.

Existing test harnesses for the daemon live under `crates/weaverd/src/tests/`
with Gherkin feature files in `crates/weaverd/tests/features/`.

Key files likely to change or be referenced:

- `crates/weaverd/src/process/launch.rs` (daemon runtime orchestration).
- `crates/weaverd/src/process/errors.rs` (launch error surface).
- `crates/weaverd/src/lib.rs` (module exports and crate-level docs).
- `crates/weaver-config/src/socket.rs` (socket endpoint contract).
- `crates/weaver-cli/src/transport.rs` (client transport expectations).
- `docs/weaver-design.md` and `docs/users-guide.md` (design and user-facing
  behaviour).
- `docs/roadmap.md` (Phase 1 socket listener entry).

## Plan of Work

Stage A: understand the current flow and requirements. Confirm where to bind
and how the daemon currently signals readiness. Review the CLI transport code
so the listener matches expected behaviour, and check whether any existing
socket cleanup logic exists to avoid duplicating it.

Stage B: introduce a dedicated listener module in `weaverd` that can bind to a
`SocketEndpoint` and accept connections via a small, testable interface. The
module should expose a connection handler trait so unit tests can record
connections without needing the full request loop. Decide and document how the
listener stops on shutdown and how it handles transient accept errors.

Stage C: wire the listener into `run_daemon_with`. The listener must bind
before the daemon reports `ready`, and it must shut down cleanly after the
shutdown signal is received. Ensure any socket files are cleaned up on exit.

Stage D: add unit tests and BDD scenarios. Unit tests should validate binding,
cleanup, and error classification. Behavioural tests should confirm that the
daemon accepts connections and that failure cases return structured errors
without crashing. Update the design and user guide documentation with the
chosen listener design and any new operator-visible behaviour. Finally, mark
the roadmap entry as complete and run the quality gates.

## Concrete Steps

Run these commands from the repository root (`/root/repo`). The `rg` commands
are safe to re-run.

1. Inventory socket-related code and daemon launch flow.

   rg -n "daemon_socket|SocketEndpoint|listener|accept" crates/weaverd
   rg -n "connect\(" crates/weaver-cli/src/transport.rs
   rg -n "socket" docs/users-guide.md docs/weaver-design.md

2. If the workspace still pins `rstest-bdd` to 0.2.x, update the workspace
   dependencies in `Cargo.toml` to 0.3.2 and adjust any tests that fail to
   compile.

3. Add a listener module, for example `crates/weaverd/src/transport.rs`, with:

   - A `DaemonListener`/`SocketListener` struct that binds to `SocketEndpoint`.
   - A `ConnectionHandler` trait for handling accepted connections.
   - A `ListenerHandle` that owns the accept loop thread and a shutdown flag.

4. Integrate the listener into `crates/weaverd/src/process/launch.rs` so the
   daemon reports ready only after binding. Ensure shutdown signals stop the
   listener and clean up Unix socket files.

5. Add unit tests alongside the listener module to validate:

   - TCP binding on an ephemeral port.
   - Unix socket binding (on Unix) and cleanup of stale socket files.
   - Error handling when the socket is already in use.

6. Add BDD coverage using `rstest-bdd` v0.3.2. Add a feature file such as
   `crates/weaverd/tests/features/daemon_socket.feature` and new step
   definitions under `crates/weaverd/src/tests/` to cover:

   - Happy path: daemon accepts a connection on the configured socket.
   - Unhappy path: binding fails when the socket is already in use.
   - Concurrency: two clients can connect without crashing the daemon.

7. Update documentation:

   - `docs/weaver-design.md` to record the listener strategy and error
     handling decisions.
   - `docs/users-guide.md` to describe any user-visible behaviour changes.
   - `docs/roadmap.md` to mark the socket listener item as done.

8. Format and validate documentation if any docs changed:

   set -o pipefail
   make fmt 2>&1 | tee /tmp/weaver-fmt.log
   make markdownlint 2>&1 | tee /tmp/weaver-markdownlint.log

   Run `make nixie` only if a Mermaid diagram was edited.

9. Run the Rust quality gates:

   set -o pipefail
   make check-fmt 2>&1 | tee /tmp/weaver-check-fmt.log
   make lint 2>&1 | tee /tmp/weaver-lint.log
   make test 2>&1 | tee /tmp/weaver-test.log

## Validation and Acceptance

Acceptance requires all of the following:

- Unit tests demonstrate binding success and failure cases for both Unix and
  TCP endpoints (platform permitting).
- New `rstest-bdd` scenarios pass and cover at least one happy path and one
  unhappy path for the listener.
- `make check-fmt`, `make lint`, and `make test` succeed.
- `docs/users-guide.md` documents any relevant behavioural changes for the
  daemon socket listener.
- `docs/weaver-design.md` records the design decisions taken.
- The Phase 1 roadmap entry for the socket listener is marked as done.

## Idempotence and Recovery

All steps are safe to re-run. If a test or lint step fails, fix the issue and
re-run the same command. If a Unix socket file is left behind by a failed run,
remove it and rerun the listener test; the implementation should also attempt
safe cleanup on startup.

## Artifacts and Notes

Example Gherkin snippet for the listener behaviour:

  Feature: Daemon socket listener

    Scenario: Accepting a client connection
      Given the daemon is running with a TCP socket
      When a client connects
      Then the daemon records the connection

## Interfaces and Dependencies

The listener module should expose a small, testable interface, for example:

- `weaverd::transport::SocketListener` with
  `fn bind(endpoint: &SocketEndpoint) -> Result<Self, ListenerError>`.
- `weaverd::transport::ConnectionHandler` trait with
  `fn handle(&self, stream: ConnectionStream)` where `ConnectionStream` wraps
  `TcpStream` or `UnixStream`.
- `weaverd::transport::ListenerHandle` with `fn shutdown(self)` and
  `fn join(self) -> Result<(), ListenerError>` to coordinate shutdown.

`ListenerError` should wrap binding and accept failures without panicking and
should be convertible into `LaunchError` so `run_daemon_with` can fail fast on
bind errors.

Dependencies should remain unchanged except for updating `rstest-bdd` and
`rstest-bdd-macros` to v0.3.2.

## Revision note

2026-01-10: Replaced the previous weaver-graph ExecPlan with a Phase 1 socket
listener plan to match the user request. This shifts scope to `weaverd`
transport setup and introduces new testing and documentation updates required
for the listener work.
