# 2.3.2 Include valid operation alternatives for unknown operations

This ExecPlan (execution plan) is a living document. The sections
`Constraints`, `Tolerances`, `Risks`, `Progress`, `Surprises & Discoveries`,
`Decision Log`, and `Outcomes & Retrospective` must be kept up to date as work
proceeds.

Status: COMPLETE

This document must be maintained in accordance with `AGENTS.md` at the
repository root.

## Purpose / big picture

Roadmap item `2.3.2` closes the current daemon-side gap for valid but
unsupported operation names.[^1] Today, `weaver observe nonexistent` reaches
`weaverd`, which replies with a plain-text error:

```plaintext
error: unknown operation 'nonexistent' for domain 'observe'
```

That message does not tell the operator which operations are valid for the
chosen domain, even though the daemon router already owns the canonical
`known_operations` arrays.[^2] After this change, unknown-operation failures
must remain daemon-routed, but both output modes must become actionable:

```plaintext
$ weaver observe nonexistent
error: unknown operation 'nonexistent' for domain 'observe'

Available operations:
  get-definition
  find-references
  grep
  diagnostics
  call-hierarchy
  get-card
```

And:

```json
$ weaver --output json observe nonexistent
{
  "status": "error",
  "type": "UnknownOperation",
  "details": {
    "domain": "observe",
    "operation": "nonexistent",
    "known_operations": [
      "get-definition",
      "find-references",
      "grep",
      "diagnostics",
      "call-hierarchy",
      "get-card"
    ]
  }
}
```

The JSON payload must contain the full operation set for the routed domain, and
the human-readable output must be rendered from that same payload so the CLI
does not drift from the daemon router. This closes Level 4 and Level 10c from
the UI gap analysis.[^3][^4]

## Constraints

- Run `make check-fmt`, `make lint`, and `make test` before considering the
  feature complete.
- Because this task also updates Markdown, run `make fmt`,
  `make markdownlint`, and `make nixie` before finishing.
- Add unit coverage and behavioural coverage using `rstest-bdd` v0.5.0 for the
  daemon and CLI surfaces. Cover the primary path, regressions, and edge cases.
- Keep unknown-operation validation daemon-side. Do not add client-side
  preflight for operation names in this task.
- Keep the daemon router as the source of truth for the returned alternatives.
  The CLI must not reconstruct the operation list from its local catalogue when
  rendering unknown-operation errors.
- Preserve the existing client-side unknown-domain flow from roadmap `2.3.1`.
  `weaver bogus something` must still fail before config loading or daemon
  startup.[^5]
- Preserve the outer JSONL transport contract. The daemon may extend the inner
  payload carried in `DaemonMessage::Stream`, but it must not replace the
  `stream` and `exit` envelope in this task.
- Keep exit status behaviour stable. Unknown-operation failures must still
  exit with status `1`.
- The returned operation list must contain every entry from the routed domain's
  `known_operations` slice, in router order, with a count equal to that slice's
  length.
- Update `docs/weaver-design.md` with the final source-of-truth and wire-format
  decision.
- Update `docs/users-guide.md` with the new human-readable and JSON examples.
- Mark roadmap item `2.3.2` done only after code, documentation, and all gates
  pass.
- Keep code files under 400 lines by extracting helpers or payload modules
  before growing a hot file past the limit.
- Comments and documentation must use en-GB-oxendict spelling.

## Tolerances (exception triggers)

- Scope: if the work grows beyond roughly 14 touched files or 300 net lines,
  stop and re-evaluate before continuing.
- Protocol shape: if satisfying the JSON acceptance criteria appears to require
  a breaking change to the outer `DaemonMessage` schema, stop and escalate.
- Shared catalogue pressure: if this task cannot be completed without creating
  a new shared crate or moving the CLI catalogue into another package, stop and
  escalate rather than broadening scope silently.
- File size: if `crates/weaverd/src/dispatch/router.rs`,
  `crates/weaverd/src/dispatch/response.rs`, or
  `crates/weaver-cli/src/output/mod.rs` would exceed 400 lines, extract a
  focused helper module before adding more code.
- Rendering ambiguity: if the existing CLI output model cannot distinguish the
  new error payload cleanly from other daemon payloads without weakening
  current human rendering, stop and document the options before proceeding.
