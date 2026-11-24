# 12.3.1 Ratify the multi-workspace daemon contracts

This ExecPlan (execution plan) is a living document. The sections `Constraints`,
`Tolerances`, `Risks`, `Progress`, `Surprises & discoveries`, `Decision log`,
`Outcomes & retrospective`, `Conformance basis`, and `Verification plan` must
be kept up to date as work proceeds.

Status: DRAFT (awaiting design review)

Branch: `12-3-1-ratify-multi-workspace-daemon-contracts`

Roadmap item: `12.3.1` in `docs/roadmap.md`.

## Purpose / big picture

Weaver is a command-line tool (`weaver`) backed by a background daemon
(`weaverd`) that answers questions about source code and applies verified
edits. By default one daemon serves one user. It remembers the directory it was
started in and treats that as the only repository, so this sequence goes wrong
today:

```sh
cd ~/work/alpha && weaver daemon start
cd ~/work/beta && weaver act apply-patch < change.patch   # edits alpha
```

Four documents propose the cure: RFC 0002 and ADRs 0008, 0009, and 0010. They
are all marked `Proposed`, and they do not yet agree with each other. Roadmap
items 12.3.2 to 12.3.6 will implement them, and 13.4.1, 14.1.1, 15.3.1, 15.3.4,
16.1.4, 16.1.5, 16.1.6, and 16.1.10 depend on the result.

This task settles the contract before that code is written. Afterwards:

- each of the four documents is `Accepted`, with no open question that a later
  task must guess at;
- every rule has one stable identifier, one owning document, one roadmap task,
  and one named form of evidence;
- an executable model of the rules shows that they can all hold together, and
  that each conflict found during planning really breaks the older wording; and
- a test fails when the documents stop agreeing about identifiers or status.

No daemon behaviour changes. The commands above still go wrong until 12.3.2 and
12.3.3 land. Success is observable by running:

```sh
cargo nextest run -p weaver-tenancy-model
```

Expected: the ratified policy passes every scenario, each superseded policy is
rejected with a named violation, and the document checks pass.

## Constraints

- C1. Implementation begins only after the user approves this plan.
- C2. No production behaviour changes. Do not modify `crates/weaverd`,
  `crates/weaver-cli`, `crates/weaver-config`, `crates/weaver-daemon-types`,
  `crates/weaver-lsp-host`, `crates/weaver-plugin*`, `crates/weaver-sandbox`,
  or their tests.
- C3. No production crate may depend on the new model crate. Later roadmap
  tasks may use it as a development dependency for conformance tests.
- C4. No new third-party dependency. `rstest`, `rstest-bdd`,
  `rstest-bdd-macros`, `proptest`, `googletest`, `pretty_assertions`,
  `cap-std`, and `camino` are already workspace dependencies and are authorized
  for the model crate.
- C5. `docs/documentation-style-guide.md` governs status lines, and requires an
  accepted ADR to have no outstanding decision. It is a shared document and is
  not edited here.
- C6. RFC 0002 remains proposal and rationale plus the normative clauses; ADRs
  hold decisions; `docs/weaver-design.md` describes the target design. A rule
  is stated normatively once and cited elsewhere.
- C7. The contract must not weaken RFC 0002's six invariants, ADR 0005's trust
  boundary, the Double-Lock rule (syntactic and semantic checks both pass
  before a commit), or roadmap 16.1.x stale-base rules. A narrower restatement
  is recorded as a decision.
