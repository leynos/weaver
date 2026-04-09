# 2.3.1 Validate domains client-side before daemon startup

This ExecPlan (execution plan) is a living document. The sections
`Constraints`, `Tolerances`, `Risks`, `Progress`, `Surprises & Discoveries`,
`Decision Log`, and `Outcomes & Retrospective` must be kept up to date as work
proceeds.

Status: DONE

This document must be maintained in accordance with `AGENTS.md` at the
repository root.

## Purpose / big picture

Today, `weaver` still treats unknown domains inconsistently.

- `weaver unknown-domain` already fails on the client side, but it prints a
  built-in catalogue of domain-operation pairs rather than the roadmap's
  required valid-domain list.
- `weaver unknown-domain anything` does not fail client-side at all. It loads
  configuration and can attempt daemon auto-start before the daemon rejects the
  request.

Roadmap item `2.3.1` requires a tighter contract.[^roadmap] After this change,
any invocation whose first positional token is not one of the three valid
domains must fail in the CLI before configuration loading, daemon connection,
or daemon auto-start. The error body must always list the valid domains
`observe`, `act`, and `verify`. When the invalid domain is within edit distance
2 of exactly one valid domain, the output must also include one deterministic
`did you mean` suggestion.[^level3] This closes Level 3 and the Level 10b
error-guidance gap.[^level10]

Observable outcome after implementation:

```plaintext
$ weaver obsrve get-definition --uri file:///tmp/x.rs --position 1:1
error: unknown domain 'obsrve'

Valid domains: observe, act, verify
Did you mean 'observe'?
```

And:

```plaintext
$ weaver bogus get-definition --uri file:///tmp/x.rs --position 1:1
error: unknown domain 'bogus'

Valid domains: observe, act, verify
```

Both commands exit non-zero and do not print `Waiting for daemon start...`.

## Constraints

- Run `make check-fmt`, `make lint`, and `make test` before considering the
  feature complete.
- Because the implementation also updates Markdown, run `make fmt`,
  `make markdownlint`, and `make nixie` before finishing.
- Add both unit coverage and behavioural coverage using `rstest-bdd` v0.5.0
  for happy paths, unhappy paths, and edge cases.
- Keep the CLI flat. Do not restructure the command surface into nested clap
  subcommands as part of this task.
- Keep validation client-side. Unknown domains must fail before configuration
  discovery, daemon socket connection, or daemon auto-start.
- Preserve the existing known-domain missing-operation behaviour from roadmap
  `2.2.4`. `weaver observe` should still emit the per-domain operation list.
- Replace the current unknown-domain guidance contract for domain-only
  invocations. The roadmap requires a valid-domain list, not a catalogue of
  domain-operation pairs.
- The valid-domain list must contain all three canonical domains:
  `observe`, `act`, and `verify`.
- Emit at most one suggestion line, and only when the best edit distance is 2
  or less.
- Do not add new third-party dependencies for edit-distance matching.
- Keep files under 400 lines. Split helpers or tests into dedicated modules
  before violating the limit.
- Update `docs/weaver-design.md` with the final validation policy and the
  suggestion rule.
- Update `docs/users-guide.md` so operators see the new unknown-domain
  behaviour.
- Mark roadmap item `2.3.1` done only after the implementation, documentation
  updates, and all gates pass.
- Comments and documentation must use en-GB-oxendict spelling.

## Tolerances (exception triggers)

- Scope: if implementation grows beyond 12 touched files or roughly 250 net
  lines, stop and escalate.
- Interfaces: if meeting the acceptance criteria requires a public API change
  outside `weaver-cli`, stop and escalate.
- Dependencies: if the suggestion logic seems to require an external crate,
  stop and escalate rather than adding one.
- Help model: if satisfying the acceptance criteria would require changing the
  `weaver <domain>` and `weaver <domain> <operation>` help contract beyond this
  roadmap item, stop and escalate.
- Ambiguity: if a tie between multiple valid domains at the same minimum edit
  distance cannot be resolved deterministically without product input, stop and
  escalate.
- File size: if `crates/weaver-cli/src/lib.rs`,
  `crates/weaver-cli/src/discoverability.rs`, or
  `crates/weaver-cli/src/tests/behaviour.rs` would exceed 400 lines, extract a
  helper module before adding more logic.

## Risks

- Risk: there is already an unknown-domain preflight path, but it only runs for
  `weaver <unknown-domain>` and currently prints `Available operations:` plus
  every built-in domain-operation pair. Mitigation: replace this path with the
  roadmap's valid-domain contract and extend it to the
  `weaver <unknown-domain> <operation>` shape as part of the same change.

- Risk: the current preflight sentinel `AppError::MissingOperationGuidance`
  describes only one of the paths it now covers. Mitigation: consider renaming
  the sentinel to something broader such as `PreflightGuidance` if that keeps
  the runtime flow clearer without widening public surface area.