- Test harness sprawl: if behavioural coverage requires a second bespoke test
  harness instead of reusing the existing fake-daemon and dispatch-world seams,
  stop and re-scope.

## Risks

- Risk: `DispatchError::UnknownOperation` currently carries only `domain` and
  `operation`, so `ResponseWriter::write_error(...)` can emit only plain text.
  Mitigation: extend the error variant or add an adjacent payload type so
  `ResponseWriter::write_error(...)` can serialize the canonical
  `known_operations` list without guessing.

- Risk: the CLI already has its own domain-operation catalogue in
  `crates/weaver-cli/src/discoverability.rs`, and prior work found drift
  between that catalogue and the daemon router. Mitigation: render unknown
  operations from daemon-provided payload data, not from `DOMAIN_OPERATIONS`.

- Risk: the current human renderer only knows a few typed JSON payloads
  (`CapabilityResolution`, diagnostics, definitions, and verification errors).
  Mitigation: add a narrow parser for `UnknownOperation` payloads and keep the
  fallback path unchanged for everything else.

- Risk: the fake-daemon CLI behaviour tests do not currently exercise
  structured error payloads. Mitigation: reuse the existing
  `start_daemon_with_lines(...)` seam to inject a realistic daemon stream for
  both human and JSON CLI scenarios.

- Risk: Cargo build-lock contention from background `cargo check` activity can
  make `make lint` and `make test` appear hung. Mitigation: if a gate stalls on
  `Blocking waiting for file lock on build directory`, inspect the lock holder
  with `ps -eo pid,ppid,stat,etime,cmd | rg 'cargo|rustc'` and clear the
  background process before rerunning the gate.

## Progress

- [x] (2026-03-28 00:00Z) Read `docs/roadmap.md`,
      `docs/ui-gap-analysis.md`, `docs/weaver-design.md`,
      `docs/users-guide.md`, and the referenced testing guides.
- [x] (2026-03-28 00:10Z) Confirmed the live daemon path:
      `DomainRouter::route_fallback(...)` returns
      `DispatchError::UnknownOperation` without any operation alternatives, and
      `ResponseWriter::write_error(...)` serializes only the display string.
- [x] (2026-03-28 00:15Z) Confirmed the live CLI path:
      `read_daemon_messages(...)` can already human-render typed JSON payloads,
      but unknown-operation errors arrive only as plain stderr text today.
- [x] (2026-03-28 00:20Z) Confirmed that the workspace already pins
      `rstest-bdd` and `rstest-bdd-macros` at `0.5.0`, so no dependency update
      is required.
- [x] (2026-03-28 00:30Z) Drafted this ExecPlan in
      `docs/execplans/2-3-2-valid-operation-alternatives-for-unknown-operations.md`.
- [x] (2026-03-28 23:15Z) Stage A: added failing unit and behavioural tests
      covering daemon payload shape, CLI human rendering, and JSON passthrough
      for unknown operations.
- [x] (2026-03-28 23:22Z) Stage B: extended `DispatchError::UnknownOperation`
      with canonical `known_operations` and taught `ResponseWriter` to emit a
      structured `UnknownOperation` JSON payload on stderr.
- [x] (2026-03-28 23:30Z) Stage C: added CLI parsing and human rendering for
      `UnknownOperation` payloads and confirmed the new `rstest-bdd` scenarios
      pass for both daemon and CLI.
- [x] (2026-03-29 12:45Z) Stage D: updated `docs/weaver-design.md`,
      `docs/users-guide.md`, and `docs/roadmap.md` to describe the shipped
      daemon-routed `UnknownOperation` contract and marked roadmap item
      `2.3.2` done.
- [x] (2026-03-29 12:59Z) Stage E: ran `make fmt`,
      `make markdownlint`, `make nixie`, `make check-fmt`, `make lint`, and
      `make test` successfully after fixing one daemon unit assertion to parse
      the inner JSON payload from the stream envelope.

## Surprises & Discoveries

- Unknown operations are not a client-side validation problem. The CLI has no
  authoritative per-domain operation validator today, and the roadmap item
  explicitly targets daemon and CLI error payloads rather than preflight.

