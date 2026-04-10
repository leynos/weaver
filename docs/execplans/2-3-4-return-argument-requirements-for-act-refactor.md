# 2.3.4 Return complete argument requirements for `act refactor`

This ExecPlan (execution plan) is a living document. The sections
`Constraints`, `Tolerances`, `Risks`, `Progress`, `Surprises & Discoveries`,
`Decision Log`, and `Outcomes & Retrospective` must be kept up to date as work
proceeds.

Status: DRAFT

Implementation must not begin until this plan is explicitly approved.

This document must be maintained in accordance with `AGENTS.md` at the
repository root.

## Purpose / big picture

Roadmap item `2.3.4` closes UI gap `5b` by making `weaver act refactor` surface
the full operator contract when required arguments are missing. Today the
daemon stops at the first missing field and returns only one message, for
example `act refactor requires --refactoring <operation>` or
`act refactor requires --file <path>`. That leaves operators guessing which
other flags are mandatory, which provider names are valid, and which
refactoring names the MVP actually supports.

After this change, running `weaver act refactor` without the required flags
must fail with one deterministic, actionable response that lists:

1. all three required flags: `--provider`, `--refactoring`, and `--file`;
2. at least one valid provider value;
3. at least one valid refactoring value.

The response should stay aligned with the Level 10 error-template policy
introduced in roadmap item `2.3.3`: a clear problem statement, the valid
alternatives, and one concrete next command.

This work is successful when the following are all true:

1. `weaver act refactor` without arguments reports the full required-argument
   set in one response instead of failing one flag at a time.
2. The response includes the registered provider names currently shipped by the
   MVP, at minimum `rope` and `rust-analyzer`.
3. The response includes the supported user-facing refactoring operations
   currently implemented by the MVP, at minimum `rename`.
4. Unit tests and `rstest-bdd` behavioural tests cover the happy path, the
   missing-arguments failure path, and edge cases such as partially supplied
   flags and unsupported values.
5. `docs/weaver-design.md`, `docs/users-guide.md`, and `docs/roadmap.md`
   reflect the final operator contract.

## Constraints

- Run `make check-fmt`, `make lint`, and `make test` before considering the
  feature complete.
- Because this task changes Markdown, also run `make fmt`,
  `make markdownlint`, and `make nixie` before finishing.
- Add both unit coverage and behavioural coverage using `rstest-bdd` v0.5.0.
  Reuse the existing `act refactor` feature harness in `crates/weaverd/` rather
  than creating a second bespoke BDD harness.
- Keep the JSONL transport envelope unchanged. This task may improve the
  `InvalidArguments` message body, but it must not add a new top-level response
  type or widen the `stream` / `exit` protocol contract.
- Treat the roadmap acceptance criteria as authoritative for this item. The
  current implementation and current documentation describe `--provider` as
  optional, but this plan assumes `--provider` becomes a required operator
  argument for `act refactor`.
- Keep provider and refactoring enumerations sourced from one canonical place
  in code. Do not duplicate provider names or supported refactorings across
  parser, handler, tests, and docs with independent hard-coded lists.
- Preserve the existing safety-critical flow after validation succeeds:
  capability resolution, plugin execution, Double-Lock verification, and atomic
  patch application must remain the only write path.
- Do not add a new external dependency for argument rendering, validation, or
  test scaffolding.
- Keep files under 400 lines. If `crates/weaverd/src/dispatch/act/refactor/`
  grows too large, extract a focused helper module rather than extending a
  crowded file.
- Comments and documentation must use en-GB-oxendict spelling.
- Update `docs/weaver-design.md` with the final MVP contract for required
  `act refactor` arguments and the source of truth for valid provider and
  refactoring values.
- Update `docs/users-guide.md` so the syntax, parameter tables, examples, and
  failure guidance match the new behaviour.
- Mark roadmap item `2.3.4` done only after implementation, documentation, and
  every validation gate passes.

## Tolerances (exception triggers)

- Scope: if implementation requires changes to more than 14 files or roughly
  450 net lines, stop and re-evaluate before continuing.