- Risk: localized messages interpolate variables through Fluent, which can
  inject bidi isolate markers into rendered output. Mitigation: keep using the
  existing `strip_bidi_isolates(...)` sanitation path for new suggestion and
  valid-domain messages.

- Risk: the source of truth for valid domains currently exists in more than one
  place: `KnownDomain`, `DOMAIN_OPERATIONS`, and the daemon router's `Domain`
  enum. Mitigation: this task should choose one authoritative CLI-side source
  for user guidance and add regression tests so the three-domain set does not
  drift.

- Risk: edit-distance helpers can attract accidental complexity and Clippy
  warnings if written imperatively. Mitigation: keep the helper tiny, pure, and
  directly unit-tested, with an early-exit threshold at distance 2.

## Progress

- [x] (2026-03-22 00:00Z) Read `docs/roadmap.md`, `docs/ui-gap-analysis.md`,
  `docs/weaver-design.md`, the relevant testing guides, and the prior `2.2.4`
  ExecPlan.
- [x] (2026-03-22 00:00Z) Confirmed the current runtime path in
  `crates/weaver-cli/src/lib.rs`: `handle_preflight(...)` already runs before
  config loading, but only emits unknown-domain guidance when the operator
  provides no operation.
- [x] (2026-03-22 00:00Z) Confirmed the current implementation gap in
  `crates/weaver-cli/src/discoverability.rs`:
  `write_unknown_domain_guidance(...)` prints a full domain-operation catalogue
  and offers no edit-distance suggestion.
- [x] (2026-03-22 00:00Z) Confirmed the current documentation drift in
  `docs/users-guide.md`: unknown domains are still documented as printing the
  built-in domain-operation catalogue.
- [x] (2026-03-22 00:00Z) Drafted this ExecPlan in
  `docs/execplans/2-3-1-validate-domains-client-side-before-daemon-startup.md`.
- [x] (2026-03-22 14:00Z) Stage A: added unit, integration, and BDD coverage
  for unknown domains with and without operations, plus typo suggestion and
  no-suggestion cases.
- [x] (2026-03-22 14:05Z) Stage B: broadened CLI preflight validation to all
  unknown domains before config loading or daemon startup, replaced the
  unknown-domain output contract with the valid-domain list, added bounded
  edit-distance suggestions, and renamed the sentinel to `PreflightGuidance`.
- [x] (2026-03-22 14:10Z) Stage C: updated `docs/weaver-design.md`,
  `docs/users-guide.md`, and `docs/roadmap.md` to reflect the shipped behaviour.
- [x] (2026-03-22 14:20Z) Stage D: ran `make fmt`, `make markdownlint`,
  `make nixie`, `make check-fmt`, `make lint`, and `make test` with logged
  output.

## Surprises & Discoveries

- `crates/weaver-cli/src/discoverability.rs` already contains
  `write_unknown_domain_guidance(...)`. This is not greenfield work; it is a
  contract correction and scope extension.

- `handle_preflight(...)` in `crates/weaver-cli/src/lib.rs` only enters the
  unknown-domain branch when `cli.operation` is missing or blank. That means
  the roadmap's main Level 3 case, `weaver bogus something`, still reaches the
  daemon path today.

- The current unknown-domain output is stronger than the old daemon error, but
  it still does not satisfy the roadmap. It prints `Available operations:` and
  lists command pairs such as `observe get-definition`, which is different from
  the required `Valid domains: observe, act, verify`.

- `KnownDomain::try_parse(...)` already performs case-insensitive parsing, so
  this task must preserve case-insensitive acceptance of the three valid
  domains while only rejecting truly unknown values.

- The workspace already pins `rstest-bdd` and `rstest-bdd-macros` at `0.5.0`,
  so no dependency update is required.

## Decision Log

- Decision: treat all unknown-domain invocations the same way regardless of
  whether an operation token is present. Rationale: the roadmap frames domain
  validation as a client-side pre-daemon concern, and operators should not see
  different unknown-domain contracts based solely on the presence of an
  operation. Date: 2026-03-22.

- Decision: keep the existing known-domain missing-operation flow and layer the
  new unknown-domain validation beside it instead of folding both into one
  message path. Rationale: roadmap `2.2.4` and `2.3.1` have different operator
  outcomes and acceptance criteria. Date: 2026-03-22.

- Decision: implement the edit-distance helper in `weaver-cli` with no new
  dependency. Rationale: the domain set is fixed at three values, the threshold
  is small, and a small pure helper is easier to audit and test than a new
  crate. Date: 2026-03-22.