- The daemon already has the exact canonical data needed for this feature:
  `DomainRoutingContext::{OBSERVE, ACT, VERIFY}.known_operations` in
  `crates/weaverd/src/dispatch/router.rs`.

- The CLI output pipeline is already prepared for this kind of change. It can
  forward raw JSON unchanged in `--output json` mode and can render typed JSON
  payloads in human mode without touching the transport layer.

- `docs/users-guide.md` currently documents the improved unknown-domain
  behaviour from `2.3.1`, but it does not yet explain that unknown operations
  are daemon-routed and should return structured alternatives.

- `ResponseWriter::write_error(...)` was sufficient for the daemon change once
  it learned to special-case `DispatchError::UnknownOperation`; no extra
  handler-level branching was needed in `DispatchConnectionHandler`.

## Decision Log

- Decision: keep unknown-operation detection daemon-side instead of
  adding CLI pre-validation. Rationale: the daemon router already owns the
  canonical operation list for each domain, while the CLI catalogue is a
  discoverability aid rather than the dispatch authority. Date: 2026-03-28.

- Decision: emit an additive structured error payload inside the
  existing `DaemonMessage::Stream` envelope instead of changing the outer JSONL
  protocol. Rationale: this satisfies the JSON-output acceptance criteria while
  preserving transport compatibility and keeping scope aligned with `2.3.2`.
  Date: 2026-03-28.

- Decision: have the CLI human renderer format the daemon-provided
  `known_operations` array into an `Available operations:` block, one operation
  per line. Rationale: the JSON payload remains machine-friendly, while the
  human output stays consistent with the discoverability style already used for
  `weaver <domain>`. Date: 2026-03-28.

- Decision: keep the list order identical to the router's
  `known_operations` slice and test that exact order. Rationale: deterministic
  order makes operator guidance predictable and gives tests a concrete
  contract. Date: 2026-03-28.

- Decision: special-case `UnknownOperation` inside
  `ResponseWriter::write_error(...)` rather than adding a second dispatch-layer
  error serializer. Rationale: this kept the change local to the existing
  error-writing seam and avoided widening `DispatchConnectionHandler`. Date:
  2026-03-28.

## Outcomes & Retrospective

Target outcome at completion:

1. `weaverd` emits a structured unknown-operation payload containing
   `domain`, `operation`, and the full `known_operations` array for the routed
   domain.
2. `weaver --output json <domain> <unknown-operation>` forwards that payload
   unchanged and still exits with code `1`.
3. `weaver <domain> <unknown-operation>` renders a readable
   `Available operations:` block from the structured daemon payload.
4. The daemon and CLI behavioural suites exercise the new path with
   `rstest-bdd` using the existing harnesses.
5. `docs/weaver-design.md`, `docs/users-guide.md`, and `docs/roadmap.md`
   reflect the shipped behaviour.
6. `make fmt`, `make markdownlint`, `make nixie`, `make check-fmt`,
   `make lint`, and `make test` all pass.

Retrospective notes:

- The daemon and CLI behavioural coverage landed without any new harnesses by
  extending the existing dispatch-world and fake-daemon seams.
- The only full-gate regression was in a new daemon unit test that asserted on
  the outer JSONL envelope as plain text. Parsing the inner `data` JSON fixed
  the test and better matched the wire contract under test.
- Final verification succeeded with `make fmt`, `make markdownlint`,
  `make nixie`, `make check-fmt`, `make lint`, and `make test`.

## Context and orientation

The change spans two crates and three documentation files.

- `crates/weaverd/src/dispatch/router.rs`
  Owns `DomainRoutingContext` and `route_fallback(...)`. This is where the
  daemon decides that an operation is unknown and where the canonical
  alternatives originate.
- `crates/weaverd/src/dispatch/errors.rs`
  Defines `DispatchError::UnknownOperation`. The chosen payload shape should be
  represented here or in a dedicated nearby payload type.
- `crates/weaverd/src/dispatch/response.rs`
  Owns `ResponseWriter` and the current `write_error(...)` helper. This is the
  likely serialization seam for emitting a structured unknown-operation error.
- `crates/weaverd/src/dispatch/handler.rs`
  Routes dispatch failures through `writer.write_error(...)`, but the
  unknown-operation serialization seam itself should remain inside
  `ResponseWriter::write_error(...)` so callers do not need their own special
  cases.