- Interfaces: if satisfying the acceptance criteria requires a new public type
  outside `crates/weaverd/src/dispatch/act/refactor/`, stop and escalate before
  broadening the API surface.
- Behaviour drift: if making `--provider` required would break an explicitly
  approved workflow outside `act refactor`, stop and document the conflict
  before proceeding.
- Dependencies: if the canonical provider/refactoring enumeration appears to
  require a new crate, stop and escalate.
- File-size pressure: if any file in
  `crates/weaverd/src/dispatch/act/refactor/` or `crates/weaver-e2e/tests/`
  would exceed 400 lines, extract helpers before adding more logic.
- Ambiguity: if the existing roadmap, design doc, and user guide cannot be
  reconciled around whether provider auto-routing should remain user-visible,
  stop and confirm the intended contract before implementation.
- Iterations: if the same validation or test suite fails more than five times
  while implementing this item, stop and report the blocker.

## Risks

- Risk: the current implementation, behavioural tests, and user guide all
  treat `--provider` as optional and rely on automatic provider routing.
  Changing the operator contract to make `--provider` required is broader than
  a message-only tweak. Severity: high. Likelihood: high. Mitigation: make the
  contract change explicit in this plan, update the affected tests and docs in
  the same change, and require approval before implementation begins.

- Risk: provider names are currently implied by manifest registration in
  `default_runtime()` and related manifest helpers. It is easy to repeat those
  names in argument validation instead of deriving them from a shared helper.
  Severity: medium. Likelihood: high. Mitigation: introduce one
  refactor-specific requirements helper that exposes the valid provider list to
  both validation code and tests.

- Risk: supported user-facing refactoring operations are narrower than the
  aspirational examples in `docs/weaver-design.md`, which still mention
  operations such as `extract_method`. Severity: medium. Likelihood: high.
  Mitigation: constrain the error message and the docs to the implemented MVP
  surface, currently `rename`, and explicitly update the design document to
  distinguish current support from future direction.

- Risk: a naive parser refactor could still short-circuit on the first missing
  flag, making the error body incomplete and brittle. Severity: medium.
  Likelihood: medium. Mitigation: drive the work with red-phase unit tests that
  assert all three required flags appear together, regardless of which subset
  of flags was supplied.

- Risk: the current BDD happy paths are named and structured around automatic
  routing. If provider becomes required, those scenarios may become misleading
  even if the write path still works. Severity: medium. Likelihood: high.
  Mitigation: rename the happy-path scenarios to use explicit provider
  selection and add one failure scenario dedicated to missing arguments.

## Progress

- [x] (2026-04-10) Read `docs/roadmap.md`, `docs/ui-gap-analysis.md`,
  `docs/weaver-design.md`, `docs/users-guide.md`, and the referenced testing
  guidance.
- [x] (2026-04-10) Confirmed that
  `crates/weaverd/src/dispatch/act/refactor/arguments.rs` currently reports
  only the first missing required flag and still models `provider` as optional.
- [x] (2026-04-10) Confirmed that the workspace already pins `rstest-bdd` and
  `rstest-bdd-macros` at `0.5.0`.
- [x] (2026-04-10) Confirmed that the current behavioural tests and user guide
  still describe automatic provider routing when `--provider` is omitted.
- [x] (2026-04-10) Drafted this ExecPlan in
  `docs/execplans/2-3-4-return-argument-requirements-for-act-refactor.md`.
- [ ] Stage A: add failing unit, behavioural, and end-to-end assertions for
  the new required-arguments contract.
- [ ] Stage B: introduce one canonical requirements helper for valid providers,
  valid refactorings, and complete missing-argument rendering.
- [ ] Stage C: update `act refactor` parsing and routing so the new
  requirements are enforced consistently.
- [ ] Stage D: update `docs/weaver-design.md`, `docs/users-guide.md`, and
  `docs/roadmap.md`.
- [ ] Stage E: run the full Markdown and Rust validation gates sequentially.

## Surprises & Discoveries

