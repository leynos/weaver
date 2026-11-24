# 12.3.1 Ratify the multi-workspace daemon contracts

This ExecPlan (execution plan) is a living document. The sections `Constraints`,
`Tolerances`, `Risks`, `Progress`, `Surprises & discoveries`, `Decision log`,
`Outcomes & retrospective`, `Conformance basis`, and `Verification plan` must
be kept up to date as work proceeds.

Status: DRAFT (design review and recheck applied; awaiting user approval)

Branch: `12-3-1-ratify-multi-workspace-daemon-contracts`

Roadmap item: `12.3.1` in `docs/roadmap.md`.

## Purpose / big picture

Weaver is a command-line tool (`weaver`) backed by a background daemon
(`weaverd`) that answers questions about source code and applies verified
edits. By default one daemon serves one user. It remembers the directory it was
started in and treats that as the only repository, so this goes wrong today:

```sh
cd ~/work/alpha && weaver daemon start
cd ~/work/beta && weaver act apply-patch < change.patch   # edits alpha
```

Four documents propose the cure: RFC 0002 and ADRs 0008, 0009, and 0010. All
are marked `Proposed`, and they contradict each other in places. Roadmap items
12.3.2 to 12.3.6 implement them, and 13.4.1, 14.1.1, 15.3.1, 15.3.4, 16.1.4,
16.1.5, 16.1.6, and 16.1.10 depend on the result.

This task settles the contract before that code is written. Afterwards:

- a new design document holds each rule once, with a stable identifier, an
  owning roadmap task, and one kind of evidence;
- the RFC and the three ADRs are `Accepted`, agree with that document, and leave
  no open question that a later task must guess at; and
- an executable model shows that the rules can hold together, and that the
  wording they replace breaks.

No daemon behaviour changes: the commands above still go wrong until 12.3.2 and
12.3.3 land. Success is observable by running:

```sh
cargo nextest run -p weaver-tenancy-model
```

Expected: the ratified rules pass in every scenario, and each superseded rule
is rejected with the invariant it breaks.

## Constraints

- C1. Implementation begins only after the user approves this plan.
- C2. No production behaviour changes. Existing crates and their tests are not
  modified.
- C3. No production crate may depend on the model crate. Tasks 12.3.2 to 12.3.6
  may use it as a development dependency.
- C4. No new third-party dependency. `rstest`, `rstest-bdd`,
  `rstest-bdd-macros`, `proptest`, `googletest`, `pretty_assertions`,
  `cap-std`, and `camino` are workspace dependencies already, and are
  authorized for the model crate as development dependencies.
- C5. `docs/documentation-style-guide.md` governs status lines and requires an
  accepted ADR to have no outstanding decision. It is shared and not edited.
- C6. The contract must not weaken RFC 0002's six invariants, ADR 0005's trust
  boundary, the Double-Lock rule (syntactic and semantic checks both pass
  before commit), or the stale-base rules of roadmap 16.1.x. A deliberate
  narrowing is recorded as a decision and put to the approver.
- C7. Vocabulary owned elsewhere stays there. Wire error envelopes, exit
  statuses, and localization belong to 12.3.2 and 13.2.2 with OrthoConfig;
  selector identifiers belong to 15.3.3; event names belong to 13.4.1.