- `crates/weaverd/src/dispatch/router/tests.rs`
  Contains parameterized router tests against `DomainRoutingContext`.
- `crates/weaverd/src/tests/dispatch_behaviour.rs` and
  `crates/weaverd/tests/features/daemon_dispatch.feature` Provide the
  `rstest-bdd` seam for daemon wire-behaviour assertions.
- `crates/weaver-cli/src/daemon_output.rs`
  Reads daemon stream messages, forwards raw JSON in JSON mode, and calls the
  human renderer otherwise.
- `crates/weaver-cli/src/output/models.rs`
  Parses typed daemon payloads such as `CapabilityResolution`.
- `crates/weaver-cli/src/output/mod.rs`
  Selects the human renderer based on the payload type and command context.
- `crates/weaver-cli/tests/features/weaver_cli.feature` and
  `crates/weaver-cli/src/tests/behaviour.rs` Provide the CLI `rstest-bdd`
  scenarios and fake-daemon steps.
- `docs/weaver-design.md`
  Must record that unknown operations remain daemon-routed and that the daemon
  payload, not the CLI catalogue, is the source of truth for alternatives.
- `docs/users-guide.md`
  Must show the new operator-visible human and JSON behaviour.
- `docs/roadmap.md`
  Must mark `2.3.2` done once the implementation and all gates are complete.

## Implementation plan

### Stage A - write the failing tests first

Start by making the missing contract observable. Add tests before any
production changes so the work follows a red-green-refactor sequence.

In `crates/weaverd/src/dispatch/router/tests.rs`, add parameterized unit tests
that exercise one unknown operation per domain and assert that the resulting
`DispatchError::UnknownOperation` carries the exact `known_operations` slice in
router order. Reuse the existing `#[rstest]` pattern already used for routing
tests.

In `crates/weaverd/tests/features/daemon_dispatch.feature`, extend the unknown
operation scenario so it asserts the wire response includes the alternatives.
Then update `crates/weaverd/src/tests/dispatch_behaviour.rs` with helpers that
parse the inner JSON payload from the daemon stream, count `known_operations`,
and compare the array against the expected domain list.

In `crates/weaver-cli/tests/features/weaver_cli.feature`, add two scenarios:
one for human output and one for `--output json`. Reuse the fake-daemon harness
in `crates/weaver-cli/src/tests/behaviour.rs` by enqueueing a daemon line that
contains the new structured unknown-operation payload. The human scenario
should assert `Available operations:` plus all operations for `observe`. The
JSON scenario should assert the raw JSON payload is forwarded unchanged and
still exits non-zero.

Suggested red-phase commands:

```sh
set -o pipefail; cargo test -p weaverd unknown_operation 2>&1 | tee /tmp/2-3-2-weaverd-red.log
set -o pipefail; cargo test -p weaver-cli unknown_operation 2>&1 | tee /tmp/2-3-2-weaver-cli-red.log
```

Expected red-phase evidence:

```plaintext
assertion failed: payload["details"]["known_operations"] == ...
assertion failed: stderr did not contain "Available operations:"
```

### Stage B - teach the daemon to emit structured unknown-operation payloads

Once the tests fail for the right reason, implement the daemon-side source of
truth.

Extend the unknown-operation path so the daemon can serialize a payload shaped
like the other typed responses already used by the CLI:

```json
{
  "status": "error",
  "type": "UnknownOperation",
  "details": {
    "domain": "<domain>",
    "operation": "<operation>",
    "known_operations": ["..."]
  }
}
```

Keep the outer `DaemonMessage::Stream` wrapper unchanged. The payload should be
written as a single JSON string on the error stream and followed by the normal
`Exit { status: 1 }` message.

The smallest viable implementation is:

1. Extend `DispatchError::UnknownOperation` so it carries the canonical
   `known_operations` slice for the routed domain.
2. Change `DomainRouter::route_fallback(...)` to pass that slice into the
   error.
3. Add a serializer helper near `ResponseWriter` that converts the error into
   the structured JSON payload above.
4. Special-case `DispatchError::UnknownOperation` inside
   `ResponseWriter::write_error(...)` so callers keep using the same error
   writing path while other errors continue through the display-string branch.