- `docs/ui-gap-analysis.md` and the roadmap acceptance criteria both speak
  about `--provider` as a required flag, but the shipped daemon code,
  behavioural tests, and user guide all moved to an optional-provider,
  auto-routing model. This item therefore needs an explicit product decision,
  not just a parser polish.

- The current missing-argument logic lives in the parser builder, not in the
  handler. That is the right seam for this work because it lets the daemon
  reject incomplete requests before plugin resolution, file I/O, or backend
  startup.

- The currently implemented user-facing refactoring surface is narrower than
  the design document's aspirational examples. In practice the MVP advertises
  `rename`, which the daemon maps to the `rename-symbol` capability contract.

- Existing BDD coverage for `act refactor` already exercises the safe write
  path and refusal path. Extending that same feature file is lower risk than
  adding a second behavioural harness.

## Decision Log

- Decision: this plan treats the roadmap acceptance criteria as the source of
  truth and therefore requires `--provider`, `--refactoring`, and `--file` for
  operator-facing `act refactor` usage. Rationale: the task explicitly says the
  no-argument response must report all three required flags. Listing
  `--provider` as required while still treating it as optional would make the
  guidance inaccurate. Date: 2026-04-10.

- Decision: keep argument validation daemon-side for this roadmap item.
  Rationale: the valid provider list comes from daemon-managed plugin
  registration, and this task can be completed without the larger clap
  subcommand redesign discussed in gap `5a`. Date: 2026-04-10.

- Decision: derive the valid provider list and the valid refactoring list from
  small shared helpers inside the `act refactor` dispatch module. Rationale:
  this keeps validation, tests, and documentation aligned and avoids hidden
  drift between parser text and runtime capabilities. Date: 2026-04-10.

- Decision: keep the error body aligned with the three-part template from
  roadmap item `2.3.3`. Rationale: `2.3.4` is another actionable-error
  improvement, so the new message should remain consistent with the rest of the
  CLI's human-readable failure surfaces. Date: 2026-04-10.

## Outcomes & Retrospective

Target outcome at completion:

1. `weaver act refactor` without arguments emits one response that names
   `--provider`, `--refactoring`, and `--file`, plus at least one valid
   provider and refactoring value.
2. The message no longer changes depending on which single missing flag is
   encountered first.
3. Successful `act refactor` flows still pass through the existing
   capability-resolution and Double-Lock patch-application path.
4. Unit tests cover complete and partial missing-argument cases, plus the
   success path and unsupported-value edge cases.
5. `rstest-bdd` scenarios cover the happy path and the new unhappy-path
   validation behaviour.
6. Any end-to-end CLI assertion added for the operator-visible output passes.
7. `docs/weaver-design.md`, `docs/users-guide.md`, and `docs/roadmap.md`
   accurately describe the shipped behaviour.
8. `make fmt`, `make markdownlint`, `make nixie`, `make check-fmt`,
   `make lint`, and `make test` all pass.

Retrospective notes will be added after implementation.

## Context and orientation

The current implementation is concentrated in
`crates/weaverd/src/dispatch/act/refactor/`.

`arguments.rs` parses raw request tokens into `RefactorArgs`. Its builder
requires `--refactoring` and `--file`, but not `--provider`, and it returns a
single `DispatchError::invalid_arguments(...)` for the first missing field.
That makes it impossible to report the full contract in one response.

`mod.rs` consumes `RefactorArgs`, reads the target file, resolves a provider,
executes the plugin, and forwards successful diff output into the existing
`act apply-patch` safety pipeline. The validation change should happen before
this heavier work begins.

`crates/weaverd/src/dispatch/act/refactor/tests.rs` already contains focused
unit tests for the handler and runtime failures. `arguments.rs` also contains
small parser-specific unit tests. Those are the right places to add red-phase
tests for the complete missing-arguments contract and for any new shared
requirements helpers.

`crates/weaverd/tests/features/refactor.feature` and
`crates/weaverd/src/dispatch/act/refactor/behaviour.rs` provide the current
`rstest-bdd` behavioural harness. Those scenarios already verify successful
diff application and deterministic refusal paths. They should be extended with
one missing-arguments scenario and updated to use the approved provider
contract.