- C8. Configuration clauses target OrthoConfig 0.11.0
  (`ConfigFilePolicy::project_root`, `EnvSource`). This task makes no
  dependency change: the workspace pins 0.10.0, and the open 13.2.1 plan (pull
  request #453) carries the upgrade. 12.3 adds no OrthoConfig command-contract
  consumer, so `docs/orthoconfig-consumer-boundary.toml` is unchanged.
- C9. `typos.toml` is generated. Completed ExecPlans and `docs/archive/` are
  historical and stay untouched.
- C10. Rust files stay within 400 lines. Clippy limits functions to four
  parameters, 70 lines, and four nesting levels, and denies indexing, integer
  division, shadowing, `unwrap`, `expect` outside tests, and undocumented
  private items. Whitaker limits cognitive complexity to nine.

## Tolerances (exception triggers)

Comparison base: the merge base with `origin/main`. Count net lines (additions
minus deletions) with `git diff --numstat "$(git merge-base origin/main HEAD)"`
and these pathspecs. ExecPlan files are excluded from every count, including
file counts. Each limit is enforced independently.

- Code: `crates/weaver-tenancy-model/**/*.rs` and `**/*.feature`.
- Configuration: `Cargo.toml`, `crates/weaver-tenancy-model/Cargo.toml`,
  `.config/**`, and `typos.local.toml`.
- Documentation: `docs/**/*.md` except `docs/execplans/**`.
- Generated, uncounted: `Cargo.lock` and `typos.toml`.

| Milestone | Code limit | Configuration limit | Documentation limit |
| --------- | ---------- | ------------------- | ------------------- |
| `EP-M0`   | 0          | 0                   | 0                   |
| `EP-M1`   | 0          | 5                   | 700                 |
| `EP-M2`   | 2,000      | 45                  | 60                  |
| `EP-M3`   | 0          | 5                   | 330                 |
| `EP-M4`   | 400        | 5                   | 90                  |
| Total     | 2,400      | 60                  | 1,180               |

_Table 1: Net-line limits._

Stop and escalate when any of these is reached:

- a limit in Table 1, or more than 40 non-ExecPlan files;
- a clause change that alters a resolution the approver accepted (D5 to D33);
- a contradiction that the model finds and no clause change removes within
  three attempts;
- any scenario exceeding 2,000,000 states, or the crate's tests exceeding 60
  seconds under `make test` once compiled;
- a need to edit production code, add a dependency, or renumber the roadmap;
- a gate that still fails after three fix attempts; or
- a milestone taking more than eight hours of active work, excluding compile
  queueing.

`Budget evidence` explains the numbers.

## Risks

- R1. The model proves something easier than the contract. Severity: high.
  Likelihood: medium. Mitigation: every model invariant cites clauses; every
  clause marked `model` must be cited; seeded faults must be rejected for the
  stated reason; and the model's limits are listed, not implied.
- R2. The contract is over-specified and 12.3.2 to 12.3.6 cannot follow it.
  Severity: high. Likelihood: medium. Mitigation: clauses state observable
  obligations, not types; names in RFC 0002 §6.2 stay provisional; numeric
  defaults stay with the implementing task (D31).
- R3. The model and production code drift apart. Severity: medium. Likelihood:
  high over time. Mitigation: 12.3.3 and 12.3.5 success text gains a
  conformance obligation; the reuse policy is in `docs/developers-guide.md`.
- R4. Exhaustive exploration grows too large. Severity: medium. Likelihood:
  medium; the Python probe covered single environment events in at most 28,968
  states, but did not finish the all-events scenario in 110 seconds.
  Mitigation: scenario tables with bounded environment budgets, a Rust
  implementation, ticket normalization, and the state ceiling above.
- R5. A finite model cannot show starvation. Severity: medium. Likelihood:
  certain; the probe passed a barging policy. Mitigation: fairness is checked
  as an explicit bypass invariant (`INV-FIFO`), not as reachability.
- R6. Pull request #260 allocates ADR 0014 for endpoint-scoped artefacts.
  Severity: low. Likelihood: medium. Mitigation: this task allocates no ADR
  number, and D30 words the dependency so that either merge order works.
- R7. The host compile queue is long. Severity: low. Likelihood: high.
  Mitigation: start each gate once and wait.
- R8. The wider plan is too heavy for a documentation item. Severity: medium.
  Likelihood: medium. Mitigation: question 5 lets the approver drop `EP-M2`.

## Progress

- [x] (2026-11-24) Branch renamed; Lody session titled.
- [x] (2026-11-24) Reconnaissance by a four-agent Wyvern team.
- [x] (2026-11-24) Prior-art research, `rustup` probes, and the throwaway
  Python feasibility probe.
- [x] (2026-11-24) Initial draft.
- [ ] Design review applied.
- [ ] Plan approved by the user.
- [ ] `EP-M0` baseline and drift check.
- [ ] `EP-M1` clauses written into RFC 0002 and ADRs 0008 to 0010.
- [ ] `EP-M2` executable contract model.
- [ ] `EP-M3` dependent documents reconciled.
- [ ] `EP-M4` document gate, acceptance, and roadmap closure.

## Surprises & discoveries

- Observation: `rustup run <toolchain> rust-analyzer`, the launch RFC 0002 §6.4
  prefers, silently runs a different `rust-analyzer` when the component is
  missing. Evidence: rustup 1.29.3 with toolchain 1.82.0 and a decoy on `PATH`
  printed `falling back to ".../rust-analyzer"`, ran the decoy, and exited 0.
  `rustup which --toolchain 1.82.0 rust-analyzer` refused. Impact: D15.
- Observation: rustup commands can download toolchains. Evidence: while
  planning, `rustup toolchain list` in a directory whose `rust-toolchain.toml`
  named `nightly-2019-01-01` installed that toolchain. It was removed. With
  `RUSTUP_AUTO_INSTALL=0`, resolution refused instead. Impact: D15.
- Observation: an ambient `RUSTUP_TOOLCHAIN` outranks the repository's
  toolchain file. Evidence: `rustup show active-toolchain` reported "overridden
  by environment variable". Language servers inherit the daemon's whole
  environment today. Impact: D15.
- Observation: RFC 0002 §6.1 says lock, PID, and health files are scoped to the
  endpoint. They are not at `fe56d226`
  (`crates/weaver-config/src/runtime.rs:84`); open pull request #260 does that
  work. Impact: D30.
- Observation: an older daemon silently ignores unknown request fields
  (`crates/weaverd/src/dispatch/request.rs:16`), so adding a `workspace` field
  would let a new client edit the wrong repository. Impact: D8.
- Observation: a language whose server times out or exits stays dead until the
  daemon restarts (`crates/weaver-lsp-host/src/host/mod.rs:367`). Impact:
  context for `LS` clauses.
- Observation: the sandbox refuses to start a plugin from a process with more
  than one thread (`crates/weaver-sandbox/src/sandbox.rs:99`), and the daemon
  is multi-threaded. Evidence: by reading only. Impact: `EP-M0` checks it and
  raises an issue if it holds; it does not change this contract.
- Observation: roadmap 19.3.1 owns Verus and Kani installation and requires
  16.1.1, which comes after every 12.3 task. Impact: D4, question 6.

## Decision log

D1 to D4 and D34 concern method. D5 to D33 resolve numbered conflicts (`X1` to
`X29`) in `Contract conflicts and proposed resolutions`; approving this plan
approves them. All are dated 2026-11-24 and were proposed by the planning agent.

- D1. Decision: revise first, accept last, in one pull request. Documents stay
  `Proposed` until `EP-M4`. Rationale: an accepted document that still
  contradicts a sibling would mislead 12.3.2. Accepting as they stand was
  rejected because the style guide forbids outstanding decisions and the
  conflicts below are real.
- D2. Decision: RFC 0002 gains a normative clause section with identifiers such
  as `WS-1`; ADRs and the design document cite them. Rationale: the same rule
  is currently worded differently in up to three places. A separate contract
  document, or a TOML manifest like the OrthoConfig boundary, was rejected: it
  would be a fifth place to keep aligned.
- D3. Decision: add a private crate, `crates/weaver-tenancy-model`, holding an
  executable specification and its tests. Rationale: the conflicts are about
  interleavings (leases, drains, budgets, cancellation), which reading finds
  poorly and exhaustive exploration finds in milliseconds. Alternatives: review
  only (no evidence that the rules hold together); TLA+, Alloy, or Quint (new
  toolchain, unreadable to Rust reviewers); `stateright` (unauthorized
  dependency); Kani (not configured, and the model is already finite); placing
  it in `weaver-docs-gate` (wrong responsibility) or in `weaverd` tests (later
  conformance tests could not share it cleanly).
- D4. Decision: no Verus or Kani milestone here. Named theorem statements go
  into RFC 0002 and are assigned to the tasks that create the kernels. See
  `Formal verification decision`.
- D34. Decision: use the status form `Accepted, YYYY-MM-DD. <summary>` for the
  RFC as well as the ADRs. Rationale: the style guide defines the ADR form and
  lets "a later process" promote an RFC; roadmap 12.3.1 plus approval of this
  plan is that process. ADR 0013 is the only accepted precedent.

## Contract conflicts and proposed resolutions

Each entry gives the conflict, the resolution, and the clauses that will carry
it. "Today" refers to `fe56d226`. Terms: a *workspace* is one repository root
served by the daemon; a *lease* is a counted right to use something; an
*exclusive lease* is held by one mutation at a time.

### Workspace identity

- X1 (D5). RFC §6.1 derives the locator from the invocation directory and says
  the same repository gains no duplicate state, but `repo` and `repo/src` are
  different canonical paths. Resolution (`WS-1`, `WS-2`): the locator names a
  starting directory. The daemon canonicalizes it, then walks up to the nearest
  directory containing a `.git` entry (directory or file), stopping at a
  filesystem boundary. That directory is the root. With no marker the starting
  directory is the root. A linked worktree or submodule is therefore its own
  workspace, and a nested repository is the innermost. An explicit
  `--workspace` option changes only the starting directory, so no override can
  create an overlapping root inside a repository. Prior art: Git discovery, and
  Watchman `watch-project`, which resolves the real path and then walks up to a
  marker.
- X2 (D6). Canonical paths do not identify bind mounts or a directory replaced
  in place. Resolution (`WS-3`): the key remains the canonical UTF-8 root.
  State also records an opaque `RootIdentity` (device and file identifier where
  the platform has them). If an active workspace already has that identity
  under another path, refuse with `workspace_alias`; if the same path has a new
  identity, retire the old state first. Prior art: `lspmux` compares roots only
  by device and inode.
- X3 (D7). Two workspaces may mutate concurrently, yet nested roots share files.
  Resolution (`WS-4`, `MU-9`): a workspace's *domain* is every path whose
  nearest root is that workspace. Domains are disjoint. A mutation target must
  be in the domain when the exclusive lease is held and again at commit. Two
  markerless roots where one contains the other cannot be active together; the
  second is refused with `workspace_overlap`.
- X4 (D8). "Missing or incompatible fields fail validation" protects a new
  daemon, not an old one, which ignores unknown keys and would use its startup
  root. Resolution (`WS-5`): the new envelope nests the command under a new key
  with a protocol identifier, so an old daemon fails to parse it. The client
  treats that failure as "restart the daemon", never as a retry.
- X5 (D9). RFC §6.1 rejects a locator that "escapes an allowed capability root",
  but no such root is defined; and on non-Unix systems the default endpoint is
  loopback TCP, which is not per-user. Resolution (`WS-6`, `CF-3`): on Unix
  sockets a root must be owned by the daemon's effective user, which also
  prevents a marker planted in a shared parent from capturing a child. On TCP
  endpoints only roots in `allowed_workspace_roots` are served, and an empty
  list serves none.
- X6 (D10). RFC §6.9 wants opaque identifiers; RFC 0003 and ADR 0012 put
  `workspace_id` in durable selector records; the registry key is a path.
  Resolution (`WS-7`, `OB-2`): `workspace_id` is a versioned, domain-separated
  digest of the canonical root, stable across daemon restarts. Telemetry uses a
  separate process-local `ordinal.generation` tag that reveals nothing.
- X7 (D11). `get-card`, `graph-slice`, and `get-definition` accept any absolute
  `file://` URI, so a request can read outside its workspace. Resolution
  (`WS-8`): request targets must be in the domain or are refused with
  `path_outside_workspace`; results may point outside it and are marked so.
  12.3.2 implements this.

### Workspace lifecycle

- X8 (D12). ADR 0008 routes on an `Arc<WorkspaceState>` obtained before eviction
  can see it; an `Arc` keeps memory alive but does not prevent retirement.
  Resolution (`LC-1`, `LC-2`): lookup and lease counting happen in one registry
  critical section. Idle eviction requires a zero count in that same section.
- X9 (D13). RFC §6.1 treats "already being retired" as a bad locator, while
  §6.2 makes creation single-flight, and the state list omits failure, refusal,
  and re-creation. Resolution (`LC-3` to `LC-6`): the states and transitions in
  `Abstract model` are normative. A key has at most one live generation. A
  draining workspace refuses new work with the retryable `workspace_draining`.
  Creation belongs to the daemon, not to the first client, so that client's
  disconnection does not fail the other waiters; it has a deadline, and failure
  publishes nothing.
- X10 (D14). RFC §6.8 makes open documents block eviction, which can retain a
  workspace forever, against §4.1. Resolution (`LC-7`): only leases and staged
  content block eviction. No document stays open after the lease that opened
  it. Today only the semantic lock opens documents, and it closes them.

### Language-server lifecycle

- X11 (D15). RFC §6.4 resolves toolchains from the workspace, preserves rustup
  precedence, launches with `rustup run`, and promises no silent fallback.
  These conflict as shown in `Surprises & discoveries`. Resolution (`LS-1` to
  `LS-3`): resolver and child start from an empty environment plus a declared
  allowlist; `RUSTUP_TOOLCHAIN` and `RUSTUP_TOOLCHAIN_SOURCE` are never
  inherited; `RUSTUP_AUTO_INSTALL=0` is set; the executable is the absolute
  path printed by `rustup which --toolchain <toolchain> rust-analyzer`; the
  child receives `RUSTUP_TOOLCHAIN=<toolchain>` explicitly. A missing toolchain
  or component is `unavailable`. An intentional override is workspace
  configuration.
- X12 (D16). "Target workspace", "adapter-selected process root", and design
  §3.1.1's "does not rely on the target workspace" disagree; §6.5 allows one
  workspace folder while the fingerprint has an ordered list. Resolution
  (`LS-4`, `LS-5`): a *project root* is a directory in the workspace's domain
  chosen by the adapter, by default the workspace root. The working directory,
  `rootUri`, and exactly one `workspaceFolders` entry all equal it. `rootUri`
  is deprecated in LSP 3.17 and kept for older servers.
- X13 (D17). "Only the current identity may receive new leases" assumes one
  server per language, but nested project roots need several. Resolution
  (`LS-6`): currency is per *slot*, meaning language plus project root.
- X14 (D18). Invariant 4 says a server is reused only while its identity
  matches, but §6.4 allows bounded staleness, and §6.4 relies on watched files
  although nothing watches files. Resolution (`LS-7`): inputs inside the
  workspace are re-read when a lease is granted; inputs outside it may be at
  most `identity_freshness` old. No file watcher is assumed. A lease pins the
  identity observed at grant.
- X15 (D19). The server states omit failed start, backoff, an open circuit, and
  draining without ill health. Resolution (`LS-8`): the table in
  `Abstract model` is normative. A timed-out or exited server is stopped and
  may be restarted; it does not disable the language.
- X16 (D20). Plugins start processes outside the pool: the rope plugin starts
  `python3`, and the rust-analyzer plugin starts its own server in a temporary
  stub crate, both from the daemon's directory and environment. Resolution
  (`LS-9`, `CF-4`): these are *auxiliary processes*. They are never reused,
  have their own budget, run under a workspace lease, and receive an explicit
  working directory, environment, and execution identity. Relative plugin paths
  resolve against the configuration file that named them.

### Mutation isolation

- X17 (D21). RFC invariant 5 requires a matching revision, but a revision read
  under an exclusive lease cannot change, and it does not see external edits.
  Resolution (`MU-1`, `MU-2`): content digests are the only commit guard. The
  revision is an opaque token of generation and counter, used to invalidate
  caches. It is advisory in selectors and never alone makes one stale.
- X18 (D22). The lease is taken "before capturing the baseline", but plugin
  planning can take 60 seconds and needs file content first. Upgrading a shared
  lease to exclusive deadlocks two planners. Resolution (`MU-3`): planning
  records digests under a shared lease and releases it; the mutation then takes
  the exclusive lease and recomputes the digests. A request acquires workspace
  coordination at most once at a time and never upgrades.
- X19 (D23). A server whose identity changes is drained and replaced, but the
  mutation holding it is mid-verification, and may be editing the very file
  that changed the identity. Resolution (`MU-4`, `MU-5`): a mutation acquires
  its whole server set before staging and pins it. If an identity changes or a
  server is lost before commit, the mutation aborts with no effect
  (`execution_identity_changed` or `server_lost`). If its own plan touches an
  identity input, semantic verification is `inconclusive` under policy.
- X20 (D24). The exclusive lease lasts until the shared document view is
  restored, but a hung server never confirms. Resolution (`MU-6`): restore or
  retire. If restoration fails or exceeds its deadline, the server is stopped
  before the lease is released. Either way no live server holds staged content.
- X21 (D25). Shutdown has a bounded deadline, yet a live commit is never
  abandoned. Resolution (`MU-7`): a commit section begins after both locks and
  the digest check pass. It contains only filesystem work, starts no process,
  waits for no server, and cannot be cancelled. Deadlines cancel everything
  before it. Forced termination during it is outside the guarantee and is
  recovered by 16.1.2.
- X22 (D26). Readers must observe one committed revision, but nothing tells a
  server that files changed. Resolution (`MU-8`): before the exclusive lease is
  released, every ready server in the workspace is sent
  `workspace/didChangeWatchedFiles` for the committed paths on the same ordered
  transport later requests use; a server that cannot be sent it is retired. LSP
  gives no acknowledgement, so the guarantee is ordering, not completion.
- X23 (D27). "Atomic commit" overstates per-file replacement with best-effort
  rollback. Resolution (`MU-10`): the documents say exactly that. 16.1.5 and
  16.1.6 own today's related gaps: deletions skip the semantic lock, and the
  baseline is read twice.
- X24 (D28). Separate sockets are an escape hatch, but two daemons serving one
  repository have independent leases. Resolution (`MU-11`): while holding the
  exclusive lease a daemon also holds an advisory file lock in the user's
  runtime directory, named from the `RootIdentity`; contention returns the
  retryable `mutation_elsewhere`. Digest checks remain the guard where advisory
  locks are unavailable.

### Bounded admission

- X25 (D29). Daemon, workspace, and server limits are separate, so one busy
  workspace can occupy every daemon slot, against "unrelated workspaces make
  progress". Waiting for a process-budget slot while holding another can
  deadlock. Resolution (`AD-1` to `AD-6`): configuration requires
  `workspace_max_in_flight < daemon_max_in_flight`; waiting counts as in
  flight; acquisition follows the rank order in `Abstract model`; budget slots
  for a request are reserved all at once, after evicting idle servers, or
  refused at once, never waited for; queues are first in, first out, and a
  waiting mutation blocks later readers; every wait has a deadline; and no
  lease or permit other than the connection permit is held while the response
  is written.
- X26 (D30). RFC §6.1 says endpoint-scoped artefacts exist. Resolution (`CF-5`):
  state it as a prerequisite of running separate sockets in one directory,
  owned by pull request #260, and say that it is not true until that merges.
- X27 (D31). Open questions ask for numeric defaults, while RFC §4.2 excludes
  them. Resolution (`CF-1`, `CF-2`): ratify each setting's name, unit, scope,
  and validity relation. The implementing task chooses and justifies the
  default: 12.3.3 for workspaces, 12.3.4 for servers, 12.3.5 for admission.
- X28 (D32). "Retryable", `unavailable`, validation failure, and refusal have
  four vocabularies, and "retry" also means a lock retry in design §4.3.6.
  Resolution (`RC-1` to `RC-4`): one envelope with `class`, `code`, `scope`,
  `retryable`, `effects` (`none`, `committed`, or `unknown`), and an optional
  bounded `retry_after_ms`. Only class `overloaded` is retryable, and it is
  produced only before a mutation stages anything, so retrying is always safe.
  Exit status stays 1 for `invalid_request` and `conflict`, and 2 otherwise.
- X29 (D33). RFC 0002 §7.2 and `AGENTS.md` ask for metrics; RFC 0001 forbids a
  metrics framework; roadmap 13.4.1 names events only after 12.3.5 emits them.
  Resolution (`OB-1`, `OB-3`): `tracing` events are required. Counters through
  the `metrics` facade are allowed, but no recorder or exporter is installed.
  Names used in 12.3 are provisional until 13.4.1. Workspace paths stay out of
  telemetry; an error may repeat the locator its own client sent. RFC 0001 is
  clarified to match.

Configuration scope (`CF-6`, part of D31): `Config` discovers a project file
from the daemon's startup directory, so the first repository's settings would
govern the others. Settings split into daemon scope, which reads no project
layer, and workspace scope, loaded per workspace with
`ConfigFilePolicy::project_root(<root>)` and an injected environment source.

Shadow semantic servers stay deferred (RFC question 7, ADR 0010): no roadmap
task needs one, and adopting them requires a new ADR.

## Outcomes & retrospective

Not started. Before completion, reconcile every implementation discovery with
the upstream artefacts in `Conformance basis`, and record whether the model
falsified any resolution.

## Context and orientation

Inspected revision: `fe56d226`.

### How the daemon works today

`weaverd` captures its directory in `crates/weaverd/src/process/launch.rs:148`
and passes it to `DispatchConnectionHandler::new`. Every request takes the one
mutex in `BackendManager::with_backends`
(`crates/weaverd/src/dispatch/backend_manager.rs:34`) for its whole duration,
including plugin runs. Issues #130 and #138 ask for that mutex to be narrowed.
It is also the only thing preventing readers from seeing staged documents, so
narrowing it before 12.3.5 would be unsafe; this plan does not close them.

`LspHost` (`crates/weaver-lsp-host/src/host/mod.rs:118`) keeps one session per
language. `ProcessLanguageServer` starts bare command names with the daemon's
directory and environment, and `initialize` sends no root
(`crates/weaver-lsp-host/src/adapter/trait_impl.rs:43`). One request is in
flight per server. The semantic lock
(`crates/weaverd/src/dispatch/act/apply_patch/semantic_lock.rs:154`) opens,
changes, checks, and closes each modified document. Commit is per-file rename
with best-effort rollback
(`crates/weaverd/src/safety_harness/transaction/commit.rs:81`). Nothing checks
a digest or revision.

The listener allows 128 handler threads and drops further connections silently
(`crates/weaverd/src/transport/listener.rs:308`). Handlers are detached, and
the shutdown deadline is logged but not enforced. Read timeouts are per read,
so a slow client can hold a slot indefinitely. `weaverd.health` is a file, so
status does not use the socket.

### Documents in scope

- `docs/rfcs/rfc-0002-multi-workspace-daemon-and-language-server-lifecycle.md`.
- `docs/adrs/adr-0008-workspace-scoped-daemon-tenancy.md`,
  `docs/adrs/adr-0009-workspace-scoped-language-server-lifecycle.md`, and
  `docs/adrs/adr-0010-workspace-local-concurrency.md`.
- `docs/weaver-design.md` §§2.1.8, 2.1.12, 3.1.1, 4.2.3, and the workspace
  manager text before §3.
- `docs/rfcs/rfc-0001-local-daemon-observability.md`, `docs/roadmap.md`,
  `docs/contents.md`, `docs/repository-layout.md`, `docs/developers-guide.md`,
  `docs/users-guide.md`, and `docs/formal-verification-methods-in-weaver.md`.

### Budget evidence

| Plan   | Limit set                                  | Outcome                                |
| ------ | ------------------------------------------ | -------------------------------------- |
| 12.1.1 | 12 documents, 6 Rust files                 | Completed; no breach recorded          |
| 12.1.2 | 25 files, 1,200 lines                      | 35 files in the squash; net code 1,441 |
| 5.2.1  | 12 source files, 380 lines                 | Completed; no breach recorded          |
| 13.2.1 | Draft raised from 4,420 to 5,930 in review | Not yet implemented                    |

_Table 2: Recorded budgets of comparable plans._

Earlier plans under-counted tests, behavioural steps, and private-item
documentation. Plan 12.1.2 also recorded a 116-file pull request as a breach
afterwards. 12.1.1 is the closest comparison: a documentation contract with a
checking crate, whose `src/lib.rs` and main test reached 530 and 401 lines. The
probe model is about 200 dense Python lines; strict lints, documentation, and
decomposition make four times that likely in Rust, plus as much again for
tests. Documentation is estimated from the 33 resolutions at about 20 lines
each across the RFC and one ADR, plus the dependent documents.

### Test facilities and gates

`rstest` 0.28, `rstest-bdd` 0.5, `proptest` 1.12, `googletest` 0.14.4, and
`pretty_assertions` 1.4.1 are locked. Kani 0.67 and `prover-tools` are on the
host, but the repository configures neither Kani nor Verus. Gates:
`make check-fmt`, `make lint`, `make test`, `make markdownlint` (which runs
`make spelling`), and `make nixie`. Run them one at a time, each through `tee`
to `/tmp/<action>-weaver-<branch>.out`, preferably by the `scrutineer` agent.

### Documents and skills to consult

Documents: those in scope, plus
`docs/adrs/adr-0005-verification-trust-boundary.md`,
`docs/adrs/adr-0012-versioned-selector-streams.md`,
`docs/rust-testing-with-rstest-fixtures.md`, `docs/rstest-bdd-users-guide.md`,
`docs/rust-doctest-dry-guide.md`,
`docs/reliable-testing-in-rust-via-dependency-injection.md`,
`docs/complexity-antipatterns-and-refactoring-strategies.md`,
`docs/ortho-config-users-guide.md`, and `docs/documentation-style-guide.md`.

Skills: `sdlc-implementation`, `execplans`, `rust-router`, then
`rust-state-machines`, `rust-unit-testing`, `proptest`, `rust-verification`, and
`arch-crate-design`; `hexagonal-architecture`; `codegraph-mcp`;
`arch-decision-records`; `domain-cli-and-daemons`;
`addressing-whitaker-findings`; `nextest`; `firecrawl-mcp`;
`en-gb-oxendict-style`; `commit-message`; `pr-creation`; `pr-babysitting`.

### Prior art consulted

- Rustup book, overrides and environment variables, and the 1.28 and 1.29
  changelogs: precedence, auto-install, and the `rust-analyzer` proxy fallback.
- Language Server Protocol 3.17: `workspaceFolders`, document ownership.
- Watchman `watch-project`: realpath then marker walk, and idle reaping.
- `lspmux` (`src/instance.rs`): instances keyed by command, arguments,
  environment, and root identity; its own comment records that holding the map
  lock during `initialize` blocks every client, which supports RFC invariant 3.
- gopls daemon guide: shared cache, separate session and view per client.
- AIP-194 and gRPC status guidance: exhausted capacity is retryable only when
  it is known to be short-lived and side-effect free.

## Conformance basis

There is no Terms of Reference. Upstream artefacts at `fe56d226`: roadmap
12.3.1 to 12.3.6; RFC 0002 and ADRs 0008 to 0010, all `Proposed`;
`docs/weaver-design.md` as listed above; RFC 0001; ADR 0005; and ADR 0012.

- `REQ-ACCEPT`: RFC 0002 and ADRs 0008 to 0010 are accepted.
- `REQ-CONSISTENT`: no unresolved contradiction between identity, server
  lifecycle, mutation isolation, and admission.
- `REQ-VOCAB`: RFC §8.1, request, error, lifecycle, and configuration
  vocabulary are fixed.
- `REQ-OPEN`: every open question and outstanding decision is resolved or
  assigned.
- `REQ-TRACE`: every clause has an owner task and a form of evidence.
- `GAP-DRIFT`: descriptions of current state that are no longer, or not yet,
  true.

```plaintext
REQ-VOCAB, REQ-OPEN, GAP-DRIFT  -> EP-M1 -> RFC clauses, ADR text
REQ-CONSISTENT                  -> EP-M2 -> ratified and superseded scenarios
REQ-CONSISTENT, GAP-DRIFT       -> EP-M3 -> reconciled documents
REQ-TRACE, REQ-ACCEPT           -> EP-M4 -> contract_documents tests, statuses
```

## Verification plan

### Abstract model

The model is a transition system over requests, workspaces, server slots, and
budgets. It has no time, threads, or files; an environment may at any step
change an execution identity, edit a file, crash or hang a server, disconnect a
client, replace a root, or begin shutdown, each a bounded number of times.

Workspace states and transitions (`LC`):

```plaintext
absent    -> creating   first request; one creator per key
creating  -> ready      initialization succeeded; published under the lock
creating  -> absent     initialization failed or timed out; waiters get it
ready     -> retired    idle and lease count zero, decided under the lock
ready     -> draining   root replaced, or shutdown, with leases outstanding
draining  -> retired    last lease released
retired   -> creating   next request, as a new generation
```

Server slot states and transitions (`LS`):

```plaintext
absent    -> starting   slot reserved in the process budget
starting  -> ready      initialize completed with the recorded identity
starting  -> backoff    start or initialize failed; budget slot returned
ready     -> draining   identity stale, idle eviction, or workspace retiring
ready     -> stopped    exit, transport failure, or repeated timeout
draining  -> stopped    last lease released, or its holder aborted
stopped   -> absent     process reaped; budget slot returned
backoff   -> absent     delay elapsed
backoff   -> open       failure limit reached
open      -> absent     circuit delay elapsed
```

Acquisition ranks (`AD`), lowest first; a request may wait only for a rank
above every rank it holds:

```plaintext
0 connection permit      1 daemon permit       2 workspace permit
3 workspace lease        4 coordination        5 server leases, in slot order
```

The registry lock is a leaf: nothing is awaited while it is held. Budget slots
are reserved, not waited for. Permits are released in reverse order, and ranks
1 to 5 are released before the response is written.

Request phases: `new`, `admitted`, `leased`, `queued`, `held`, `serving`, and
for mutations `staged`, `verified`, `committing`, `restoring`; then `done` with
an outcome and an `effects` value. Precondition for a run: limits satisfy
`CF-2`. A violated precondition is a configuration error, reported before any
request is admitted.

Workspace roots are modelled separately, as finite sequences of path components
with a set of marked directories. `root(p)` is the longest marked prefix of
`p`, or the starting directory if there is none; `domain(k)` is the set of `p`
with `root(p) = k`.

### Invariants and lemmas

Invariants over every reachable state (`INV`) and over runs (`L`):

- `INV-GEN` (`LC-3`): at most one live generation exists per key, and no
  request holds a lease on a generation that is not live or draining.
- `INV-EXCL` (`MU-3`, `AD-5`): an exclusive holder is the only coordination
  holder in its workspace.
- `INV-STAGE` (`MU-6`): no reader is served while a slot it uses is staged, and
  no slot is staged when its workspace has no exclusive holder.
- `INV-PIN` (`MU-4`): the identity at commit equals the identity at baseline for
  every server in the set.
- `INV-BASE` (`MU-1`): the bytes replaced at commit have the baseline digests.
- `INV-BUDGET` (`AD-4`): processes in `starting`, `ready`, or `draining` never
  exceed the process budget; auxiliary processes never exceed theirs.
- `INV-CAP` (`AD-1`): in-flight counts never exceed their limits.
- `INV-RETRY` (`RC-2`): an `overloaded` outcome has `effects = none`.
- `INV-FIFO` (`AD-5`): no request is granted coordination before an earlier
  enqueued, uncancelled request with which it conflicts.
- `INV-RANK` (`AD-3`): every wait is for a rank above all held ranks.
- `INV-REG` (`LC-2`): no step that waits, starts a process, or performs
  input/output occurs inside a registry critical section.
- `L-PROGRESS`: from every reachable state, system steps alone can reach a
  quiescent state with no leases, no staged content, and every request done.
  This follows from `INV-RANK` (the wait graph is acyclic), finite queues, and
  restore-or-retire.
- `L-INDEPENDENT` (`AD-1`): let `D` and `W` be the daemon and workspace limits,
  `1 <= W < D`. If at most `floor((D - 1) / W)` other workspaces are saturated,
  a request for a further workspace is not refused at daemon scope.
- `L-RETRYSAFE`: from `INV-RETRY` and phase order, repeating a retryable request
  cannot apply an edit twice because of the first attempt.
- `L-COMMIT` (`MU-7`): cancellation or shutdown at any step leaves `effects`
  equal to `none` before `committing`, and `committed` after it.
- `L-ROOT` (`WS-2`): `root` is total, idempotent, and constant on a domain, so
  every locator in one domain selects one key.
- `L-DOMAIN` (`WS-4`): domains of marked roots are pairwise disjoint and cover
  every path beneath a marker; adding a marker only refines the partition.

`L-PROGRESS` is a possibility property, not a fairness property. `INV-FIFO`
bounds overtaking; elapsed time is bounded by deadlines, which the model does
not represent.

### Axioms (trusted, not verified here)

- `AXIOM-LSP`: a language server processes messages on one transport in order,
  and document content is client-owned until closed (LSP 3.17).
- `AXIOM-RUSTUP`: `rustup which --toolchain` fails when the component is
  missing, and `RUSTUP_AUTO_INSTALL=0` prevents installation. Observed with
  1.29.3; 12.3.4 must exercise the real tool.
- `AXIOM-FS`: renaming one file is atomic for observers on one filesystem;
  device and file identifiers distinguish live directories.
- `AXIOM-KILL`: a killed child holds no documents.
- `AXIOM-LOCK`: an advisory file lock is released when its process exits.
- `AXIOM-SCOPE`: small scenarios are representative. This is a hypothesis, not a
  theorem, and is the main residual gap of bounded exploration.

### Methods per obligation

- Obligation: every `INV` except `INV-REG`, plus `L-PROGRESS`, `L-RETRYSAFE`,
  and `L-COMMIT`. Method: exhaustive breadth-first exploration, with backward
  reachability for `L-PROGRESS`. Rationale: the obligations concern
  interleavings of few actors; exploration is complete within the bound and
  returns a shortest trace. Domain: scenario table of up to four requests, two
  workspaces, two slots each, budgets of one to four, and one or two
  environment events. Artefact:
  `crates/weaver-tenancy-model/tests/ratified.rs`. Evidence: each scenario
  reports no violation, its exact state count, and that every request phase and
  environment event occurred at least once. Non-vacuity: the next obligation.
- Obligation: each superseded rule violates a named invariant. Method: `rstest`
  table over single-rule policy changes, each paired with the ratified control
  in the same scenario. Artefact: `tests/superseded.rs`. Evidence: the exact
  invariant and a shortest trace. Cases: lookup then lease, waiting for budget,
  holding or releasing on failed restoration, per-step identity, lease upgrade,
  shared capacity, no digest check, unserialized coordination, barging,
  draining as replacement, and cancellable commit.
- Obligation: `INV-REG`. Method: structural. Registry steps are a separate type
  with no access to budgets or servers, checked by a unit test that enumerates
  them. Residual gap: production conformance belongs to 12.3.3, whose roadmap
  text already requires it.
- Obligation: `L-INDEPENDENT`. Method: exhaustive over `1 <= W < D <= 12`, and
  `proptest` beyond. Oracle: direct count of free permits. Artefact:
  `tests/admission.rs`. Negative control: `W = D` must fail.
- Obligation: `L-ROOT`, `L-DOMAIN`. Method: exhaustive over trees of depth at
  most four and branching two, and `proptest` beyond. Oracle: a separate
  set-based definition, not the walk. Classes required in the generated sample:
  no marker, one marker, nested markers, marker at the start, marker at the
  filesystem root, and sibling markers. Negative controls: choosing the
  outermost marker, and using the starting directory. Artefact:
  `tests/roots.rs`.
- Obligation: the documents agree. Method: example and mutation tests over
  Markdown. Artefact: `tests/contract_documents.rs`. Non-vacuity: synthetic
  documents with a duplicate, undefined, or unowned clause, a stale status, or
  an outstanding decision are rejected, and the clause count is non-zero.

Regression files for `proptest` are committed if a failure is found. No
snapshot tests are planned: the task produces no output format. No end-to-end
test is planned: no observable workflow changes, and 12.3.6 owns that suite.
`EP-M0` and `EP-M4` run the existing suites to show that nothing changed.

### Formal verification decision

No lemma is selected for Verus or Kani in this task, for these reasons:

- there is no production kernel yet, so a proof would cover a second
  description of the rules whose correspondence rests on review;
- the interleaving obligations concern finite control state, where exhaustive
  exploration already reports concrete traces;
- `docs/formal-verification-methods-in-weaver.md` introduces Verus only after
  contracts stop moving, and roadmap 19.3.1 owns the tools.

`AGENTS.md` asks for exhaustive Verus proof of introduced lemmas or contractual
business logic, and the task brief asks for selective use with unproved theorem
statements kept in design documents until implementation reaches them. The plan
follows the second as the reading of the first. These statements go into RFC
0002 as hypotheses about the eventual proof structure:

| Theorem          | Statement                                                  | Kernel owner | Warranted tool                                  |
| ---------------- | ---------------------------------------------------------- | ------------ | ----------------------------------------------- |
| `T-DOMAIN`       | `L-ROOT` and `L-DOMAIN` for paths of any length            | 12.3.2       | Verus, over the pure root function              |
| `T-LIFECYCLE`    | `INV-GEN` is inductive for any number of keys and requests | 12.3.3       | Verus, if the registry kernel is pure           |
| `T-COORDINATION` | `INV-EXCL` and `INV-FIFO` are inductive for any queue      | 12.3.5       | Verus, over the queue kernel                    |
| `T-RANK`         | `INV-RANK` implies an acyclic wait graph                   | 12.3.5       | Verus                                           |
| `T-CAPACITY`     | `L-INDEPENDENT` for all limits                             | 12.3.5       | Verus or Kani                                   |
| `T-COMMIT`       | `L-COMMIT` and restore-or-retire                           | 16.1.6       | Kani first, per 19.3.3                          |
| `T-IDENTITY`     | structural identity equality                               | 12.3.4       | Property tests; derived equality needs no proof |

_Table 3: Theorem statements handed to later tasks._

Reason classes, configuration validation, telemetry, and document checks need
only conventional tests. Proof in later tasks does not replace the integration
tests of 12.3.6.

### Suggested BDD features

Proposed feature file:
`crates/weaver-tenancy-model/tests/features/tenancy_contract.feature`, run by
`rstest-bdd` against the model.

```gherkin
Feature: Multi-workspace daemon contract

  Scenario: A busy repository does not block another
    Given a daemon limit of 3 and a workspace limit of 2
    And two requests are waiting in workspace alpha
    When a read arrives for workspace beta
    Then the read is admitted

  Scenario: A repository is reused while a request holds it
    Given workspace alpha is ready with one lease
    When idle eviction runs
    Then workspace alpha stays ready

  Scenario: A hung language server cannot keep staged content alive
    Given a mutation in workspace alpha has staged content
    And the language server for alpha hangs
    When the mutation is abandoned
    Then the language server is stopped
    And a later read in alpha does not observe staged content

  Scenario: A toolchain change during verification has no effect on disk
    Given a mutation in workspace alpha has staged content
    When the execution identity for alpha changes
    Then the mutation ends with code "execution_identity_changed"
    And its effects are "none"

  Scenario: Overload is safe to retry
    Given every process slot is leased
    When a mutation needs another language server
    Then the mutation ends with class "overloaded"
    And its effects are "none"

  Scenario: Disconnection does not abandon a commit
    Given a mutation in workspace alpha is committing
    When its client disconnects
    Then the mutation's effects are "committed"

  Scenario: Directories in one repository select one workspace
    Given a repository rooted at "/work/alpha"
    When a request starts in "/work/alpha/src/lib"
    Then its workspace is "/work/alpha"
```

## Plan of work

### `EP-M0` baseline and drift check

Record `git rev-parse HEAD` and the result of each gate. Re-check every "today"
claim in this plan whose file changed since `fe56d226`, and the state of pull
requests #260 and #453. Determine, by a scratch test that is not committed,
whether the sandbox refuses plugins from the multi-threaded daemon; if so,
raise an issue and record its number. If a conflict entry is falsified, correct
it here and escalate when its resolution changes. If the baseline is red, stop.

### `EP-M1` contract clauses and revised decisions

In RFC 0002: correct §3 and §6.1; add `## 6.10. Normative contract` with the
clause families, state tables, rank order, reason vocabulary, configuration
vocabulary, and theorem table; rewrite §§6.1 to 6.9 and §7 where an `X` entry
applies, citing clauses instead of restating them; replace §11 with a
disposition for each question; extend §9 with the alternatives rejected in D5
to D33; and add the prior art. Each clause is one sentence of obligation with
an owner task and an evidence kind (`model`, `unit`, `integration`,
`end-to-end`, or `review`).

In ADRs 0008 to 0010: update the decision outcome, consequences, and risks to
cite clauses; turn each outstanding decision into a resolution or a named,
non-blocking deferral under known risks. Keep the options and decision
statements as history unless an entry changes them. Status stays `Proposed`.

Validation: `make fmt`, `make check-fmt`, `make markdownlint`, `make nixie`.
Commit the RFC and each ADR separately.

### `EP-M2` executable contract model

Create `crates/weaver-tenancy-model` (`publish = false`, no runtime
dependencies). Suggested modules: `policy.rs` (one ratified policy and the
single-rule variants), `state.rs`, `lifecycle.rs`, `admission.rs`,
`coordination.rs`, `servers.rs`, `mutation.rs`, `environment.rs`, `step.rs`,
`invariants.rs`, `explore.rs`, and `roots.rs`. Everything is pure and
deterministic; ordered collections keep counts and traces stable. Tickets are
renumbered by queue position so equivalent states coincide.

Work in red and green steps. First write `tests/superseded.rs` with one
superseded rule and its expected violation; it fails because nothing explores.
Add the explorer and the smallest model that produces the violation. Then add
the ratified control, which fails until the rule is implemented. Repeat per
rule, then add `ratified.rs`, `admission.rs`, `roots.rs`, and the behavioural
scenarios. Any surviving violation of the ratified policy is a contract defect:
change the RFC clause in the same commit and log it, or escalate if it alters
an approved resolution.

Add the workspace member, the repository-layout entry, and the module docs that
state the model's limits. Validation: all gates, and a recorded table of
scenarios and state counts in `Artefacts and notes`.

### `EP-M3` dependent documents

- `docs/weaver-design.md`: align §§2.1.12, 3.1.1, 4.2.3, and the manager text;
  correct §2.1.8 on artefacts; cite clauses.
- RFC 0001: clarify metrics, paths, and saturation as in X29.
- `docs/roadmap.md`: add clause references to the success text of 12.3.2 to
  12.3.6, 16.1.5, and 16.1.6, including model conformance for 12.3.3 and
  12.3.5. Do not renumber or add tasks.
- `docs/developers-guide.md`: a section on the contract: where clauses live, how
  to amend one, the model's scope and reuse policy, and how to add a scenario.
- `docs/users-guide.md`: state plainly that one daemon serves the directory it
  was started in, and that two sockets in one directory cannot run together.
- `docs/formal-verification-methods-in-weaver.md`: list the theorem statements.
- `docs/contents.md` and `docs/repository-layout.md`: entries.

Search for stale claims with
`git grep -nE 'RFC 0002|ADR 00(08|09|10)|workspace manager' -- docs ':!docs/execplans'`.

### `EP-M4` document gate and acceptance

Red: add `tests/contract_documents.rs`, which fails because the statuses are
`Proposed`. It checks that clause identifiers are unique; that every cited
identifier is defined; that each has an owner listed in the roadmap and an
evidence kind; that clauses marked `model` are exactly those cited by model
invariants; that the RFC and ADR statuses agree; and that no accepted document
has outstanding decisions. Green: change the RFC and ADRs to
`Accepted, <date>. <summary>`, update ADR dates and `docs/contents.md`, and
mark 12.3.1 done in `docs/roadmap.md`.

Acceptance is the user's act. Set the statuses in the pull request and treat
its merge as the acceptance; the date is the date of the commit.

## Milestones and plateaus

| Milestone | Discharges                | Plateau                                  | Remaining gap        |
| --------- | ------------------------- | ---------------------------------------- | -------------------- |
| `EP-M0`   | baseline                  | Nothing tracked changes but this plan    | All                  |
| `EP-M1`   | `REQ-VOCAB`, `REQ-OPEN`   | Documents agree, still `Proposed`        | No machine evidence  |
| `EP-M2`   | `REQ-CONSISTENT`          | Contract shown satisfiable in the bounds | Other documents lag  |
| `EP-M3`   | `GAP-DRIFT`               | All live documents describe one contract | Not accepted         |
| `EP-M4`   | `REQ-TRACE`, `REQ-ACCEPT` | Accepted and guarded                     | Implementation tasks |

_Table 4: Milestones._

At each boundary check: assigned requirements met; no production file changed;
no resolution changed without a logged decision; no new dependency; trace links
current. Compatibility decision for every milestone: none. The crate is private
and the documents have no external consumer. Recovery: revert the milestone's
commits; each milestone is independent of later ones.

## Concrete steps

```sh
git fetch origin
git rev-parse HEAD
B=$(git branch --show-current)
make check-fmt 2>&1 | tee "/tmp/check-fmt-weaver-$B.out"
make lint 2>&1 | tee "/tmp/lint-weaver-$B.out"
make test 2>&1 | tee "/tmp/test-weaver-$B.out"
make markdownlint 2>&1 | tee "/tmp/markdownlint-weaver-$B.out"
make nixie 2>&1 | tee "/tmp/nixie-weaver-$B.out"
cargo nextest run -p weaver-tenancy-model   # EP-M2 onwards
```

Use the default target directory and the shared Cargo cache. A message about
waiting for a compile slot is normal; do not retry or kill the build.

## Validation and acceptance

- `make check-fmt`, `make lint`, `make test`, `make markdownlint`, and
  `make nixie` pass after `EP-M2`, `EP-M3`, and `EP-M4`; the Markdown gates
  pass after `EP-M1`.
- `cargo nextest run -p weaver-tenancy-model` passes. Reverting any one ratified
  rule in `policy.rs` makes `ratified.rs` fail with a named invariant.
- Deleting a clause cited by an ADR, or setting one ADR back to `Proposed`,
  makes `contract_documents` fail.
- `git diff --stat "$(git merge-base origin/main HEAD)" -- crates` lists only
  `crates/weaver-tenancy-model`.
- Every `X` entry is traceable to a clause, and every RFC question and ADR
  outstanding decision has a disposition.
- Roadmap 12.3.1 is checked; 12.3.2 to 12.3.6 are not.

## Idempotence and recovery

All steps can be repeated. Model and document tests are deterministic. Seeded
faults are permanent table entries, not edits to revert. `make fmt` may reflow
Markdown; commit that separately if it touches unrelated files.

## Artefacts and notes

Feasibility probe (throwaway Python, 2026-11-24, not committed). One to four
requests, two workspaces, two slots each, at most one event per environment
kind; the all-events case for three requests did not finish in 110 seconds.

| Rule changed                    | Violation found               | States searched |
| ------------------------------- | ----------------------------- | --------------- |
| Look up, then count the lease   | lease on retired generation   | 8               |
| Wait for process budget         | no progress to quiescence     | 205             |
| Hold lease until restored       | no progress to quiescence     | 698             |
| Release without restoring       | staged content outlives lease | 229             |
| Re-read identity per step       | verdict mixes identities      | 125             |
| Upgrade shared to exclusive     | no progress to quiescence     | 161             |
| Workspace limit equals daemon's | cross-workspace starvation    | 49              |
| No digest check                 | lost update                   | 208             |
| No exclusion                    | exclusive lease shared        | 42              |
| Barging queue                   | none: needs `INV-FIFO`        | 119             |

_Table 5: Probe results. Every ratified control passed; the largest searched
28,968 states._

The probe did not model creation, aliases, domains, plugins, shutdown, or
post-commit notification. `EP-M2` must.

Questions for the approver:

1. D5: is the tenancy root the nearest `.git` ancestor, with no way to name an
   exact root inside a repository?
2. D11: should read commands refuse targets outside the workspace domain? This
   will stop `get-card` on, for example, dependency sources.
3. D9: should loopback TCP endpoints serve only listed roots?
4. D28: is the cross-daemon advisory lock wanted, or is the guarantee per
   daemon with digests as the only cross-daemon guard?
5. D3: build the executable model (`EP-M2`), or ratify on review alone?
6. D4: leave Verus to the kernel-owning tasks? They precede roadmap 19.3.1, so
   agreeing also means deciding later whether 19.3.1 moves earlier.
7. D13: should a request that meets a draining workspace be refused as
   retryable, rather than wait?
8. D32: keep exit status 1 and 2, with retryability only in the structured
   error?

## Interfaces and dependencies

New private crate `weaver-tenancy-model`, with no runtime dependencies.
Development dependencies: `rstest`, `rstest-bdd`, `rstest-bdd-macros`,
`proptest`, `googletest`, `pretty_assertions`, `cap-std`, and `camino`, all
from the workspace. Intended public surface, provisional:

```rust,ignore
pub struct Policy { /* one field per ratified rule */ }
impl Policy { pub fn ratified() -> Self; }
pub struct Scenario { /* requests, limits, environment budget */ }
pub enum Invariant { /* one variant per INV and L identifier */ }
impl Invariant { pub const fn clauses(self) -> &'static [&'static str]; }
pub struct Exploration { pub states: usize, pub verdict: Verdict }
pub fn explore(scenario: &Scenario, policy: &Policy) -> Exploration;
```

Reuse policy, to be recorded in `docs/developers-guide.md`: the crate is an
executable specification and test oracle. It has no runtime consumer. Tasks
12.3.2 to 12.3.6 may depend on it for tests that compare production decisions
with the model. It must not gain input/output, threads, or a clock. A sweep
found no existing state-space explorer, lease, registry, or admission
abstraction in the workspace.

In hexagonal terms the clauses describe the daemon's inner policy. Ports to be
defined by later tasks are the workspace filesystem, server launcher, toolchain
resolver, clock, and telemetry sink; their adapters are outside this task.

Reason vocabulary to be ratified (`RC`):

| Class             | Retryable | Codes                                                                                                                                                                                                    |
| ----------------- | --------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `invalid_request` | no        | `protocol_unsupported`, `workspace_missing`, `workspace_malformed`, `workspace_inaccessible`, `workspace_not_utf8`, `workspace_untrusted`, `workspace_not_allowed`, `path_outside_workspace`             |
| `conflict`        | no        | `workspace_overlap`, `workspace_alias`, `stale_base`, `execution_identity_changed`                                                                                                                       |
| `unavailable`     | no        | `toolchain_missing`, `component_missing`, `server_start_failed`, `server_circuit_open`, `server_lost`                                                                                                    |
| `overloaded`      | yes       | `connection_capacity`, `daemon_capacity`, `workspace_capacity`, `server_capacity`, `process_budget`, `auxiliary_budget`, `queue_deadline`, `workspace_draining`, `daemon_stopping`, `mutation_elsewhere` |
| `cancelled`       | no        | `client_disconnected`, `shutdown`, `request_deadline`                                                                                                                                                    |
| `internal`        | no        | `internal`                                                                                                                                                                                               |

_Table 6: Reason classes and codes._

Configuration vocabulary to be ratified (`CF`): daemon scope `connection_max`,
`daemon_max_in_flight`, `workspace_max_in_flight`, `process_budget`,
`auxiliary_process_budget`, `workspace_max`, `workspace_idle`, `server_idle`,
`queue_deadline`, `request_deadline`, `shutdown_deadline`, `restore_deadline`,
`identity_freshness`, `restart_backoff`, `restart_limit`, `circuit_delay`, and
`allowed_workspace_roots`; workspace scope capability overrides, adapter
commands, environment allowlist additions, and toolchain override. Relations:
`1 <= workspace_max_in_flight < daemon_max_in_flight <= connection_max`;
`process_budget` at least the largest server set one mutation may need;
`queue_deadline < request_deadline`; every budget and deadline is finite and
positive.

## Revision note

2026-11-24: initial draft. No implementation work has started.