Do not widen this into a general error-envelope refactor. That broader cleanup
belongs to roadmap `2.3.3`.

### Stage C - render the new payload in the CLI

After the daemon emits a structured payload, teach the CLI to render it.

Add a parser for the new `UnknownOperation` payload in
`crates/weaver-cli/src/output/models.rs`. Then update
`crates/weaver-cli/src/output/mod.rs` so `render_human_output(...)` can detect
that payload and render it into the same multi-line guidance block asserted by
the CLI behaviour tests.

Keep `--output json` simple. The CLI already forwards raw payloads unchanged in
JSON mode, so no extra formatting is required once the daemon sends JSON in the
stream data.

Add or extend unit tests around the parser and human renderer so they prove:

- the payload is accepted only when `"type": "UnknownOperation"`,
- the rendered text includes the error line and every known operation,
- payloads with a different `"type"` still fall through to the existing
  renderer logic, and
- raw JSON mode remains unchanged.

### Stage D - update the design and operator documentation

Document the behavioural contract immediately after the code settles.

Update `docs/weaver-design.md` in the CLI/daemon protocol section to state that
unknown domains are rejected client-side, while unknown operations are rejected
daemon-side because the daemon router owns the canonical per-domain operation
lists. Record that the daemon emits a structured `UnknownOperation` payload and
that the CLI human renderer formats the `known_operations` array rather than
consulting a local list.

Update `docs/users-guide.md` with one human-readable example and one
`--output json` example so operators understand both surfaces. Make it clear
that the returned operations list comes from the daemon router and may include
implemented and not-yet-implemented operations alike.

After code, tests, and docs are complete, mark roadmap item `2.3.2` done in
`docs/roadmap.md`.

### Stage E - run the full gates and capture evidence

Run the documentation gates first because this task changes Markdown, then run
the Rust gates required by the roadmap and `AGENTS.md`.

```sh
set -o pipefail; make fmt 2>&1 | tee /tmp/2-3-2-make-fmt.log
set -o pipefail; make markdownlint 2>&1 | tee /tmp/2-3-2-make-markdownlint.log
set -o pipefail; make nixie 2>&1 | tee /tmp/2-3-2-make-nixie.log
set -o pipefail; make check-fmt 2>&1 | tee /tmp/2-3-2-make-check-fmt.log
set -o pipefail; make lint 2>&1 | tee /tmp/2-3-2-make-lint.log
set -o pipefail; make test 2>&1 | tee /tmp/2-3-2-make-test.log
```

Expected success evidence:

```plaintext
$ tail -n 5 /tmp/2-3-2-make-lint.log
...
Finished `dev` profile ... target(s) in ...

$ tail -n 5 /tmp/2-3-2-make-test.log
...
test result: ok. ... passed; 0 failed; ...
```

If a gate stalls on a Cargo build lock, inspect active `cargo` or `rustc`
processes before rerunning the command:

```sh
ps -eo pid,ppid,stat,etime,cmd | rg 'cargo|rustc'
```

## Acceptance checklist

This work is complete only when all of the following are true:

1. Unknown-operation daemon responses carry the routed domain's complete
   `known_operations` set in JSON.
2. The JSON payload count matches the router slice length for the routed
   domain.
3. Human-readable CLI output includes the same full list as an
   `Available operations:` block.
4. `--output json` forwards the structured payload unchanged.
5. Unit tests and `rstest-bdd` behavioural tests pass for both daemon and CLI
   paths.
6. `docs/weaver-design.md`, `docs/users-guide.md`, and `docs/roadmap.md` are
   updated.
7. `make fmt`, `make markdownlint`, `make nixie`, `make check-fmt`,
   `make lint`, and `make test` all pass.

[^1]: `docs/roadmap.md` item `2.3.2`.
[^2]: `docs/ui-gap-analysis.md` Level 4.
[^3]: `docs/ui-gap-analysis.md#level-4--unknown-operation-weaver-observe-nonexistent`.
[^4]: `docs/ui-gap-analysis.md#level-10--error-messages-and-exit-codes`.
[^5]: `docs/execplans/2-3-1-validate-domains-client-side-before-daemon-startup.md`.