- C8. Configuration clauses use OrthoConfig 0.11.0 as the task brief requires.
  The facilities they need (`ConfigFilePolicy::project_root`, `scope_order`,
  `EnvSource`) were read in the published sources of 0.10.0, which the
  workspace pins, and of 0.11.0. This task changes no dependency: 12.3 adds no
  OrthoConfig command-contract consumer, and the open 13.2.1 plan (pull request
  #453) carries the upgrade. `docs/orthoconfig-consumer-boundary.toml` is
  unchanged.
- C9. `typos.toml` is generated. Completed ExecPlans and `docs/archive/` stay
  untouched.
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

| Milestone | Code estimate | Code limit | Configuration limit | Documentation limit |
| --------- | ------------- | ---------- | ------------------- | ------------------- |
| `EP-M0`   | 0             | 0          | 0                   | 0                   |
| `EP-M1`   | 0             | 0          | 5                   | 950                 |
| `EP-M2`   | 1,000         | 1,200      | 40                  | 70                  |
| `EP-M3`   | 1,700         | 2,040      | 5                   | 60                  |
| `EP-M4`   | 650           | 780        | 5                   | 20                  |
| `EP-M5`   | 0             | 0          | 5                   | 350                 |
| `EP-M6`   | 200           | 240        | 5                   | 80                  |
| Total     | 3,550         | 4,260      | 65                  | 1,530               |

_Table 1: Net-line estimates and limits._

Stop and escalate when any of these is reached:

- a limit in Table 1, or more than 50 non-ExecPlan files;
- a change to a clause that alters an obligation in `Clauses` (rewording does
  not count);
- a contradiction the model finds that no rewording removes within three tries;
- any scenario above 500,000 states, or the crate's tests above 60 seconds once
  compiled;
- a need to edit production code, add a dependency, or renumber the roadmap;
- a gate that still fails after three fix attempts; or
- a milestone above eight hours of active work, excluding compile queueing.

`Budget evidence` explains the numbers. If `EP-M2` finishes above 1,000 code
lines, re-estimate `EP-M3` before starting it. Measure the test time of `EP-M2`
in the unoptimized profile before adding the larger scenarios.

## Risks

- R1. The model proves something easier than the contract. Severity: high.
  Likelihood: medium; the first draft's probe did this. Mitigation: the
  representation table, monitors that do not read the policy, and the fault
  matrix, in which each invariant is checked alone.
- R2. The contract is over-specified and later tasks cannot follow it.
  Severity: high. Likelihood: medium. Mitigation: clauses state obligations,
  not types; names in RFC 0002 §6.2 stay provisional; numeric defaults, wire
  shapes, and user-visible surface stay with their owners.
- R3. The model and production code drift apart. Severity: medium. Likelihood:
  high over time. Mitigation: pure decision functions that conformance tests
  can call, obligations added to 12.3.3 and 12.3.5, and a retirement rule (D3).
- R4. One approval binds many rules. Severity: medium. Likelihood: certain.
  Mitigation: full clause text here, and questions for each rule that changes
  user-visible behaviour or trust.
- R5. A finite model cannot show starvation or elapsed time. Severity: medium.
  Likelihood: certain. Mitigation: `INV-FIFO` is a history monitor; deadlines
  are modelled as steps that are always enabled; real time is left to 12.3.6.
- R6. Pull request #260 (endpoint-scoped artefacts, ADR 0014) edits design
  §2.3.2, the users' and developers' guides, and `docs/contents.md`, and
  currently conflicts with `main`. Severity: low. Likelihood: medium.
  Mitigation: allocate no ADR number, limit any §2.3.2 edit to one sentence,
  and say in RFC 0002 only that the behaviour depends on #260.
- R7. The host compile queue is long. Severity: low. Likelihood: high.
  Mitigation: start each gate once and wait.
- R8. Upstream behaviour of `pyrefly` and `tsgo` was not checked from source.
  Severity: medium. Likelihood: medium. Mitigation: clauses require adapter
  conformance tests in 12.3.4, with stopping the server as the safe fallback.

## Progress

- [x] (2026-11-24) Branch renamed; Lody session titled.
- [x] (2026-11-24) Reconnaissance by a four-agent Wyvern team.
- [x] (2026-11-24) Prior-art research, `rustup` probes, and first probe model.
- [x] (2026-11-24) Initial draft (`b359866c`).
- [x] (2026-11-24) Five-reviewer design review: four "revise" or equivalent,
      one "conditions". Findings applied; replacement rules probed.
- [x] (2026-11-24) All five rechecked the rewrite: four "conditions", one
      "revise", with every remaining point described as wording. Conditions
      applied; probes repeated with policy-independent monitors.
- [ ] Plan approved by the user.
- [ ] `EP-M0` baseline and drift check.
- [ ] `EP-M1` contract document, proof sketches, and revised RFC and ADRs.
- [ ] `EP-M2` registry, permit, and ownership kernels.
- [ ] `EP-M3` coordination, server, and mutation kernels.
- [ ] `EP-M4` behavioural scenarios and fault matrix.
- [ ] `EP-M5` dependent documents reconciled.
- [ ] `EP-M6` reference check, acceptance, and roadmap closure.

## Surprises & discoveries

- Observation: `rustup run <toolchain> rust-analyzer`, the launch RFC 0002 §6.4
  prefers, silently runs another `rust-analyzer` when the component is missing.
  Evidence: rustup 1.29.3 with toolchain 1.82.0 and a decoy on `PATH` printed
  `falling back to ".../rust-analyzer"`, ran the decoy, and exited 0.
  `rustup which` refused. Impact: `LS-6`.
- Observation: read-only rustup commands can install toolchains. Evidence:
  during planning, `rustup toolchain list` in a directory whose toolchain file
  named `nightly-2019-01-01` installed it; it was removed afterwards. With
  `RUSTUP_AUTO_INSTALL=0` resolution refused. Impact: `LS-5`.
- Observation: an ambient `RUSTUP_TOOLCHAIN` outranks the repository's file, and
  language servers inherit the daemon's environment today. Impact: `LS-5`.
- Observation: `rustup which --toolchain` rejects the absolute path that a
  `path=` toolchain resolves to; `RUSTUP_TOOLCHAIN=<path> rustup which` accepts
  it (reviewer's test on 1.29.3). Impact: `LS-6`.
- Observation: rust-analyzer answers a diagnostics request with an empty success
  while it is still loading, and `didClose` queues an asynchronous reload
  rather than reverting content (reviewer's reading of upstream source). Impact:
  `LS-3`, `MU-6`, `MU-10`. Today's semantic lock is affected; `EP-M0` raises
  an issue if it confirms this.
- Observation: an older daemon ignores unknown request fields
  (`crates/weaverd/src/dispatch/request.rs:16`), so adding a `workspace` field
  would let a new client edit the wrong repository. Impact: `WS-7`.
- Observation: RFC 0002 §6.1 and design §2.3.2 say lock, PID, and health files
  are scoped to the endpoint. At `fe56d226` they are not
  (`crates/weaver-config/src/runtime.rs:84`). Impact: X24.
- Observation: a removed and recreated directory reuses its inode unless a
  handle is held (reviewer's test on ext4: 20 of 20, against 0 of 20). Impact:
  `WS-3`.
- Observation: the sandbox refuses to start a plugin from a process with more
  than one thread (`crates/weaver-sandbox/src/sandbox.rs:99`), and the daemon
  has several; its Linux command type also cannot set a working directory or
  environment. Evidence: by reading only. Impact: `LS-9` is a prerequisite of
  plugin work; `EP-M0` checks it and raises an issue.
- Observation: a commit that replaces a file, fails, and cannot restore it must
  still change the revision; otherwise another plan commits over a state it
  never read. Evidence: the third probe's monitor compared file histories and
  found two violations of the then-ratified rule, which no reviewer had
  reported. Impact: `MU-12`.
- Observation: workspace `rust-analyzer.toml` and `.cargo/config.toml` can name
  programs that a server runs (reviewer's reading of upstream documentation).
  Impact: `TR-1` and `TR-4` are worded accordingly.
- Observation: the first draft's `L-INDEPENDENT`, `L-COMMIT`, `L-DOMAIN`,
  `INV-PIN`, and `INV-BASE` were false as stated, and its probe did not monitor
  several of them. Impact: `Verification plan` was rewritten.

## Decision log

All dated 2026-11-24. All were proposed by the planning agent and revised after
review. Approving the plan approves `Clauses`.

- D1. Decision: revise first, accept last, in one pull request. Documents stay
  `Proposed` until `EP-M6`. Rationale: the style guide forbids outstanding
  decisions in an accepted ADR, and accepting contradictory text would mislead
  12.3.2. Precedent: ADR 0013, accepted inside its pull request.
- D2. Decision: normative clauses live in a new
  `docs/multi-workspace-daemon-design.md`. RFC 0002 keeps its proposal and
  rationale; ADRs record decisions; both cite clauses. Rationale: the style
  guide gives RFCs no amendment process, says ADRs record decisions, and puts
  definitions in design documents. The first draft put clauses in the RFC; the
  contracts reviewer showed that later amendments would have no route.
- D3. Decision: add a private crate, `crates/weaver-tenancy-model`, as an
  executable specification made of small kernels with pure decision functions.
  Rationale: the conflicts are about interleavings, which reading finds poorly;
  five reviewers and the author all missed at least one that exploration found.
  Alternatives: review only; TLA+, Alloy, or Quint (a new toolchain);
  `stateright` (unauthorized dependency); Kani (not configured); one monolithic
  model (a reviewer ran the first probe's three-request case with all events:
  1,664,258 states, with several rules unmodelled). Retirement: when 12.3.3 and
  12.3.5 have pure production kernels explored directly, they delete the
  corresponding model kernel.
- D4. Decision: ratify the interlocking core only, and state constraints on
  matters owned elsewhere. Rationale: the reviewers' strongest alternative; it
  satisfies the roadmap text without binding exit codes, wire shapes, or
  selector formats. The first draft ratified those and was wrong about exit
  status (`crates/weaverd/src/dispatch/errors.rs:108`).
- D5. Decision: an unmarked directory is refused instead of becoming a fallback
  workspace; `.weaver-root` is added as a marker. Rationale: the draft's
  fallback produced overlapping workspaces whose validity depended on request
  order, and a definition under which such a workspace could not edit its own
  files. An explicit marker also lets a user name an exact root while keeping
  roots a function of the filesystem.
- D6. Decision: keep both the revision and digests as commit guards for plans
  Weaver makes, checked at exclusive grant, with digests rechecked per file.
  Rationale: the draft dropped the revision, which allowed write skew across
  the planning gap (a rename planned, then another mutation adding a use
  elsewhere) and contradicted RFC invariant 5, design §4.2.3, and RFC 0003 §6.9.
- D7. Decision: no cross-daemon lock. Isolation is per daemon (`MU-14`).
  Rationale: RFC §4.2 excludes coordination across daemons, and a lock in the
  runtime directory fails in exactly the multi-daemon cases. If wanted later,
  an advisory lock on the open root handle is the better form; a reviewer
  confirmed that it contends across mount and user namespaces.
- D8. Decision: no Verus or Kani milestone. See `Formal verification decision`.
- D9. Decision: `tracing` only; no `metrics` use in 12.3. Rationale: RFC 0001
  makes a metrics crate a non-goal pending its own design, and `AGENTS.md`
  requires `metrics` only where metrics are required. RFC 0002 §7.2 is amended
  rather than RFC 0001. The draft had called a reversal a clarification.
- D10. Decision: repository code executed by a language server is an accepted,
  documented risk for the first implementation (`TR-4`), and repository files
  may not choose what Weaver executes (`TR-1`). Rationale: opening a repository
  in an editor carries the first risk already; the second would be new.
- D11. Decision: narrow RFC invariant 4 to "equals the identity observed at
  grant, with inputs outside the workspace at most `identity_freshness` old",
  and check it again before commit entry (`LS-4`, `MU-7`). Rationale: nothing
  can hold an external identity still, so the absolute form cannot be met. Put
  to the approver as question 10, as C6 requires.
- D12. Decision: children inherit the daemon's startup environment minus a
  scrub list. Rationale: reviewers disagreed. An allowlist breaks builds that
  need variables it omits, and missing variables make diagnostics weaker, so a
  mutation could pass wrongly; inheritance shares the first shell's `PATH` and
  secrets with every repository's build. Put to the approver as question 5.

Decision records: D5, `TR-2`, `TR-3`, and `CF-1` are recorded in ADR 0008; D10,
D11, D12, `TR-1`, `TR-4`, and `TR-5` in ADR 0009; D6 and D7 in ADR 0010.

## Contract conflicts

"Today" means `fe56d226`. A _workspace_ is one repository root served by the
daemon. A _lease_ is a counted right to use something. A _shared_ lease may be
held by several readers; an _exclusive_ lease by one mutation alone. Each entry
gives the conflict, then the clauses that resolve it.

Identity and trust:

- X1. The locator comes from the invocation directory, yet the same repository
  must have one state; `repo` and `repo/src` differ. `WS-1`, `WS-2`.
- X2. Canonical paths do not identify bind mounts or a replaced directory.
  `WS-3`, `WS-5`.
- X3. Two workspaces may mutate concurrently, but nested roots share files.
  `WS-2`, `WS-4`.
- X4. A new daemon rejects bad requests, but an old one ignores the new field.
  `WS-7`.
- X5. RFC §6.1 rejects locators outside "an allowed capability root" that is
  never defined, and non-Unix endpoints default to loopback TCP, which is not
  per-user. `TR-2`, `TR-3`.
- X6. Durable selectors carry a `workspace_id`, telemetry wants an opaque one,
  and the registry key is a path. `WS-8`, `OB-2`.
- X7. Commands must affect only their own repository, but target bases and
  reads of any absolute URI are unspecified. `WS-4`, `WS-6`.
- X8. Server configuration is workspace-specific, yet a cloned repository must
  not choose programs to run. `TR-1`, `TR-4`, `TR-5`.

Lifecycle:

- X9. An `Arc` obtained before eviction keeps memory alive but does not prevent
  retirement, and which state is workspace-owned is stated in three different
  ways. `LC-1`, `LC-2`, `LC-4`, `LC-8`.
- X10. "Already being retired" is called a bad locator, while creation is
  single-flight; failure, refusal, and re-creation have no transitions. `LC-3`,
  `LC-5`, `LC-6`.
- X11. Open documents block eviction, against bounded retention. `LC-7`.
- X12. Server states omit warming, failed start, backoff, and healthy draining;
  `initialize` is treated as readiness; server requests and watched-file
  support are assumed. `LS-2`, `LS-3`, `LS-7`, `LS-8`.
- X13. "Target workspace", "adapter-selected process root", and design §3.1.1
  disagree; one folder conflicts with an ordered list; "the current identity"
  is singular although roots nest. `LS-1`, `LS-2`.
- X14. Toolchains come from the workspace, rustup precedence is preserved, and
  there is no silent fallback: these cannot all hold with `rustup run` and an
  inherited environment. `LS-5`, `LS-6`.
- X15. Invariant 4 is absolute, but freshness is bounded and nothing watches
  files. `LS-4`, `MU-7`.
- X16. Plugins start servers outside the pool, its identity, and its budget.
  `LS-9`, `TR-5`.

Mutation:

- X17. The baseline is captured under the exclusive lease, but plugin planning
  takes up to 60 seconds and needs content first. `MU-1` to `MU-3`.
- X18. Server replacement drains leases while a mutation holds one and may be
  editing the input that changed. `MU-5` to `MU-7`.
- X19. The exclusive lease lasts until document state is restored, but a hung
  server never confirms, and closing a document is not restoration. `MU-10`,
  `MU-11`.
- X20. Shutdown is bounded, yet a live commit is not abandoned; checks precede
  the commit, yet 16.1.6 compares immediately before replacement; "atomic"
  overstates per-file replacement. `MU-4`, `MU-8`, `MU-9`, `MU-13`.
- X21. Readers observe one committed revision, but nothing tells servers that
  files changed. `MU-10`, `MU-12`.
- X22. Separate sockets are an escape hatch, but leases are per daemon. `MU-14`.

Admission, vocabulary, configuration:

- X23. Independent limits let one workspace fill the daemon or the process
  budget, and waiting for a slot while holding one can deadlock. `AD-1` to
  `AD-9`.
- X24. Endpoint-scoped artefacts are described as existing. Corrected as a
  dependency on #260.
- X25. Open questions ask for numeric defaults that RFC §4.2 excludes. `CF-3`,
  `CF-4`.
- X26. "Retryable", `unavailable`, validation failure, and refusal have separate
  vocabularies. `RC-1` to `RC-4`.
- X27. RFC 0002 asks for metrics that RFC 0001 defers. `OB-1`.
- X28. Project configuration is discovered from the daemon's startup directory,
  so the first repository's settings would govern the others, and a repository
  could redirect the client with `daemon_socket`. `CF-1`, `CF-2`, `CF-5`.
- X29. Shadow servers are "not required", yet 16.1.5 needs servers to observe
  deletions and moves that overlays cannot express. `MU-6`, and a recorded
  trigger in ADR 0010.

## Clauses

Form: identifier (owner task; evidence): obligation. Evidence `model` also
requires a conformance test in the owner task. These sentences are the proposed
content of `docs/multi-workspace-daemon-design.md`; `EP-M1` replaces references
to this plan's tables with section names. "Pinned" means reserved by the
mutation in question. An instance is "outgoing" while draining or stopping.

Workspace identity:

- `WS-1` (12.3.2; integration): A locator is an absolute starting directory; the
  daemon canonicalizes it and never infers a workspace from its own directory.
  On Unix the request also carries the start's device and file identifiers, and
  a difference is refused as `workspace_view_mismatch`.
- `WS-2` (12.3.2; model): The root is the nearest ancestor-or-self of the start
  that contains a `.git` entry or a `.weaver-root` file. With neither, the
  request is refused as `workspace_unmarked`; a lookup error other than absence
  is `workspace_inaccessible`; a start inside `.git` or a bare repository is
  `topology_unsupported`.
- `WS-3` (12.3.3; integration): The key is the canonical UTF-8 root. State holds
  an open root handle for its life, performs workspace input/output relative to
  it, and rechecks that the path still names it and is still marked: at lease
  grant, where failure is `workspace_replaced`, and at commit entry, where
  failure is `stale_base`.
- `WS-4` (12.3.3; model): A path is owned by its nearest marked
  ancestor-or-self. Every mutation target must be owned by the workspace at
  exclusive grant and at commit entry, and may not be a marker or have a `.git`
  component in any letter case; otherwise the refusal is `target_not_owned`.
- `WS-5` (12.3.3; unit): If another path with the same root identity is active,
  an idle holder is retired; otherwise the request is refused as
  `workspace_alias`.
- `WS-6` (12.3.2; integration): Every target names its base, and patch paths are
  root-relative. Reads outside the workspace are permitted, marked external,
  and never stored in its caches; mutations through a symlink that leaves the
  root are refused. A read target is a regular file opened without blocking and
  read to a bound.
- `WS-7` (12.3.2; end-to-end): An older daemon cannot parse the new envelope as
  a routable request. A new daemon identifies its protocol before any result,
  and refuses an older client in that client's framing. A missing
  identification is reported and never causes an automatic restart.
- `WS-8` (15.3.3; review): A durable workspace identifier is an equality hint
  and never grants authority.

Trust:

- `TR-1` (12.3.4; integration): Weaver itself never starts a program, sets a
  variable, or uses a toolchain path because a workspace file names it; only
  daemon or user scope may. A workspace `path=` toolchain is
  `toolchain_untrusted`.
- `TR-2` (12.3.2; integration): The root and its marker must be owned by the
  effective user or the superuser and not be writable by every user; a
  group-writable root or marker also needs user scope to list the root in
  `allowed_workspace_roots`, which can waive this clause and nothing else.
  Otherwise the refusal is `workspace_untrusted`. Inputs Weaver itself reads
  above the root follow the same rule without a waiver.
- `TR-3` (12.3.2; integration): Requests that select a workspace are served only
  on an endpoint in an owner-only directory, or one with equivalent
  authentication; otherwise the refusal is `endpoint_not_private`.
- `TR-4` (12.3.4; review): Language servers may run any program named by
  repository build or server configuration, during reads as well as mutations.
  This is an accepted and documented risk.
- `TR-5` (12.3.4; integration): Auxiliary processes start in a private directory
  with interpreter isolation where the interpreter offers it.

Workspace lifecycle:

- `LC-1` (12.3.3; model): Lookup returns a counted lease taken in the same
  registry critical section, and the count equals the outstanding leases.
- `LC-2` (12.3.3; integration): A registry critical section never waits, starts
  a process, or performs input/output.
- `LC-3` (12.3.3; model): A key has at most one live generation, and only the
  transitions in Table 5 occur.
- `LC-4` (12.3.3; model): Retirement is decided in a registry critical section
  while the count is zero.
- `LC-5` (12.3.3; model): Creation belongs to the daemon and has a deadline; a
  client's cancellation does not end it, and failure or shutdown publishes
  nothing and gives every waiter the same outcome.
- `LC-6` (12.3.3; model): Entering `draining` stops starting and warming
  instances. A draining workspace grants no lease and starts no server,
  refusing with `workspace_draining`; a stopping daemon refuses with
  `daemon_stopping`.
- `LC-7` (12.3.3; model): Only leases block retirement. A starting or warming
  instance holds one until it is ready or reaped. No document stays open after
  the lease that opened it.
- `LC-8` (12.3.3; integration): Every routed operation resolves exactly one key
  first. Caches, servers, revisions, and coordination belong to exactly one
  workspace; only immutable registries are daemon-wide.

Language-server lifecycle:

- `LS-1` (12.3.4; integration): A slot is a language and a project root owned by
  the workspace; a nested toolchain file starts a project root. The working
  directory, `rootUri`, and exactly one `workspaceFolders` entry equal that
  root and are always sent.
- `LS-2` (12.3.4; model): Server instances follow Table 6. A slot holds at most
  one current and one outgoing instance; while one is outgoing, a stale current
  instance grants nothing and requests receive `server_restarting`.
- `LS-3` (12.3.4; integration): An instance is ready only after the healthy
  signal its adapter declares; completing `initialize` is not enough. A
  not-ready signal, or a committed project-model change, returns it to warming.
- `LS-4` (12.3.4; integration): A lease is granted only if the running identity
  equals the resolved one: language, command, arguments, configuration,
  declared environment, project root, and the path, file identity, size, and
  modification time of each resolved executable (for Rust: rust-analyzer,
  rustc, and cargo). Inputs inside the workspace, including absent candidates,
  are re-read at grant; inputs outside it may be at most `identity_freshness`
  old. Identity that changes again during acquisition is `identity_unstable`.
- `LS-5` (12.3.4; integration): Children receive the daemon's startup
  environment minus a scrub list (`RUSTUP_TOOLCHAIN`, `RUSTUP_TOOLCHAIN_SOURCE`,
  `CARGO`, `RUSTC`, `RUST_RECURSION_COUNT`), plus `RUSTUP_AUTO_INSTALL=0`;
  relative `PATH` entries are dropped.
- `LS-6` (12.3.4; integration): The Rust toolchain is the parent of the `bin`
  directory that `rustup which rustc` prints in the project root, run with that
  environment and no `RUSTUP_TOOLCHAIN`. That absolute path is then set as
  `RUSTUP_TOOLCHAIN` for `rustup which rust-analyzer`, whose result is launched
  by path, and for the child. A missing toolchain or component, or rustup older
  than 1.28.1, is `unavailable`. User scope may name an executable instead,
  including on hosts without rustup.
- `LS-7` (12.3.4; model): Every abnormal stop counts towards backoff and the
  circuit, and no stop disables a language permanently. A stop under `MU-10` is
  abnormal only if it missed a deadline.
- `LS-8` (12.3.4; integration): The client advertises only what it implements,
  in particular no dynamic watched-file registration without a watcher, and
  answers every server request. External edits a server does not watch may stay
  unseen.
- `LS-9` (12.3.4; integration): Auxiliary processes are never reused, have their
  own budget, run under a workspace lease, and receive an explicit directory,
  environment, and execution identity from a single-threaded launcher.

Mutation isolation:

- `MU-1` (12.3.5; model): A plan made by Weaver records the revision and the
  digests of everything it read. Reads and server queries during planning hold
  shared coordination; a plugin given its content does not. Until `MU-3` is
  implemented, the exclusive lease spans planning.
- `MU-2` (12.3.5; model): A request waiting for workspace coordination holds
  none, and never upgrades it.
- `MU-3` (12.3.5; model): At exclusive grant the plan's generation and revision
  must be current, and its digests must match; otherwise it is replanned at
  most once, then refused as `stale_base`. A client-authored plan carries no
  revision and matches its expected content.
- `MU-4` (16.1.6; unit): Expected target state is either absent, or a file with
  digest, mode, and kind.
- `MU-5` (12.3.5; model): A mutation reserves its whole server and formatter set
  under the exclusive lease, before staging, and pins it; a lost server or a
  changed identity before commit entry aborts it with no effect.
- `MU-6` (16.1.5; model): Files no adapter claims need no semantic verdict. For
  claimed files the verdict is `inconclusive` if a pinned server was not ready
  throughout, or if the plan contains a deletion, move, or project-model change
  its adapter cannot represent. 16.1.5 decides whether policy permits
  committing on `inconclusive`; until then it refuses.
- `MU-7` (12.3.5; model): Identity is resolved again after the last verification
  response and before commit entry; a later change does not invalidate the
  verdict.
- `MU-8` (16.1.6; model): The commit section contains only filesystem work,
  cannot be cancelled, rechecks each target immediately before replacing it,
  creates without replacing, and on failure restores only files that still hold
  its bytes; the outcome is committed, none, or unknown.
- `MU-9` (16.1.6; review): Commit is per-file replacement with best-effort
  rollback and is not described as atomic across files.
- `MU-10` (12.3.5; model): Before the exclusive lease is released, within one
  `restore_deadline` for the whole set that also covers writes: on commit, each
  serving instance holds an overlay equal to the committed bytes for each
  created or replaced path, answers a barrier, and then closes it; on abort,
  each instance the mutation staged is changed to the baseline, answers a
  barrier, and then closes. Deletions, moves, and aborted creations need an
  adapter witness that the content took effect. Starting and warming instances
  queue the committed paths and grant no lease until this is done. Any instance
  that cannot comply is stopped: its transport is closed and it serves nothing
  more.
- `MU-11` (12.3.5; unit): A committed mutation reports success even when `MU-10`
  stopped servers.
- `MU-12` (12.3.5; model): A revision is an opaque token of daemon nonce,
  generation, and counter, and changes whenever a commit section may have
  replaced anything, including one that failed. It does not see external edits,
  so caches also key on content.
- `MU-13` (12.3.5; model): `shutdown_deadline` cancels work before commit entry
  and waits for commit sections; forced termination is outside the guarantee.
- `MU-14` (12.3.5; review): Isolation is guaranteed within one daemon and one
  workspace. Content checks are the only guard between daemons, and servers of
  an enclosing workspace may lag a nested workspace's commits.

Bounded admission:

- `AD-1` (12.3.5; integration): Connection permits are bounded, with deadlines
  for reading the whole request and writing the whole response; exhaustion
  sends a diagnostic before closing whenever the transport can.
- `AD-2` (12.3.5; model): Daemon and workspace permits are taken together or
  refused, never awaited; a workspace's second or later permit is granted only
  if the permits left after the grant are at least the number of workspaces
  allowed but holding none.
- `AD-3` (12.3.5; unit): Waiting and running requests share the admitted limit
  and are reported separately.
- `AD-4` (12.3.5; model): Coordination is first in, first out; a later
  conflicting request never overtakes, and running requests stay within
  `workspace_max_running`.
- `AD-5` (12.3.5; model): A request waits only for a strictly higher rank than
  any it holds, and every wait has a deadline.
- `AD-6` (12.3.5; model): Process slots for a phase are claimed and leased in
  one step, or refused; idle victims are part of the claim and are reaped within
  `kill_deadline`; unreaped claims count against every budget and are released
  if their owner leaves; a starting instance counts as leased.
- `AD-7` (12.3.5; model): One key, including instances of its earlier
  generations, cannot hold the whole process budget; a set larger than its
  share is refused as `server_set_too_large`.
- `AD-8` (12.3.5; integration): Only the connection permit is held while the
  response is written.
- `AD-9` (12.3.5; unit): Each server has bounded queued and in-flight requests;
  exhaustion is `server_capacity`.

Reasons, configuration, and observability:

- `RC-1` (12.3.2; unit): Every refusal has a class, a code, and an effects
  value; clients act on the class, tolerate unknown codes, and codes are never
  repurposed.
- `RC-2` (12.3.5; model): A retryable class is produced only by a run that never
  staged, with effects none.
- `RC-3` (12.3.5; unit): A retry hint is optional and at most `retry_hint_max`.
- `RC-4` (12.3.2; review): The wire form preserves `RC-1` to `RC-3`;
  localization and exit status remain 13.2.2's.
- `CF-1` (12.3.2; integration): Neither binary reads daemon-scope settings,
  including `daemon_socket`, from a project file or a file it extends.
- `CF-2` (12.3.3; integration): Workspace-scope values layer the project file,
  found with the root as the project root, over daemon-scope values of the same
  name. Path selectors and other environment or command-line layers of the
  daemon process do not apply per workspace. An invalid file affects that
  workspace only, as `workspace_config_invalid`.
- `CF-3` (12.3.5; unit): The relations under `Configuration vocabulary` are
  validated before any request is admitted.
- `CF-4` (12.3.3, 12.3.4, and 12.3.5; review): The task that uses a setting
  chooses and justifies its default, and decides whether it has a flag, with
  localized help from the existing catalogues.
- `CF-5` (12.3.2; unit): Daemon-scope settings apply to every client; a client
  cannot change them in a running daemon.
- `OB-1` (12.3.5; unit): Transitions and saturation emit `tracing` events under
  provisional names until 13.4.1; 12.3 adds no metrics.
- `OB-2` (12.3.3; unit): Telemetry identifies a workspace by a process-local
  ordinal and generation, never by path, and existing path-bearing events are
  changed; an error may repeat its own client's locator.

### Reason vocabulary

| Class             | Retryable | Codes                                                                                                                                                                                                                                                          |
| ----------------- | --------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `invalid_request` | no        | `protocol_unsupported`, `workspace_missing`, `workspace_malformed`, `workspace_inaccessible`, `workspace_not_utf8`, `workspace_unmarked`, `workspace_untrusted`, `workspace_view_mismatch`, `topology_unsupported`, `target_not_owned`, `endpoint_not_private` |
| `unavailable`     | no        | `toolchain_missing`, `component_missing`, `toolchain_untrusted`, `resolver_failed`, `server_start_failed`, `server_circuit_open`, `workspace_create_failed`, `workspace_config_invalid`, `server_set_too_large`, `identity_unstable`                           |
| `overloaded`      | yes       | `connection_capacity`, `daemon_capacity`, `workspace_capacity`, `workspace_limit`, `server_capacity`, `process_budget`, `auxiliary_budget`, `queue_deadline`                                                                                                   |
| `transitional`    | yes       | `workspace_draining`, `workspace_alias`, `workspace_replaced`, `daemon_stopping`, `server_restarting`                                                                                                                                                          |
| `stale`           | no        | `stale_base`, `execution_identity_changed`, `server_lost` (mutations; a read receives `server_restarting`)                                                                                                                                                     |
| `cancelled`       | no        | `client_disconnected`, `shutdown`, `request_deadline`                                                                                                                                                                                                          |
| `commit`          | no        | `commit_rolled_back` (effects none), `commit_incomplete` (effects unknown)                                                                                                                                                                                     |
| `internal`        | no        | `internal`                                                                                                                                                                                                                                                     |

_Table 2: Reason classes and codes._

`stale` means "plan again"; repeating the same bytes is not useful. Every class
except `commit` and `internal` implies effects none. A verification verdict
(`failed`, `unavailable`, or `inconclusive`) is a result, not a refusal, and
belongs to 16.1.x.

### Configuration vocabulary

Names describe daemon-scope settings and are ratified as concepts; the owning
task fixes spelling. Limits: `connection_max`, `daemon_max_admitted`,
`workspace_max`, `workspace_max_admitted`, `workspace_max_running`,
`server_queue_max`, `server_in_flight_max`, `process_budget`,
`workspace_process_max`, `auxiliary_process_budget`. Durations: `read_deadline`,
`write_deadline`, `queue_deadline`, `request_deadline`, `creation_deadline`,
`resolver_deadline`, `start_deadline`, `restore_deadline`, `kill_deadline`,
`shutdown_deadline`, `identity_freshness`, `workspace_idle`, `server_idle`,
`backoff_delay`, `circuit_delay`, `retry_hint_max`. Counts: `restart_limit`,
`timeout_limit`, `healthy_reset`. Trust: `allowed_workspace_roots`, adapter
commands, additional environment, toolchain override. Workspace scope holds
only settings that select no program: capability overrides and adapter options
of that kind. The existing `daemon_socket`, `log_filter`, and `log_format` are
daemon scope, and `locale` belongs to the client.

Relations: `workspace_max <= daemon_max_admitted < connection_max`;
`1 <= workspace_max_running <= workspace_max_admitted`;
`workspace_max_admitted <= daemon_max_admitted - workspace_max + 1`, which
`AD-2` implies anyway; `1 <= workspace_process_max < process_budget`; every
duration is positive and finite; `queue_deadline < request_deadline`; and
`restore_deadline + kill_deadline <= shutdown_deadline`.

### Dispositions

| Open item                                  | Disposition                                                                                                   |
| ------------------------------------------ | ------------------------------------------------------------------------------------------------------------- |
| RFC Q1, ADR 0008: locator source, override | Invocation directory, `WS-1`; any option changes only the start. Its spelling is 12.3.2's, with ADR 0007      |
| RFC Q2, ADR 0008: nested repositories      | `WS-2`, `WS-4`; worktrees and submodules are separate workspaces                                              |
| RFC Q3, ADR 0009: environment allowlists   | `LS-5`: startup snapshot minus a scrub list; adapters declare which variables enter the fingerprint           |
| RFC Q4, ADR 0009: freshness                | `LS-4`; value chosen by 12.3.4                                                                                |
| RFC Q5, ADRs 0008 to 0010: budgets, limits | `CF-3`, `CF-4`                                                                                                |
| RFC Q6, ADR 0010: retry guidance           | `RC-3`; optional numeric hint                                                                                 |
| RFC Q7, ADR 0010: shadow-server threshold  | Deferred to a new ADR; trigger: 16.1.5 cannot represent a delete or move by overlay, or measured read latency |
| RFC Q8, ADR 0008: aliases, identity        | `WS-3`, `WS-5`; hard links and bind-mounted subtrees are a stated limit guarded by content checks             |
| ADR 0009: related nested project roots     | `LS-1`: one server per project root in the first implementation                                               |
| ADR 0010: fairness, cancellation           | `AD-4`, `MU-8`, `MU-13`                                                                                       |
| RFC §6.8: eviction order                   | Least recently used, ties by slot order; 12.3.4                                                               |
| RFC §6.9: symlink policy                   | Canonicalize the start; below the root, `WS-6`; 12.3.2                                                        |
| RFC §8.1: vocabulary                       | Table 2, `Configuration vocabulary`, Tables 5 and 6; wire shape is 12.3.2's                                   |
| RFC invariants 3 and 6                     | `LC-2`; `AD-1`, `AD-2`                                                                                        |

_Table 3: Disposition of every open item._

## Outcomes & retrospective

Not started. Before completion, reconcile every implementation discovery with
the upstream artefacts in `Conformance basis`, and record whether the model
falsified any clause.

## Context and orientation

Inspected revision: `fe56d226`.

### How the daemon works today

`weaverd` captures its directory in `crates/weaverd/src/process/launch.rs:148`
and passes it to `DispatchConnectionHandler::new`. Every request takes the one
mutex in `BackendManager::with_backends`
(`crates/weaverd/src/dispatch/backend_manager.rs:34`) for its whole duration,
including plugin runs; its only production caller is
`crates/weaverd/src/dispatch/handler/mod.rs:176`. Issues #130 and #138 ask for
that mutex to be narrowed. It is also the only thing that keeps readers from
seeing staged documents, so narrowing it before 12.3.5 would be unsafe. This
plan does not close those issues.

`LspHost` (`crates/weaver-lsp-host/src/host/mod.rs:118`) keeps one session per
language. `ProcessLanguageServer` starts bare command names with the daemon's
directory and environment, and `initialize` sends no root
(`crates/weaver-lsp-host/src/adapter/trait_impl.rs:43`). One request is in
flight per server, server requests go unanswered, and a language whose server
times out or exits stays dead until the daemon restarts. The semantic lock
(`crates/weaverd/src/dispatch/act/apply_patch/semantic_lock.rs:154`) opens,
changes, checks, and closes each modified document. Commit is per-file rename
with best-effort rollback
(`crates/weaverd/src/safety_harness/transaction/commit.rs:81`). Nothing checks
a digest or a revision.

The listener allows 128 handler threads and drops further connections silently
(`crates/weaverd/src/transport/listener.rs:308`). Handlers are detached, and
the shutdown deadline is logged but not enforced. `weaverd.health` is a file,
so daemon status does not use the socket. `Config` discovers project files from
the startup directory (`crates/weaver-config/src/lib.rs:107`).

### Documents in scope

- New: `docs/multi-workspace-daemon-design.md`.
- `docs/rfcs/rfc-0002-multi-workspace-daemon-and-language-server-lifecycle.md`.
- `docs/adrs/adr-0008-workspace-scoped-daemon-tenancy.md`,
  `docs/adrs/adr-0009-workspace-scoped-language-server-lifecycle.md`, and
  `docs/adrs/adr-0010-workspace-local-concurrency.md`.
- `docs/weaver-design.md` §§2.1.12, 2.3.1, 2.3.3 and the workspace-manager text
  that follows it, 2.1.7, 3.1, 3.1.1, 4.2.2, 4.2.3, 4.3.6, and 4.3.7.
- `docs/rfcs/rfc-0003-sempai-query-to-selector-vertical-slice.md` §6.9 and
  `docs/adrs/adr-0012-versioned-selector-streams.md`, for cross-references only.
- `docs/roadmap.md`, `docs/contents.md`, `docs/repository-layout.md`,
  `docs/developers-guide.md`, `docs/users-guide.md`, and
  `docs/formal-verification-methods-in-weaver.md`.

### Abstraction sweep

A search of the workspace found no state-space explorer, lease, registry,
admission, or queue abstraction to reuse. The nearest gate crate,
`weaver-docs-gate`, validates the OrthoConfig boundary manifest; its types,
errors, and remediation text are specific to that purpose.

### Budget evidence

| Plan   | Limit set                                  | Outcome                                |
| ------ | ------------------------------------------ | -------------------------------------- |
| 12.1.1 | 12 documents, 6 Rust files                 | Completed; no breach recorded          |
| 12.1.2 | 25 files, 1,200 lines                      | 35 files in the squash; net code 1,441 |
| 5.2.1  | 12 source files, 380 lines                 | Completed; no breach recorded          |
| 13.2.1 | Draft raised from 4,420 to 5,930 in review | Not yet implemented                    |

_Table 4: Recorded budgets of comparable plans._

Earlier plans under-counted tests, behavioural steps, and private-item
documentation; 12.1.2 also recorded a 116-file pull request as a breach. The
first probe was 184 dense Python lines that omitted six areas. Two reviewers
judged the draft's 2,000-line model estimate too low (2,800 to 3,800). The
estimate is now 3,550 over four milestones with 20 per cent contingency, and
splitting the model into kernels allows a stop after any of them. Documentation
is estimated at about five lines for each of 64 clauses, plus the tables, proof
sketches, and the dependent documents. 12.1.1's checking crate, with files of
530 and 401 lines, shows that the 400-line limit needs early decomposition.

### Test facilities and gates

`rstest` 0.28, `rstest-bdd` 0.5, `proptest` 1.12, `googletest` 0.14.4, and
`pretty_assertions` 1.4.1 are locked. Kani 0.67 and `prover-tools` are on the
host, but the repository configures neither Kani nor Verus. Gates:
`make check-fmt`, `make lint`, `make test`, `make markdownlint` (which runs
`make spelling`), and `make nixie`. Run them one at a time, each through `tee`
to `/tmp/<action>-weaver-<branch>.out`, preferably by the `scrutineer` agent.

### Documents and skills to consult

Documents: those in scope, and
`docs/adrs/adr-0005-verification-trust-boundary.md`,
`docs/adrs/adr-0007-agent-native-command-surface.md`,
`docs/rfcs/rfc-0001-local-daemon-observability.md`,
`docs/rust-testing-with-rstest-fixtures.md`, `docs/rstest-bdd-users-guide.md`,
`docs/rust-doctest-dry-guide.md`,
`docs/reliable-testing-in-rust-via-dependency-injection.md`,
`docs/complexity-antipatterns-and-refactoring-strategies.md`,
`docs/ortho-config-users-guide.md`, and `docs/documentation-style-guide.md`.

Skills: `sdlc-implementation`, `execplans`, `rust-router`, then
`rust-state-machines`, `rust-unit-testing`, `proptest`, `rust-verification`,
`verus` for the sketches, and `arch-crate-design`; `hexagonal-architecture`;
`codegraph-mcp`; `arch-decision-records`; `domain-cli-and-daemons`;
`addressing-whitaker-findings`; `nextest`; `firecrawl-mcp`;
`en-gb-oxendict-style`; `commit-message`; `pr-creation`; `pr-babysitting`.

### Prior art consulted

- Rustup book and the 1.28 and 1.29 changelogs: precedence, auto-install, and
  the `rust-analyzer` proxy fallback.
- Language Server Protocol 3.17: workspace folders, document ownership,
  ordering.
- Watchman `watch-project`: resolve the real path, then walk up to a marker.
- `lspmux` (`src/instance.rs`): instances keyed by command, arguments,
  environment, and root identity; its own comment says that holding the map
  lock during `initialize` blocks every client, which supports `LC-2`.
- AIP-194 and gRPC status guidance: exhausted capacity is retryable only when
  short-lived and free of side effects, which supports `RC-2`.

## Conformance basis

There is no Terms of Reference. Upstream artefacts at `fe56d226`: roadmap
12.3.1 to 12.3.6; RFC 0002 and ADRs 0008 to 0010, all `Proposed`;
`docs/weaver-design.md` as listed; RFC 0001; RFC 0003; ADR 0005; and ADR 0012.

- `REQ-ACCEPT`: RFC 0002 and ADRs 0008 to 0010 are accepted.
- `REQ-CONSISTENT`: no unresolved contradiction between identity, server
  lifecycle, mutation isolation, and admission.
- `REQ-VOCAB`: RFC §8.1 vocabulary is fixed, or its owner is named (D4).
- `REQ-OPEN`: every open item has a disposition (Table 3).
- `REQ-TRACE`: every clause has an owner and a kind of evidence.
- `GAP-DRIFT`: statements about the current state that are not true.

```plaintext
REQ-VOCAB, REQ-OPEN, GAP-DRIFT -> EP-M1 -> contract document, RFC, ADRs
REQ-CONSISTENT (identity)      -> EP-M2 -> registry, permit, ownership kernels
REQ-CONSISTENT (the rest)      -> EP-M3 -> coordination, server, mutation kernels
REQ-CONSISTENT                 -> EP-M4 -> fault matrix, behavioural scenarios
REQ-CONSISTENT, GAP-DRIFT      -> EP-M5 -> reconciled documents
REQ-TRACE, REQ-ACCEPT          -> EP-M6 -> reference check, statuses
```

## Verification plan

### Abstract model

Five kernels, each a small transition system with pure decision functions.
There are no threads, files, or clocks. A deadline is a step that is always
enabled once its wait has begun. Environment steps, each bounded: execution
identity change, file edit, server exit, server hang, client disconnection,
root replacement, marker addition or removal, adapter loading, commit
input/output failure, rollback failure, and shutdown.

- `K-REGISTRY`: entries, generations, counts, and the set of leases as ghost
  state (state used only to state properties).
- `K-PERMIT`: permits held per workspace, the daemon limit, and the number of
  workspaces allowed.
- `K-COORDINATION`: one workspace's queue, holders, and grant history.
- `K-MUTATION`: requests in two workspaces, server instances, the process
  budget, staged content, revisions, and file versions. It takes coordination
  as the specification that `K-COORDINATION` establishes.
- `K-OWNERSHIP`: path component sequences and a set of marked directories.

History that a property needs is an explicit ghost field: lease grants, queue
arrivals, the writer of each file version, when an adapter really finished
loading, the last identity and target observations, and whether a run ever
staged. A fault is one field of `Policy`, and `steps` calls the decision
functions rather than repeating them. Kernels are composed only through the
assumption that `K-MUTATION` makes about coordination; that is a residual gap of
`T-RANK`.

| From       | To         | When                                                                 |
| ---------- | ---------- | -------------------------------------------------------------------- |
| `absent`   | `creating` | first request; daemon not stopping                                   |
| `creating` | `ready`    | initialized; published under the registry lock                       |
| `creating` | `absent`   | failure, deadline, shutdown, replaced, or unmarked; all waiters told |
| `ready`    | `absent`   | count zero, and idle, over limit, replaced, unmarked, or shutdown    |
| `ready`    | `draining` | count above zero, and replaced, unmarked, or shutdown                |
| `draining` | `absent`   | last lease released                                                  |

_Table 5: Workspace transitions. A later `creating` is a new generation._

| From                  | To         | When                                                                      |
| --------------------- | ---------- | ------------------------------------------------------------------------- |
| `absent`              | `starting` | slot reserved; not in backoff; circuit closed                             |
| `starting`            | `warming`  | `initialize` completed                                                    |
| `warming`             | `ready`    | the adapter's healthy readiness signal, and `MU-10` queue applied         |
| `ready`               | `warming`  | not-ready signal, or a committed project-model change                     |
| `starting`, `warming` | `stopping` | deadline, exit, identity change, workspace draining, shutdown, or `MU-10` |
| `ready`               | `draining` | identity stale, idle eviction, workspace retirement, or shutdown          |
| `ready`, `draining`   | `stopping` | exit, transport failure, `timeout_limit`, or `MU-10`                      |
| `draining`            | `stopping` | no leases remain                                                          |
| `stopping`            | `absent`   | child reaped; only now is the process slot free                           |

_Table 6: Server instance transitions. An abnormal stop increments the slot's
failure count; `healthy_reset` completions clear it; `restart_limit` failures
open the circuit for `circuit_delay`._

Wait order. A request waits only for a rank strictly higher than any it holds:
0 connection permit; 1 daemon and workspace permits (refused, not awaited); 2
workspace lease, including creation; 3 coordination; 4 server set, by
reservation. Queue positions order grants within rank 3 and never justify a
wait. Waits for the supervisor (start, reap, barrier, resolver) are bounded by
their own deadlines and never depend on a request; only requests cause starts,
and a request ends when its start fails. The registry lock is a leaf.

Mutation phases: `planning`, `planned` (no coordination held), `queued`, `held`,
`reserved`, `baseline`, `staged`, `verified`, `committing`, `rolling_back`,
`publishing`, `restoring`, then `done` with an outcome and an effects value.
Reads: `queued`, `held`, `reserved`, `serving`, `done`.

Precondition of every run: the relations under `Configuration vocabulary`. A
violation is a configuration error reported before admission.

### Representation relationships

| Model                      | Production state (later task)                | Relationship to establish there                            |
| -------------------------- | -------------------------------------------- | ---------------------------------------------------------- |
| entry generation and count | registry entry (12.3.3)                      | count equals live lease guards; a guard names one entry    |
| ghost lease set            | lease guard values                           | each guard decrements once, on drop, under the lock        |
| permits per workspace      | admission counters (12.3.5)                  | the sum equals requests past parsing and not answered      |
| queue and holders          | coordinator (12.3.5)                         | position is arrival order; cancelled entries are removed   |
| server instance state      | supervised child (12.3.4)                    | `absent` exactly when the child has been reaped            |
| staged flag                | overlay known to the session (12.3.5)        | clear exactly when `MU-10` completed or the server stopped |
| file version               | digest, mode, kind, or absence (16.1.6)      | equal versions imply equal expected state                  |
| revision                   | token of nonce, generation, counter (12.3.3) | changes with every Weaver commit                           |
| marked directory           | `.git` or `.weaver-root` entry (12.3.2)      | evaluated beneath the held root handle                     |

_Table 7: What later conformance tests must relate._

### Invariants and lemmas

Every `INV` is checked by a monitor that reads state, steps, and ghost fields,
not the policy.

- `INV-COUNT` (`LC-1`): the count equals the number of outstanding leases.
- `INV-GEN` (`LC-1`, `LC-3`, `LC-4`, `LC-6`, `LC-7`): one live generation per
  key; every lease is on it and was granted while it was ready; a starting or
  warming instance belongs to a generation that is live.
- `INV-WAIT` (`LC-5`): creation waiters all receive one outcome, independent of
  any client, and no entry is visible before it is initialized.
- `INV-PERMIT` (`AD-2`): with limit `D`, `N` workspaces allowed, `F` free
  permits, and `A` workspaces holding any, `F >= N - A` and `F >= 0`.
- `INV-EXCL` (`AD-4`): an exclusive holder is alone.
- `INV-RUN` (`AD-4`): running requests stay within their limit.
- `INV-FIFO` (`AD-4`): no grant precedes an earlier, uncancelled, conflicting
  arrival.
- `INV-RANK` (`AD-5`, `AD-6`, `MU-2`): each wait is for a rank strictly above
  everything held, and the explicit wait-for graph is acyclic.
- `INV-BUDGET` (`AD-6`, `AD-7`): instances not `absent` plus unreaped claims
  never exceed the budget, nor a key's share.
- `INV-SERVER` (`LS-2`, `LS-7`): a slot has at most one current and one outgoing
  instance, only Table 6 transitions occur, and `starting` stays reachable.
- `INV-READY` (`MU-6`): a passing verdict uses servers that had really loaded
  throughout, and no passing verdict covers a plan its adapter cannot represent.
- `INV-STAGE` (`MU-1`, `MU-10`): no query is answered by an instance holding
  another request's staged content, and no serving instance holds any after its
  exclusive lease ends.
- `INV-SYNC` (`MU-10`): a read served after a commit is served by an instance
  that held the committed content at a barrier, or started after it.
- `INV-OWN` (`WS-4`): at commit entry the workspace owns every target.
- `INV-PIN` (`MU-5`, `MU-7`): every commit entry is preceded, after the last
  verification response, by an observation of the whole set equal to baseline.
- `INV-READ` (`MU-3`, `MU-12`): at exclusive grant, no other Weaver mutation has
  written anything the plan read since it was read.
- `INV-BASE` (`MU-8`): every replacement is immediately preceded by an
  observation equal to the expected state, and rollback overwrites only this
  commit's bytes; only an edit inside `AXIOM-WINDOW` can be lost.
- `INV-RETRY` (`MU-5`, `RC-2`): a retryable outcome has effects none and its run
  never entered `staged`.
- `INV-EFFECT` (`MU-8`, `MU-13`): before `committing`, effects are none; after
  it, cancellation and shutdown do not change the outcome, and effects are none
  only if every replaced file was restored.
- `L-FIRST` (`AD-2`): while fewer than `N` workspaces hold permits, a workspace
  holding none is not refused a daemon or workspace permit. It may still be
  refused as `workspace_limit` while every registry entry is pinned. Sketch:
  from `INV-PERMIT`, `A < N` gives `F >= 1`. The invariant holds initially
  because `D >= N`; a first permit lowers both sides by one; a later permit is
  granted only if `F - 1 >= N - A`; a release raises `F` by one and `N - A` by
  at most one.
- `L-TERMINATE` (`AD-5`, `MU-10`): with environment steps exhausted, the system
  graph is acyclic, and in every sink each request is done, each entry and
  instance is `absent` or `ready`, and nothing is staged. It is checked twice:
  with deadline steps, and again with them disabled and no hang or exit, so
  that a deadline cannot hide a deadlock.
- `L-SAFE-RETRY` (`RC-2`): repeating a retryable request cannot repeat an
  effect of the first attempt, from `INV-RETRY` and `INV-EFFECT`.
- `L-OWNER` (`WS-2`, `WS-4`): the owner is marked, is the path or an ancestor,
  and no marked path lies strictly between; it is undefined exactly when
  neither the path nor any ancestor is marked.
- `L-REFINE` (`WS-4`): adding a marker `m` changes the owner only of `m` and
  paths beneath it whose owner was above `m` or undefined, and changes it to
  `m`.

Connection capacity and process budget have no per-workspace first-come
guarantee beyond `AD-7`, and refused requests have no retry fairness. These are
stated limits, not lemmas.

### Axioms (trusted, not verified here)

- `AXIOM-LSP`: messages on one transport are delivered in order, and document
  content is client-owned until closed. Processing order is _not_ assumed;
  hence barriers and adapter conformance tests.
- `AXIOM-RUSTUP`: `rustup which` fails when the component is missing, and
  `RUSTUP_AUTO_INSTALL=0` prevents installation (1.28.1 or later; observed with
  1.29.3). 12.3.4 exercises the real tool.
- `AXIOM-FS`: renaming one file is atomic for observers on one filesystem; a
  held directory handle prevents reuse of its identity.
- `AXIOM-BARRIER`: a barrier response implies that earlier overlay
  notifications on that transport were applied. Conformance-tested per adapter
  in 12.3.4; an adapter that fails is stopped instead. It says nothing about
  reloads from disk.
- `AXIOM-WINDOW`: an external edit, or a marker change, between a recheck and
  its rename cannot be detected.
- `AXIOM-KILL`: a reaped child holds no documents.
- `AXIOM-SCOPE`: small scenarios are representative. This is a hypothesis and
  the main residual gap of bounded exploration.

### Methods per obligation

- Obligation: `INV-COUNT`, `INV-GEN`, `INV-WAIT`, `INV-PERMIT`, `L-FIRST`,
  `INV-EXCL`, `INV-RUN`, and `INV-FIFO`. Method: exhaustive exploration of
  reachable states, and an inductiveness check over _every_ state in a bounded
  domain that satisfies the invariant. Rationale: the second finds invariants
  too weak to support a later proof; `INV-GEN` without `INV-COUNT` fails it.
  Domain: two keys, three requests; `N <= 4`, `D <= 8`; queues of four.
  Artefacts: `tests/registry.rs`, `tests/permits.rs`, `tests/coordination.rs`;
  `proptest` extends permits and queues beyond the bound.
- Obligation: the remaining `INV`, `L-TERMINATE`, and `L-SAFE-RETRY`. Method:
  exhaustive exploration with acyclicity and sink checks. Domain: a scenario
  table in which each row names the conflict it must contain (at least: two
  planners in one workspace; a planner and a reader; two mutations over a tight
  budget in two workspaces; a cold start during a mutation; a hang at each
  phase; a commit failure with and without rollback failure; shutdown at each
  phase). Artefact: `tests/mutation.rs`. Evidence: no violation, exact state
  counts, and that every phase and environment step occurred; `stale_base`
  between two Weaver mutations and effects unknown must both be reached.
- Obligation: `L-OWNER`, `L-REFINE`. Method: exhaustive over every marker set on
  trees of depth two and branching two, and on chains of depth six; `proptest`
  beyond. Oracle: the set-based definition, not a walk. Artefact:
  `tests/ownership.rs`. Excluded: symlinks, ownership, mounts, hard links,
  races.
- Obligation: each replaced rule is really broken. Method: the fault matrix,
  Table 8. Artefact: `tests/faults.rs`. Each row changes one rule; the named
  invariant is checked alone, so that an earlier violation cannot hide it; the
  ratified control passes in the same scenario.
- Obligation: clause references resolve. Method: reference check. Artefact:
  `tests/references.rs`. See `EP-M6` for its limits.
- Obligation: `integration`, `unit`, and `end-to-end` clauses. Method: none
  here; the owner task supplies them. `review` clauses are read by the approver.

| Clause  | Fault (one rule negated)                   | Must fail                   |
| ------- | ------------------------------------------ | --------------------------- |
| `WS-2`  | outermost marker                           | `L-OWNER`                   |
| `WS-2`  | starting directory as root                 | `L-OWNER`                   |
| `WS-4`  | new marker applied to the whole tree       | `L-REFINE`                  |
| `WS-4`  | ownership checked only at grant            | `INV-OWN`                   |
| `LC-1`  | look up, then count                        | `INV-GEN`                   |
| `LC-1`  | release a lease twice                      | `INV-COUNT`                 |
| `LC-3`  | new generation while draining              | `INV-GEN`                   |
| `LC-4`  | retire without reading the count           | `INV-GEN`                   |
| `LC-5`  | publish before initialization              | `INV-WAIT`                  |
| `LC-5`  | creator's disconnection fails the waiters  | `INV-WAIT`                  |
| `LC-6`  | a draining workspace grants a lease        | `INV-GEN`                   |
| `LC-7`  | a starting instance holds no lease         | `INV-GEN`                   |
| `LS-2`  | replace again while one is outgoing        | `INV-SERVER`                |
| `LS-7`  | an exited server stays current for ever    | `INV-SERVER`                |
| `MU-1`  | planning query without coordination        | `INV-STAGE`                 |
| `MU-2`  | upgrade shared to exclusive                | `INV-RANK`                  |
| `MU-3`  | no revision check                          | `INV-READ`                  |
| `MU-5`  | reserve formatter slots after staging      | `INV-RETRY`                 |
| `MU-5`  | identity read per step, not pinned         | `INV-PIN`                   |
| `MU-6`  | `initialize` means ready                   | `INV-READY`                 |
| `MU-6`  | pass a plan the adapter cannot represent   | `INV-READY`                 |
| `MU-7`  | no check after verification                | `INV-PIN`                   |
| `MU-8`  | check once before the section              | `INV-BASE`                  |
| `MU-8`  | roll back without checking bytes           | `INV-BASE`                  |
| `MU-8`  | cancellable commit                         | `INV-EFFECT`                |
| `MU-8`  | report a failed rollback as none           | `INV-EFFECT`                |
| `MU-10` | close without restoring                    | `INV-STAGE`                 |
| `MU-10` | release the lease while a server hangs     | `INV-STAGE`                 |
| `MU-10` | notify only ready servers                  | `INV-SYNC`                  |
| `MU-10` | barrier without an overlay                 | `INV-SYNC`                  |
| `MU-10` | hold the lease until a hung server answers | `L-TERMINATE`               |
| `MU-12` | a failed commit keeps the revision         | `INV-READ`                  |
| `MU-13` | shutdown abandons a commit                 | `INV-EFFECT`                |
| `AD-2`  | reserve measured before the grant          | `INV-PERMIT`                |
| `AD-2`  | no reserve                                 | `INV-PERMIT`, `L-FIRST`     |
| `AD-4`  | readers ignore an exclusive holder         | `INV-EXCL`                  |
| `AD-4`  | readers overtake a waiting mutation        | `INV-FIFO`                  |
| `AD-4`  | ignore the running limit                   | `INV-RUN`                   |
| `AD-5`  | no deadline on a start                     | `L-TERMINATE`               |
| `AD-6`  | wait for process slots one at a time       | `INV-RANK`                  |
| `AD-6`  | free the slot before reaping               | `INV-BUDGET`                |
| `AD-7`  | ignore the key's share                     | `INV-BUDGET`                |
| `RC-2`  | retryable `commit_incomplete`              | `INV-RETRY`, `L-SAFE-RETRY` |

_Table 8: Fault matrix, keyed by the clause each fault negates. Every `model`
clause, invariant, and lemma has at least one row._

`googletest` matchers and `pretty_assertions` are used for traces and tables. No
`insta` snapshot is planned: there is no output format. No end-to-end test is
planned: no observable workflow changes, and 12.3.6 owns that suite. `EP-M0`
and each code milestone run the existing suites. `proptest` regression files
are committed if a failure is found.

### Formal verification decision

No lemma is selected for Verus or Kani in this task:

- there is no production kernel, so a proof would cover a second description
  whose correspondence rests on review;
- the interleaving obligations concern finite control state, where exploration
  reports concrete traces, and the inductiveness checks remove the main reason
  a later proof would fail; and
- the pure lemmas are small: `L-FIRST` is four lines of arithmetic above, and
  `L-OWNER` and `L-REFINE` are checked against an independent specification.

`AGENTS.md` asks for an exhaustive Verus proof of introduced lemmas or
contractual business logic. The task brief asks for selective use, with theorem
statements left in design documents until implementation reaches them. This
plan reads the first through the second; question 8 asks the approver to
confirm. `EP-M1` writes, for each theorem, the ghost state, preconditions,
strengthened invariant, measure, and key cases. These are hypotheses about
proof structure; no `assume`, `admit`, or external proof body is introduced.

| Theorem          | Statement                                      | Contract and strengthening                                     | Kernel | Tested by                  | Not covered by tests       |
| ---------------- | ---------------------------------------------- | -------------------------------------------------------------- | ------ | -------------------------- | -------------------------- |
| `T-OWNER`        | `L-OWNER`, `L-REFINE` for any path length      | post of `owner`: the set-based definition; induction on length | 12.3.2 | exhaustive, `proptest`     | paths beyond the bound     |
| `T-LIFECYCLE`    | `INV-GEN` inductive for any keys and requests  | conjoin `INV-COUNT`; sketch below                              | 12.3.3 | inductiveness check        | unbounded keys             |
| `T-PERMIT`       | `INV-PERMIT` inductive, hence `L-FIRST`        | pre `D >= N`; post of `admit` preserves it                     | 12.3.5 | exhaustive, `proptest`     | unbounded limits           |
| `T-COORDINATION` | `INV-EXCL`, `INV-FIFO` inductive for any queue | queue sorted by ticket; holders conflict-free                  | 12.3.5 | inductiveness check        | longer queues              |
| `T-RANK`         | the wait graph is acyclic                      | measure: rank, strictly increasing                             | 12.3.5 | `INV-RANK` in `K-MUTATION` | composition across kernels |
| `T-COMMIT`       | `INV-EFFECT` for any number of files           | per-file status: untouched, replaced, restored, or abandoned   | 16.1.6 | `tests/mutation.rs`        | more than two files        |

_Table 9: Theorem statements, and how they relate to tests._

Sketch of `T-LIFECYCLE`. Ghost state: a finite set `L` of leases, each with a
key and a generation, and the greatest generation ever used per key. Invariant:
at most one entry per key; its count is the number of leases in `L` with that
key; each of those has the entry's generation; no lease has a key without an
entry; and no entry exceeds the greatest generation. Lookup requires a ready
entry and adds one lease of its generation and one to the count. Release
requires membership and removes both. Retirement and the end of draining
require a count of zero, so by the invariant no lease has that key, and
removing the entry preserves it. Draining changes no lease. Creation requires
no entry, so no lease has the key, and uses a greater generation. Without the
count conjunct, retirement's step does not follow; the probe exhibits it.

Verus is warranted for `T-OWNER` and `T-LIFECYCLE` if their kernels stay pure,
and Kani first for `T-COMMIT`, as 19.3.3 says. The others are adequately served
by exploration and property tests unless their kernels grow. Roadmap 19.3.1
owns the tools and requires 16.1.1, which depends on 13.4.1 and so on 12.3.5;
the theorems are therefore candidates for 19.3.4, not obligations of 12.3.x,
unless 19.3.1 moves earlier. Identity equality, reason classes, configuration
validation, telemetry, and references need only conventional tests. A later
proof does not replace the integration tests of 12.3.6.

### Suggested BDD features

Proposed feature file:
`crates/weaver-tenancy-model/tests/features/tenancy_contract.feature`, run by
`rstest-bdd` against the kernels (`EP-M4`).

```gherkin
Feature: Multi-workspace daemon contract

  Scenario: A busy repository does not shut out another
    Given 3 daemon permits shared by 2 workspaces
    And workspace alpha holds 2 permits
    When alpha asks for another permit and then beta asks for one
    Then alpha is refused with class "overloaded"
    And beta is admitted

  Scenario: A repository in use is not retired
    Given workspace alpha is ready with one lease
    When idle retirement runs
    Then workspace alpha stays ready

  Scenario: A hung language server cannot keep staged content alive
    Given a mutation in alpha has staged content
    And the language server for alpha hangs
    When the mutation is abandoned
    Then the language server is stopped before the exclusive lease ends

  Scenario: A toolchain change during verification has no effect on disk
    Given a mutation in alpha has staged content
    When the execution identity for alpha changes
    Then the mutation ends with code "execution_identity_changed"
    And its effects are "none"

  Scenario: Another edit between planning and applying is noticed
    Given a rename in alpha was planned at the current revision
    And another mutation in alpha has since committed
    When the rename is granted the exclusive lease
    Then it is replanned or ends with code "stale_base"

  Scenario: Overload is safe to retry
    Given every process slot is leased
    When a mutation needs another language server
    Then it ends with class "overloaded"
    And its effects are "none"

  Scenario: Disconnection does not abandon a commit
    Given a mutation in alpha is committing
    When its client disconnects
    Then the commit runs to its own outcome

  Scenario: A failed rollback is reported honestly
    Given a commit in alpha fails after replacing one file
    When restoring that file also fails
    Then the mutation ends with code "commit_incomplete"
    And its effects are "unknown"

  Scenario: Directories in one repository select one workspace
    Given a repository rooted at "/work/alpha"
    When a request starts in "/work/alpha/src/lib"
    Then its workspace is "/work/alpha"

  Scenario: An unmarked directory is refused
    Given no root marker above "/scratch/notes"
    When a request starts in "/scratch/notes"
    Then it is refused with code "workspace_unmarked"
```

## Plan of work

### `EP-M0` baseline and drift check

Record the revision and each gate's result. Re-check every "today" claim whose
file changed since `fe56d226`, and the state of pull requests #260 and #453.
With scratch tests that are not committed, determine whether the sandbox
refuses plugins from the multi-threaded daemon, whether a cold `rust-analyzer`
returns empty diagnostics, and whether closing a staged document leaves staged
content visible to a later read. Raise an issue for each confirmed defect and
record its number. Correct any falsified conflict here and escalate if a clause
changes. If the baseline is red, stop.

### `EP-M1` contract document and revised decisions

Create `docs/multi-workspace-daemon-design.md`: purpose, terms, clauses, Tables
2, 3, 5, 6, and 7, wait order, configuration vocabulary, axioms, theorem
sketches, and how a clause is amended (a pull request that changes the clause,
its evidence, and any ADR whose decision it alters). Before freezing the text,
check any invariant whose statement changes by enumerating every state of its
small domain that satisfies it and confirming that each step preserves it; a
scratch script is enough, because `EP-M2` and `EP-M3` make the check permanent.

Revise RFC 0002: correct §§3 and 6.1; rewrite §§6.1 to 6.9 and 7 where a
conflict applies, citing clauses; drop the metrics requirement; replace §11
with the dispositions; extend §9 with the alternatives in D3 to D7; add the
prior art. Revise ADRs 0008 to 0010: decision outcome, consequences, and risks
cite clauses; each outstanding decision becomes a resolution or a named,
non-blocking deferral under known risks. Keep options and decision statements
as history unless a clause changes them. Status stays `Proposed`.

Validation: `make fmt`, `make check-fmt`, `make markdownlint`, `make nixie`.
The other live documents lag until `EP-M5`.

### `EP-M2` registry, permit, and ownership kernels

Create `crates/weaver-tenancy-model` (`publish = false`, no runtime
dependencies) with `explore` (breadth-first search, shortest traces, acyclicity
and sink checks, and the inductiveness check), `registry`, `permits`, and
`ownership`. Each kernel exposes its state, its decision functions, its steps,
and its monitors. Ordered collections keep counts and traces stable.

Work red then green, one rule at a time: write a fault row that expects a named
violation, which fails because nothing explores; add the smallest kernel that
produces it; add the ratified control, which fails until the rule exists. Add
the workspace member and the repository-layout entry. Module documentation
states what each kernel leaves out.

### `EP-M3` coordination, server, and mutation kernels

Add `coordination`, then `servers` and `mutation`, in the same way. Keep the
scenario table in one file, each row naming the conflict it contains. Renumber
queue tickets by position so that equivalent states coincide. Record state
counts in `Artefacts and notes`. A surviving violation of a ratified rule is a
contract defect: reword the clause in the same commit and log it, or escalate
if its obligation changes.

### `EP-M4` behavioural scenarios and the full fault matrix

Add the feature file and steps, complete Table 8, and add a test that every
`model` clause, invariant, and lemma has at least one fault row that is killed
for the stated reason.

### `EP-M5` dependent documents

- `docs/weaver-design.md`: align the listed sections and cite clauses. If #260
  has merged, leave §2.3.2 alone; otherwise add one sentence saying that its
  artefact text awaits #260.
- `docs/roadmap.md`: add clause references to the success text of 12.3.2 to
  12.3.6, 15.3.4 (content-keyed cache), 16.1.5, and 16.1.6; add conformance to
  12.3.3 and 12.3.5; note in 13.2.2 and 15.3.3 the constraints `RC-4` and
  `WS-8`; add the new document to 12.3's list; and replace "remains internal to
  the daemon protocol and runtime" with the user-visible effects of `WS-2`,
  `TR-2`, `TR-3`, and `CF-1`. Do not renumber or add tasks.
- RFC 0003 §6.9 and ADR 0012: cite `MU-12` and `WS-8`. Their status is
  unchanged.
- `docs/developers-guide.md`: where clauses live, how to amend one, the model's
  scope, reuse and retirement policy, and how to add a scenario or fault.
- `docs/users-guide.md`: state plainly that a daemon serves the directory it was
  started in, whichever directory a command is run from.
- `docs/formal-verification-methods-in-weaver.md`: list the theorem statements.
- `docs/contents.md` and `docs/repository-layout.md`: entries.

Search for stale claims with
`git grep -nE 'RFC 0002|ADR 00(08|09|10)|workspace manager' -- docs ':!docs/execplans'`.

### `EP-M6` reference check, acceptance, and closure

Add `tests/references.rs`: clause identifiers are unique; every identifier
cited in live documents is defined; each clause has an owner that exists in the
roadmap and a kind of evidence; and the set of clauses marked `model` equals
the set of clauses that killed fault rows negate. When a kernel is retired
(D3), its rows move to the owner crate and its clauses change from `model` to
`production` in the same pull request. Include synthetic documents with a
duplicate, an undefined citation, and an unknown owner, which must be rejected.
The check shows that references resolve. It cannot show that prose agrees, and
it does not pin status text; a test that pinned text was removed in pull
request #449.

If question 7 drops the model, this test is dropped too, and the references are
checked by the reviewer with `git grep`.

Then change the RFC and ADRs to `Accepted, <date>. <summary>`, update the ADR
dates and `docs/contents.md`, and mark 12.3.1 done. The approver accepts by
approving the pull request; set the date when it is ready to merge and record
the approver in `Decision log`.

## Milestones and plateaus

| Milestone | Discharges                  | Plateau                               | Remaining gap        |
| --------- | --------------------------- | ------------------------------------- | -------------------- |
| `EP-M0`   | baseline                    | Nothing tracked changes but this plan | All                  |
| `EP-M1`   | `REQ-VOCAB`, `REQ-OPEN`     | The contract exists, `Proposed`       | No machine evidence  |
| `EP-M2`   | `REQ-CONSISTENT` (identity) | Three kernels verified                | Coordination onwards |
| `EP-M3`   | `REQ-CONSISTENT` (the rest) | All kernels verified                  | Matrix incomplete    |
| `EP-M4`   | `REQ-CONSISTENT`            | Every invariant has a killed fault    | Other documents lag  |
| `EP-M5`   | `GAP-DRIFT`                 | Live documents describe one contract  | Not accepted         |
| `EP-M6`   | `REQ-TRACE`, `REQ-ACCEPT`   | Accepted                              | Implementation tasks |

_Table 10: Milestones._

At each boundary check: assigned requirements met; no production file changed;
no obligation changed without escalation; no new dependency; trace links
current. Compatibility decision for every milestone: none, because the crate is
private and the documents have no external consumer. Recovery: revert the
milestone's commits; earlier milestones stand without later ones.

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
waiting for a compile slot is normal; do not retry or kill the build. Run any
`rustup` command with `RUSTUP_AUTO_INSTALL=0` and outside directories that have
a toolchain file.

## Validation and acceptance

- `make check-fmt`, `make lint`, and `make test` pass after `EP-M2`, `EP-M3`,
  `EP-M4`, and `EP-M6`; `make check-fmt`, `make markdownlint`, and `make nixie`
  pass after every milestone that changes documentation.
- `cargo nextest run -p weaver-tenancy-model` passes; reverting any ratified
  rule makes a control fail with a named invariant.
- `git diff --stat "$(git merge-base origin/main HEAD)" -- crates` lists only
  `crates/weaver-tenancy-model`.
- Every conflict maps to a clause, and every item in Table 3 to a disposition.
- Roadmap 12.3.1 is checked; 12.3.2 to 12.3.6 are not.

There is no Verus milestone, so no Verus command is an acceptance criterion.

## Idempotence and recovery

All steps can be repeated. Model tests are deterministic. Faults are permanent
table entries, not edits to revert. `make fmt` may reflow Markdown; commit that
separately if it touches unrelated files.

## Artefacts and notes

First probe (throwaway Python, not committed). It found nine of ten faults,
with traces of at most eleven steps, and every control it ran passed. Review
showed that it did not monitor identity pinning, retry, order, ranks, limits,
commit effects, or ownership, that its identity and digest checks could not
fail in the ratified case, and that it had no planning phase. It establishes
that exploration is feasible and nothing more.

| Fault                               | Violation found               | States searched |
| ----------------------------------- | ----------------------------- | --------------- |
| Look up, then count the lease       | lease on retired generation   | 8               |
| Wait for process slots              | no progress                   | 205             |
| Hold the lease until restored       | no progress                   | 698             |
| Release without restoring           | staged content outlives lease | 229             |
| Re-read identity per step           | verdict mixes identities      | 125             |
| Upgrade shared to exclusive         | no progress                   | 161             |
| Workspace limit equals the daemon's | cross-workspace starvation    | 49              |
| No digest check                     | lost update                   | 208             |
| No exclusion                        | exclusive lease shared        | 42              |
| Overtaking queue                    | none; needs a history monitor | 119             |

_Table 11: First probe._

Later probes, after review, of the replacement rules. The second probe's
registry result was definitional, and its mutation controls could not fail,
because they compared revisions instead of file histories. These are the
results of repeating them with real transitions and history monitors:

| Rule                                     | Result                                                                             |
| ---------------------------------------- | ---------------------------------------------------------------------------------- |
| `AD-2`, reserve measured after the grant | Inductive in all 1,235 states (`N <= 4`, `D <= 8`); no first request refused       |
| `AD-2`, reserve measured before, or none | 778 non-inductive steps each                                                       |
| `INV-GEN` alone                          | 12 steps from 54 satisfying states break it                                        |
| `INV-GEN` with `INV-COUNT`               | no step from 18 satisfying states breaks either                                    |
| Nearest marker                           | no violation of `L-OWNER` or `L-REFINE` in 896 cases                               |
| Outermost marker                         | 320 violations of `L-OWNER`, 1,088 of `L-REFINE`                                   |
| Starting directory                       | 448 violations of `L-OWNER`; `L-REFINE` does not reject it                         |
| `MU-3`, `MU-8`, `MU-12` together         | 362 states, no violation; stale, none, unknown, and committed outcomes all reached |
| A failed commit keeps the revision       | 2 violations of `INV-READ`                                                         |
| No revision check                        | 14 violations of `INV-READ`                                                        |
| No per-file recheck                      | 8 violations of `INV-BASE`                                                         |
| Rollback without checking bytes          | 6 violations of `INV-BASE`                                                         |
| Failed rollback reported as none         | 6 violations of `INV-EFFECT`                                                       |
| Upgrade                                  | deadlock, and 4 waits at a rank already held                                       |
| Release, then acquire                    | no deadlock, and no such wait                                                      |

_Table 12: Repeated probes. They cover two mutations, two files, one external
edit, and one input/output failure. `MU-5` to `MU-7`, `MU-10`, `MU-13`, `AD-4`,
`AD-6`, `AD-7`, and `LS-2` have not been probed in their present form; `EP-M2`
and `EP-M3` are their first test._

Design review, 2026-11-24. Five reviewers covered concurrency, identity and
security, language servers and mutation, verification, and contracts and
delivery. The main changes: clauses moved to a design document (D2) and written
out in full; scope narrowed (D4); unmarked roots refused (D5); revision
restored (D6); cross-daemon lock removed (D7); trust clauses added (D10);
readiness, barriers, and project-model inputs added; permit rule replaced by
one with a proof; commit section corrected; false lemmas replaced; kernels,
representation table, inductiveness checks, and fault matrix added; estimates
raised. Two suggestions were not taken: deferring `TR-2` and `LS-9` wholesale,
because both close contradictions in the four documents, and committing a
formal proof now. All five reviewers then rechecked the rewrite. Their
remaining points were applied: a strictly increasing rank order, separate
deadlock and deadline checks, a fault matrix keyed by clause, ghost state,
overlay-based synchronization, rustup resolution without a circular
environment, truthful trust clauses, group-writable roots, retryable codes
never raised after staging, an ownership clause, and `MU-3` owned by 12.3.5.
Nobody has reviewed the result of that last pass, except by the probes above.

Questions for the approver:

1. D5, `WS-2`: are `.git` and `.weaver-root` the only markers, with every other
   directory refused? Users of Mercurial, Jujutsu without Git, and unpacked
   archives must add `.weaver-root`, and existing end-to-end fixtures need a
   marker when 12.3.2 lands.
2. `TR-2`: refuse roots and markers that every user can write, and
   group-writable ones unless listed? Shared checkouts and some containers must
   then be listed.
3. `TR-3`: refuse workspace selection on endpoints that are not private? This
   makes non-Unix defaults and explicit TCP unusable, and no scheduled task
   adds authentication.
4. D10: is `TR-4` (servers may run any program a repository's own build or
   server configuration names) acceptable? And is `TR-1` (Weaver itself never
   runs a program because a workspace file names it) wanted?
5. D12, `LS-5`: inherit the daemon's startup environment, so that the first
   shell's `PATH` and variables reach every repository's build, or use an
   allowlist?
6. `CF-1`: stop reading `daemon_socket` and other daemon settings from project
   `weaver.toml` files? This changes documented behaviour when 12.3.2 lands.
7. D3: build the model at this size, or ratify on review alone, which drops
   `EP-M2` to `EP-M4` and the reference test?
8. D8: leave Verus to 19.3.4, or move 19.3.1 earlier so that `T-OWNER` and
   `T-LIFECYCLE` are proved with their kernels?
9. D7, `MU-14`: is a guarantee within one daemon enough?
10. D11: is the narrower form of invariant 4 acceptable?

## Interfaces and dependencies

New private crate `weaver-tenancy-model`, with no runtime dependencies and the
development dependencies in C4. Provisional surface:

```rust,ignore
pub trait Kernel {
    type State: Clone + Ord; // includes ghost fields
    type Step: Clone + Ord;
    fn initial(&self) -> Vec<Self::State>;
    fn steps(&self, state: &Self::State) -> Vec<(Self::Step, Self::State)>;
}
pub enum Monitor<K: Kernel> {
    State(fn(&K::State) -> bool),
    Step(fn(&K::State, &K::Step, &K::State) -> bool),
    Graph(fn(&Graph<K>) -> bool),
}
pub struct Policy { /* one field per ratified rule; a fault changes one */ }
pub fn explore<K: Kernel>(kernel: &K, monitor: &Monitor<K>) -> Exploration<K>;
/// Every step from a state satisfying all hypotheses that breaks the goal.
pub fn inductive_failures<K: Kernel>(
    kernel: &K,
    domain: &[K::State],
    hypotheses: &[fn(&K::State) -> bool],
    goal: fn(&K::State) -> bool,
) -> Vec<(K::State, K::Step, K::State)>;

// Decision functions that conformance tests compare with production.
pub fn admit(permits: &Permits, workspace: Key, policy: &Policy) -> Admission;
pub fn may_retire(entry: &Entry) -> bool;
pub fn grant(queue: &Coordination) -> Vec<Ticket>;
pub fn reserve(budget: &Budget, set: &ServerSet) -> Reservation;
pub fn owner<'a>(markers: &'a MarkerSet, path: &PathSegments) -> Option<&'a PathSegments>;
```

Reuse policy, to be recorded in `docs/developers-guide.md`: the crate is an
executable specification and test oracle with no runtime consumer. Tasks 12.3.2
to 12.3.6 may depend on it for tests. It must not gain input/output, threads,
or a clock. A kernel is deleted when a pure production kernel is explored
directly with the same monitors.

In hexagonal terms the clauses describe the daemon's inner policy. Ports that
later tasks define are the workspace filesystem, server launcher, toolchain
resolver, clock, and telemetry sink; their adapters are outside this task, and
the dependency rule forbids the policy from naming them.

## Revision note

2026-11-24: initial draft, then rewritten after a five-reviewer design review.
The rewrite narrowed scope, moved clauses to a design document, wrote them out,
replaced false lemmas and their probes, and raised the estimates. A recheck by
the same reviewers led to a further revision of the clauses, monitors, fault
matrix, probes, and questions, and to one clause (`MU-12`) that a probe found.
No implementation work has started.