- Decision: when exactly one valid domain is within distance 2, emit one
  suggestion. When none qualify, emit no suggestion. When multiple domains tie
  at the same best distance within the threshold, prefer catalogue order only
  if that rule is explicitly documented in `docs/weaver-design.md`; otherwise
  stop and escalate under the ambiguity tolerance. Rationale: the roadmap
  requires a single suggestion, so any tie-break policy must be explicit and
  reviewable. Date: 2026-03-22.

- Decision: update the Fluent catalogue rather than hard-coding new English
  strings directly in Rust. Rationale: the CLI already routes discoverability
  copy through `ortho_config::Localizer`, and this task should preserve that
  pattern. Date: 2026-03-22.

## Outcomes & Retrospective

Target outcome at completion:

1. `weaver <unknown-domain>` fails client-side with the canonical valid-domain
   list and no daemon interaction.
2. `weaver <unknown-domain> <operation>` also fails client-side before config
   load or daemon auto-start.
3. A close typo such as `obsrve` emits exactly one `Did you mean 'observe'?`
   line.
4. A distant invalid domain emits no suggestion line.
5. Known-domain missing-operation guidance still behaves as delivered in
   roadmap `2.2.4`.
6. Unit tests, integration tests, and `rstest-bdd` scenarios cover happy,
   unhappy, and edge cases.
7. `docs/weaver-design.md`, `docs/users-guide.md`, and `docs/roadmap.md`
   reflect the final behaviour.
8. `make check-fmt`, `make lint`, and `make test` pass, along with the
   Markdown gates required by `AGENTS.md`.

Retrospective notes:

- Reusing the `2.2.4` preflight seam kept the change local to `weaver-cli`
  and avoided any daemon or config-surface edits.
- Renaming the sentinel from `MissingOperationGuidance` to
  `PreflightGuidance` clarified the now-shared early-exit path and reduced
  future maintenance ambiguity.
- A tiny bounded Levenshtein helper was sufficient for the fixed three-domain
  set; no external dependency or broader fuzzy-matching framework was needed.

## Context and orientation

The implementation surface is concentrated in `weaver-cli`.

- `crates/weaver-cli/src/lib.rs`
  Owns `CliRunner::run_with_handler(...)` and `handle_preflight(...)`. This is
  where the CLI currently decides whether to exit before config loading.
- `crates/weaver-cli/src/discoverability.rs`
  Owns `KnownDomain`, `DOMAIN_OPERATIONS`,
  `write_missing_operation_guidance(...)`,
  `write_unknown_domain_guidance(...)`, and the current
  `should_emit_domain_guidance(...)` predicate.
- `crates/weaver-cli/src/errors.rs`
  Owns the current sentinel `AppError::MissingOperationGuidance`.
- `crates/weaver-cli/locales/en-US/messages.ftl`
  Holds the localized message IDs for the discoverability and guidance paths.
- `crates/weaver-cli/src/tests/unit/missing_operation_guidance.rs`
  Already verifies that preflight guidance avoids config loading by using a
  panicking loader.
- `crates/weaver-cli/src/tests/behaviour.rs` and
  `crates/weaver-cli/tests/features/weaver_cli.feature` Provide the
  `rstest-bdd` coverage for operator-visible CLI flows.
- `crates/weaver-cli/tests/main_entry.rs`
  Verifies binary-level behaviour with `assert_cmd`.
- `crates/weaverd/src/dispatch/router.rs`
  Remains the daemon-side backstop and still owns the daemon's own domain set,
  but the roadmap item here is explicitly about the CLI rejecting invalid
  domains before reaching that layer.

Current behaviour to replace:

```plaintext
$ weaver unknown-domain
error: unknown domain 'unknown-domain'

Available operations:
  observe get-definition
  observe find-references
  ...
```

Current behaviour still missing:

```plaintext
$ weaver unknown-domain get-definition
Waiting for daemon start...
failed to spawn weaverd binary '"weaverd"': No such file or directory
```

The finished feature must remove both mismatches.

## Plan of work

### Stage A: lock the desired contract in tests first (red phase)

Add failing tests before editing the runtime code.

Unit coverage in
`crates/weaver-cli/src/tests/unit/missing_operation_guidance.rs` should cover:

- Unknown domain without operation emits `error: unknown domain '<value>'`.
- Unknown domain without operation emits `Valid domains: observe, act, verify`.
- Unknown domain without operation does not emit `Available operations:` or any
  domain-operation pairs.
- Unknown domain with an operation still fails before configuration loading.
  Use the existing panicking-loader pattern so the red test proves there is no
  config discovery.
- Close typo cases produce exactly one suggestion line.
- Non-close typo cases produce no suggestion line.
- Edge cases for the pure distance/suggestion helper:
  trimming, case-insensitive comparison, exact match, threshold `2`, and
  deterministic tie handling if a tie-break rule is adopted.

Integration coverage in `crates/weaver-cli/tests/main_entry.rs` should assert
the same operator-visible contract for the compiled binary, especially:

- `weaver obsrve get-definition` fails without printing
  `Waiting for daemon start...`.
- `stderr` includes the valid-domain list.
- `stderr` includes exactly one suggestion for `obsrve`.

BDD coverage in `crates/weaver-cli/tests/features/weaver_cli.feature` and
`crates/weaver-cli/src/tests/behaviour.rs` should add or replace scenarios for:

- Unknown domain without operation.
- Unknown domain with operation.
- Close typo with suggestion.
- Distant typo without suggestion.

Each scenario must finish with `And no daemon command was sent`.

Go/no-go:

- Do not proceed until at least one new test fails because the old
  implementation still prints `Available operations:` or still attempts the
  daemon path for `weaver <unknown-domain> <operation>`.

### Stage B: implement shared preflight validation and suggestion logic (green phase)

Refactor the discoverability layer so the CLI can answer one question before
config loading: "is the supplied domain valid, and if not, what guidance should
the operator see?"

Implementation outline:

1. Introduce a small pure helper in `crates/weaver-cli/src/discoverability.rs`
   or a sibling helper module that:
   - normalizes the raw domain token,
   - recognizes the three valid domains,
   - calculates the best edit-distance match within threshold 2, and
   - returns a structured result the writer can render.
2. Broaden the preflight predicate so it covers both:
   - known domain with missing operation, and
   - unknown domain with or without an operation.
3. Keep `write_missing_operation_guidance(...)` for the known-domain path.
4. Replace `write_unknown_domain_guidance(...)` output so it writes:
   - the unknown-domain error line,
   - a blank line,
   - `Valid domains: observe, act, verify`,
   - an optional `Did you mean '<domain>'?` line, and
   - no operation catalogue.
5. Update `handle_preflight(...)` so an unknown-domain result returns the
   preflight sentinel before config loading or daemon startup.
6. If the sentinel name is now misleading, rename it and update the narrow
   error-to-exit-code mapping in `CliRunner::map_result_to_exit_code(...)`.
7. Update `crates/weaver-cli/locales/en-US/messages.ftl` with the new message
   IDs and fallbacks for:
   - valid-domain list heading/body,
   - optional suggestion line,
   - any renamed unknown-domain guidance strings.

Keep the implementation idempotent and low-risk:

- Use the existing `strip_bidi_isolates(...)` helper on any localized string
  with interpolated variables.
- Do not touch daemon routing in this roadmap item.
- Do not broaden this task into unknown-operation suggestions; that is roadmap
  `2.3.2`.

Go/no-go:

- Do not proceed until the targeted `weaver-cli` unit, integration, and BDD
  tests pass locally.

### Stage C: document the contract and mark the roadmap item complete

Update the design and operator docs once the code and tests are stable.

- `docs/weaver-design.md`
  Document that unknown-domain validation now happens client-side before config
  load and daemon startup, and record the suggestion rule: distance threshold
  2, single suggestion only, and the final tie behaviour.
- `docs/users-guide.md`
  Replace the current statement that unknown domains print the built-in
  domain-operation catalogue. Include one concrete example showing the new
  `Valid domains:` line and, if retained, the `Did you mean` hint.
- `docs/roadmap.md`
  Mark item `2.3.1` complete only after the implementation and all gates pass.

Go/no-go:

- Do not mark the roadmap item complete before the final quality gates succeed.

### Stage D: run the full gate sequence and capture evidence

Run the required gates with `tee` and `set -o pipefail` so failures are
observable despite truncated terminal output:

```sh
set -o pipefail; make fmt 2>&1 | tee /tmp/2-3-1-make-fmt.log
set -o pipefail; make markdownlint 2>&1 | tee /tmp/2-3-1-make-markdownlint.log
set -o pipefail; make nixie 2>&1 | tee /tmp/2-3-1-make-nixie.log
set -o pipefail; make check-fmt 2>&1 | tee /tmp/2-3-1-make-check-fmt.log
set -o pipefail; make lint 2>&1 | tee /tmp/2-3-1-make-lint.log
set -o pipefail; make test 2>&1 | tee /tmp/2-3-1-make-test.log
```

Expected evidence:

- All six commands exit `0`.
- `make test` output shows the new `weaver-cli` unit, integration, and BDD
  cases passing.
- No test output includes `Waiting for daemon start...` for the new
  unknown-domain scenarios.

## References

\[^level3\]:
[docs/ui-gap-analysis.md Level 3](../ui-gap-analysis.md#level-3--unknown-domain-weaver-bogus-something)
 \[^level10\]:
[docs/ui-gap-analysis.md Level 10](../ui-gap-analysis.md#level-10--error-messages-and-exit-codes)

[^roadmap]: %5Bdocs/roadmap.md%5D(../roadmap.md)