`crates/weaver-e2e/tests/refactor_rope_cli_snapshots.rs` and
`crates/weaver-e2e/tests/refactor_rust_analyzer_cli_snapshots.rs` exercise the
operator-facing CLI workflows. A small end-to-end assertion or snapshot should
be added here if needed to prove that the human-visible `weaver act refactor`
output matches the acceptance criteria.

## Implementation plan

### Stage A: lock in the required behaviour with failing tests

Start by adding failing tests before any production change.

Add parser-level or handler-level unit tests that assert:

- no-argument requests mention `--provider`, `--refactoring`, and `--file`
  together;
- partially specified requests still mention every missing required flag in one
  message rather than only the first missing flag;
- the message includes valid provider names and valid refactoring names;
- successful requests with all required flags still parse cleanly.

Extend the existing `rstest-bdd` feature with one scenario for missing
arguments and, if `--provider` becomes required, rename the current happy-path
scenarios so they pass explicit provider values instead of relying on automatic
routing.

If the daemon-level tests are not enough to prove the operator-facing CLI
contract, add one end-to-end assertion under `crates/weaver-e2e/tests/` that
invokes `weaver act refactor` without arguments and checks the rendered
response.

### Stage B: introduce one canonical requirements model

Add a small helper that defines the `act refactor` operator contract in one
place. It should expose:

- the required flag set and their display strings;
- the valid provider names currently registered by the default runtime;
- the valid user-facing refactoring names currently supported by the daemon.

Keep this helper close to the `act refactor` module so it remains private to
the feature. If file-size pressure appears, extract a new helper module such as
`requirements.rs` rather than expanding `arguments.rs` or `mod.rs` indefinitely.

This helper should also own the formatting of the comprehensive
missing-arguments message so tests can assert a stable contract without
duplicating string construction logic.

### Stage C: enforce the contract in parsing and routing

Refactor argument parsing so it collects missing required flags first and emits
one comprehensive `DispatchError::InvalidArguments` message after parsing
completes.

If approval confirms that `--provider` is now required, update `RefactorArgs`
and the parsing pipeline accordingly. Then adjust the downstream routing code
and tests so successful paths supply an explicit provider while still using the
existing capability-resolution logic to validate provider, language, and
capability compatibility.

Keep unsupported-value handling deterministic. Unsupported providers or
refactorings should still fail clearly, but they should not mask the new
missing-arguments contract when required flags are absent.

### Stage D: update design and user documentation

Update `docs/weaver-design.md` anywhere it currently implies that provider
selection is automatic or that the MVP supports refactorings beyond the
implemented surface. The design doc should explicitly record the approved
operator contract and why the valid values come from shared runtime metadata.

Update `docs/users-guide.md` so the syntax line, parameter tables, examples,
and failure guidance match the new behaviour. The guide should tell operators
exactly which flags are required, which provider names are valid today, and
which refactorings the MVP supports today.

After the feature ships and all gates pass, mark roadmap item `2.3.4` as done
in `docs/roadmap.md`.

### Stage E: validate end to end

Run the documentation validators first because this task modifies Markdown:

```sh
set -o pipefail
make fmt 2>&1 | tee /tmp/2-3-4-fmt.log
```

```sh
set -o pipefail
make markdownlint 2>&1 | tee /tmp/2-3-4-markdownlint.log
```

```sh
set -o pipefail
make nixie 2>&1 | tee /tmp/2-3-4-nixie.log
```

Then run the Rust quality gates:

```sh
set -o pipefail
make check-fmt 2>&1 | tee /tmp/2-3-4-check-fmt.log
```

```sh
set -o pipefail
make lint 2>&1 | tee /tmp/2-3-4-lint.log
```

```sh
set -o pipefail
make test 2>&1 | tee /tmp/2-3-4-test.log
```

Review the captured logs if any command fails. Do not mark the roadmap item
done until every command exits successfully.
