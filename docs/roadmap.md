# Roadmap

## 0. Foundation & Tooling (Complete)

### 0.1. Establish foundation and documentation baseline

- [x] 0.1.1. Set up the project workspace, CI/CD pipeline, and core
      dependencies.
- [x] 0.1.2. Normalize parser and Semgrep documentation style and navigation,
      including
      `docs/contents.md` and `docs/repository-layout.md`, as delivered in
      `docs/execplans/sempai-design.md`.

## 1. Core MVP & Safety Harness Foundation

*Goal: Establish the core client/daemon architecture, basic LSP integration,
and the foundational security and verification mechanisms. The MVP must be safe
for write operations from day one.*

### 1.1. Deliver CLI and daemon foundation

*Outcome: Ship a pair of crates (`weaver-cli`, `weaverd`) that honour the
design contract in `docs/weaver-design.md` and expose the lifecycle expected by
`docs/documentation-style-guide.md`.*

- [x] 1.1.1. Define the shared configuration schema for `weaver-cli` and
      `weaverd`
      in `weaver-config`, using `ortho-config` to merge config files,
      environment overrides, and CLI flags for daemon sockets, logging, and the
      capability matrix defaults.
      - Acceptance criteria: Schema documented in crate docs, integration tests
        demonstrate precedence order (file < env < CLI), and default sockets
        align with the design doc.
- [x] 1.1.2. Implement the `weaver-cli` executable as the thin JSON Lines
      (JSONL)
      client that
      initializes configuration via `ortho-config`, exposes the
      `--capabilities` probe, and streams requests to a running daemon over
      standard IO.
      - Acceptance criteria: CLI command surface mirrors the design table,
        capability probe outputs the negotiated matrix, and JSONL framing is
        validated with golden tests.
- [x] 1.1.3. Implement the `weaverd` daemon bootstrap that consumes the shared
      configuration, starts the Semantic Fusion backends lazily, and supervises
      them with structured logging and error reporting.
      - Acceptance criteria: Bootstrap performs health reporting hooks,
        backends start only on demand, and failures propagate as structured
        events.
- [x] 1.1.4. Implement robust daemonisation and process management for
      `weaverd`,
      including backgrounding with `daemonize-me`, PID/lock file handling,
      health checks, and graceful shutdown on signals.
      - Acceptance criteria: Background start creates PID and lock files,
        duplicate starts fail fast, and signal handling shuts down within the
        timeout budget.
- [x] 1.1.5. Provide lifecycle commands in `weaver-cli` (for example,
      `daemon start`,
      `daemon stop`, `daemon status`) that manage the daemon process, verify
      socket availability, and surface actionable errors when start-up fails.
      - Acceptance criteria: Lifecycle commands call into shared helper logic,
        refuse to start when sockets are bound, and emit recovery guidance for
        the operator.

- [x] 1.1.6. Implement the socket listener in `weaverd` to accept client
      connections
      on the configured Unix domain socket (or TCP socket on non-Unix
      platforms).
      - Acceptance criteria: Daemon binds to the socket path from configuration,
        accepts concurrent connections, and gracefully handles connection errors
        without crashing the daemon.

- [x] 1.1.7. Implement the JSONL request dispatch loop in `weaverd` that reads
      `CommandRequest` messages from connected clients, routes them to the
      appropriate domain handler, and streams `CommandResponse` messages back.
      - Acceptance criteria: Request parsing rejects malformed JSONL with
        structured errors, domain routing covers `observe` and `act` commands,
        and responses include the terminal `exit` message with appropriate
        status codes.

- [x] 1.1.8. Wire end-to-end domain command execution from CLI through daemon to
      backend, starting with `observe get-definition` as the first complete
      path.
      - Acceptance criteria: `weaver observe get-definition` with a running
        daemon returns LSP definition results, errors propagate with structured
        messages, and the CLI exits with the daemon-provided status code.

- [x] 1.1.9. Build the `weaver-lsp-host` crate with support for initialization,
    capability detection, and core LSP features (definition, references,
    diagnostics) for Rust, Python, and TypeScript.

- [x] 1.1.10. Implement process-based language server adapters for
      `weaver-lsp-host`.
    The `LspHost` currently requires external callers to register
    `LanguageServer` implementations via `register_language()`. This step adds
    concrete adapters that spawn real language server processes (e.g.,
    `rust-analyzer`, `pyrefly`, `tsgo`).
  - Acceptance criteria: `SemanticBackendProvider::start_backend()` registers
    adapters for configured languages, adapters spawn server processes and
    communicate via stdio, server shutdown is handled gracefully on daemon
    stop, and missing server binaries produce clear diagnostic errors.

- [x] 1.1.11. Add human-readable output rendering for commands that return code
    locations or diagnostics, using `miette` or a compatible renderer to
    show context blocks.
  - Acceptance criteria: Definition, reference, diagnostics, and safety
    harness failure outputs include file headers, line-numbered source
    context, and caret spans in human-readable mode; JSONL output remains
    unchanged; missing source content falls back to path-and-range with a
    clear explanation.

- [x] 1.1.12. Implement the initial version of the `weaver-sandbox` crate, using
    `birdcage` for its focused scope and production usage, prioritising robust
    Linux support via namespaces and seccomp-bpf.

- [x] 1.1.13. Implement the full "Double-Lock" safety harness logic in
      `weaverd`.
    This is a critical, non-negotiable feature for the MVP. All `act` commands
    must pass through this verification layer before committing to the
    filesystem.
  - Acceptance criteria: Edit transactions pass through syntactic and semantic
    lock validation before commit, failures leave the filesystem untouched,
    and behaviour-driven development (BDD) scenarios cover success, syntactic
    failure, semantic failure, and backend unavailable error paths.

- [x] 1.1.14. Implement atomic edits to ensure that multi-file changes either
      succeed
    or fail as a single transaction.
  - Acceptance criteria: Two-phase commit with prepare (temp files) and commit
    (atomic renames) phases, rollback restores original content on partial
    failure, and new file creation properly tracks file existence for
    rollback.

## 2. Syntactic & Relational Intelligence

*Goal: Add the Tree-sitter and call graph layers to provide deeper structural
and relational understanding of code.*

### 2.1. Deliver syntax and graph foundations

- [x] 2.1.1. Create the `weaver-syntax` crate and implement the structural
      search
    engine for `observe grep` and `act apply-rewrite`, drawing inspiration from
    ast-grep's pattern language.

- [x] 2.1.2. Integrate the "Syntactic Lock" from `weaver-syntax` into the
    "Double-Lock" harness.

- [x] 2.1.3. Extend the `LanguageServer` trait with document sync methods
    (`did_open`, `did_change`, `did_close`) to enable semantic validation
    of modified content at real file paths without writing to disk.

- [x] 2.1.4. Create the `weaver-graph` crate and implement the LSP Provider for
      call
    graph generation, using the `textDocument/callHierarchy` request as the
    initial data source.

### 2.2. Deliver `act apply-patch` command

*Outcome: Provide a safety-locked patch application path that mirrors the
`apply_patch` semantics for agents and integrates with the Double-Lock harness.*

- [x] 2.2.1. Add JSONL request/response types and a `weaver act apply-patch`
      command
    that reads the patch stream from standard input (STDIN) and forwards it to
    the daemon.
  - Acceptance criteria: CLI streams raw patch input, returns non-zero exit
    codes on failure, and surfaces structured errors.
- [x] 2.2.2. Implement the patch parser and matcher in `weaverd` to support
      modify,
    create, and delete operations, including fuzzy matching, line-ending
    normalization, and path traversal checks.
  - Acceptance criteria: patch application is atomic per command, missing
    hunks are rejected, and parent directories are created for new files.
- [x] 2.2.3. Integrate apply-patch with the safety harness using syntactic and
    semantic locks, ensuring no on-disk writes on lock failure.
  - Acceptance criteria: Tree-sitter validates modified/new files, LSP
    diagnostics are compared against the pre-edit baseline, and failures
    leave the filesystem untouched.
- [x] 2.2.4. Add unit, BDD, and end-to-end tests covering create/modify/delete
      and
    failure paths (missing hunk, invalid header, traversal attempt).
  - Acceptance criteria: tests pass under `make test` and error messaging is
    asserted for each failure mode.

### 2.3. Deliver Semgrep-compatible query routing foundation (`sempai`)

*Outcome: Implement the hybrid Semgrep-compatible routing strategy from*
*`docs/adr-003-sempai-semgrep-compatible-query-engine.md`, with explicit*
*backend selection and diagnostics across ast-grep and Weaver-native matching.*

- [ ] 2.3.1. Define and implement the rule-capability routing matrix for
      Semgrep-style
    operators and captures.
  - Acceptance criteria: each supported operator has an explicit mapped backend
    class (`ast-grep`, `Weaver-native`, or unsupported), and routing decisions
    include stable reason codes.
- [ ] 2.3.2. Implement the Semgrep-compatible front-end normalization flow that
    produces a deterministic internal formula for routing.
  - Acceptance criteria: equivalent rule forms normalize to the same internal
    representation, and normalization failures return structured diagnostics.
- [ ] 2.3.3. Implement the ast-grep execution path for rules that map cleanly
      and
    return deterministic captures.
  - Acceptance criteria: mapped fixtures execute through ast-grep with stable
    capture output and no implicit fallback.
- [ ] 2.3.4. Implement the Weaver-native execution path for supported
      constructs that
    do not map cleanly to ast-grep.
  - Acceptance criteria: fallback execution is explicit in diagnostics and
    preserves normalized rule semantics for covered operators.
- [ ] 2.3.5. Add conformance and regression suites for mapped and non-mapped
      operator
    behaviour, including captures, negation, and deep-matching boundaries.
  - Acceptance criteria: regression fixtures cover routing parity and mismatch
    diagnostics across Rust, Python, Go, and TypeScript.
- [ ] 2.3.6. Publish user-facing compatibility boundaries and routing
      diagnostics in
    the Semgrep reference documentation.
  - Acceptance criteria: docs identify guaranteed operators, fallback-only
    operators, and unsupported constructs with stable terminology.

## 3. Plugin Ecosystem & Specialist Tools

*Goal: Build the plugin architecture to enable orchestration of best-in-class,
language-specific tools.*

### 3.1. Establish plugin platform foundation

- [x] 3.1.1. Design and implement the `weaver-plugins` crate, including the
      secure
    IPC protocol between the `weaverd` broker and sandboxed plugin processes.
    *(Phase 3.1.1 — see `docs/execplans/3-1-1-weaver-plugins-crate.md`)*

### 3.2. Deliver capability-first `act extricate`

*Outcome: Implement the `extricate-symbol` capability model and command flow*
*defined in `docs/adr-001-plugin-capability-model-and-act-extricate.md`, using*
*the Rust implementation strategy in*
*`docs/rust-extricate-actuator-plugin-technical-design.md`.*

- [ ] 3.2.1. Add capability ID scaffolding and resolver policy for actuator
    capabilities (`rename-symbol`, `extricate-symbol`, `extract-method`,
    `replace-body`, `extract-predicate`).
  - Acceptance criteria: capability IDs are strongly typed in daemon routing,
    and resolution output includes language, selected provider, and policy
    rationale.
- [ ] 3.2.2. Extend plugin manifest schema and broker loading to support
      capability
    declarations and capability-aware selection.
  - Acceptance criteria: manifest validation enforces capability fields, and
    provider selection respects language plus capability compatibility.
- [ ] 3.2.3. Add the `weaver act extricate --uri --position --to` command
      contract and
    wire capability discovery output for `extricate-symbol`.
  - Acceptance criteria: CLI request shape is stable across providers, and
    capability probe output reports extrication support by language.
- [ ] 3.2.4. Extend the Rope plugin with `extricate-symbol` support for Python.
  - Acceptance criteria: plugin returns unified diffs through existing patch
    application flow and preserves symbol semantics for supported Python shapes.
- [ ] 3.2.5. Implement Rust `extricate-symbol` orchestration with built-in
      capability
    ownership in `weaverd` and plugin-backed execution stages via
    `weaver-plugin-rust-analyzer`.
  - Acceptance criteria: flow includes overlay transaction planning, RA
    definition and references queries, code-action repair, and semantic
    verification before commit.
- [ ] 3.2.6. Extend plugin and daemon failure schemas with deterministic refusal
    diagnostics and hard rollback guarantees.
  - Acceptance criteria: refusal paths emit structured `PluginDiagnostic`
    payloads, include stable error codes, and leave the filesystem unchanged.
- [ ] 3.2.7. Add unit, behavioural, and end-to-end coverage for Python and Rust
    extrication, including ambiguous import repair and incomplete payload
    failures.
  - Acceptance criteria: tests assert meaning-preservation probes, module graph
    updates, and deterministic failure semantics.

### 3.3. Deliver first actuator plugin wave

- [ ] 3.3.1. Develop the first set of actuator plugins:

  - [x] A plugin for `rope` to provide advanced Python refactoring.

  - [x] A plugin for `rust-analyzer` to provide advanced Rust refactoring.

  - [ ] A plugin for `srgn` to provide high-performance, precision
        syntactic editing.

### 3.4. Deliver first specialist sensor plugin

- [ ] 3.4.1. Develop the first specialist sensor plugin:

  - [ ] A plugin for `jedi` to provide supplementary static analysis for
        Python.

### 3.5. Refine graceful degradation guidance

- [ ] 3.5.1. Refine the graceful degradation logic to suggest specific
      plugin-based
    solutions when core LSP features are missing.

### 3.6. Deliver static analysis provider integration

- [ ] 3.6.1. Implement the Static Analysis Provider for `weaver-graph` (e.g.,
    wrapping PyCG) as the first major graph plugin.

## 4. Advanced Agent Support & RAG

*Goal: Introduce features specifically designed to support advanced agent
planning and human-in-the-loop workflows.*

### 4.1. Deliver advanced agent workflow foundations

- [ ] 4.1.1. Implement the `onboard-project` command based on the "Meta-RAG"
      design,
    orchestrating other Weaver components to generate the `PROJECT.dna` summary
    file.

- [ ] 4.1.2. Implement a hybrid interactive mode (`--interactive`) that, in
      case of
    a "Double-Lock" verification failure, presents the proposed diff and the
    resulting errors to a human user for manual review, approval, or rejection.

- [ ] 4.1.3. Begin research and development for the Dynamic Analysis Ingestion
    provider for `weaver-graph`, allowing it to consume and merge profiling
    data from tools like `gprof` and `callgrind`.

## 5. CLI discoverability and help completion

*Goal: Close every discoverability and help-surface gap identified in the
[UI gap analysis](docs/ui-gap-analysis.md) so users can discover domains,
operations, plugins, and arguments without reading source code.*

*In scope: command help surfaces, argument discoverability, actionable errors,
plugin introspection, and capability introspection messaging.*

*Out of scope: new semantic editing capabilities unrelated to discoverability,
new plugin runtime engines, and unrelated daemon orchestration changes.*

Priority labels for Phase 5 align with the
[gap-analysis priority table](docs/ui-gap-analysis.md#summary-of-gaps-and-priority):

- `P0`: Immediate baseline discoverability gaps that block safe CLI adoption.
- `P1`: High-impact guidance and validation gaps required for dependable use.
- `P2`: Important discoverability improvements that reduce operator friction.
- `P3`: Lower-priority capability introspection enhancements.

Section priority mapping (non-scheduling metadata): 5.1=`P0`, 5.2=`P1`,
5.3=`P1-P2`, 5.4=`P2`, and 5.5=`P3`.

### 5.1. Deliver baseline guidance and top-level discoverability

- [x] 5.1.1. Show short help when `weaver` is invoked without arguments.
      See
      [Level 0](docs/ui-gap-analysis.md#level-0--bare-invocation-weaver)
      and
      [Level 10](docs/ui-gap-analysis.md#level-10--error-messages-and-exit-codes)
      (10d).
  - [x] Replace bare missing-domain output with short help and a clear next
        step.
  - [x] Acceptance criteria: `weaver` with no arguments exits non-zero, prints
        a `Usage:` line, lists the three valid domains (`observe`, `act`,
        `verify`), and includes exactly one pointer to `weaver --help`.
- [ ] 5.1.2. List all domains and operations in top-level help output.
      See
      [Gap 1a](docs/ui-gap-analysis.md#gap-1a--domains-not-enumerated)
      and
      [Gap 1b](docs/ui-gap-analysis.md#gap-1b--operations-not-enumerated).
  - [ ] Add an `after_help` catalogue covering `observe`, `act`, and `verify`
        operations.
  - [ ] Acceptance criteria: `weaver --help` lists all three domains and every
        CLI-supported operation for each domain, and completes without daemon
        startup or socket access.
- [ ] 5.1.3. Add top-level version output and long-form CLI description.
      See
      [Gap 1d](docs/ui-gap-analysis.md#gap-1d--no---version-flag)
      and
      [Gap 1e](docs/ui-gap-analysis.md#gap-1e--no-long-description-or-after-help-text).
  - [ ] Enable clap-provided `--version` and `-V` support.
  - [ ] Add a `long_about` quick-start block aligned with the
        [users guide](docs/users-guide.md).
  - [ ] Acceptance criteria: `weaver --version` and `weaver -V` both exit 0
        and emit the same version string, and `weaver --help` includes at
        least one runnable quick-start command example.
- [ ] 5.1.4. Provide contextual guidance when a domain is supplied without an
      operation. See
      [Level 2](docs/ui-gap-analysis.md#level-2--domain-without-operation-weaver-observe)
      and
      [Level 10](docs/ui-gap-analysis.md#level-10--error-messages-and-exit-codes)
      (10e).
  - [ ] Print available operations for the provided domain and a follow-up help
        command.
  - [ ] Acceptance criteria: `weaver <domain>` without an operation exits
        non-zero, lists all operations registered for that domain, and includes
        one concrete `weaver <domain> <operation> --help` hint.

### 5.2. Enrich validation and actionable error responses

- [ ] 5.2.1. Validate domains client-side before daemon startup.
      See
      [Level 3](docs/ui-gap-analysis.md#level-3--unknown-domain-weaver-bogus-something)
      and
      [Level 10](docs/ui-gap-analysis.md#level-10--error-messages-and-exit-codes)
      (10b).
  - [ ] Reject unknown domains with a valid-domain list.
  - [ ] Add edit-distance suggestions for close typos.
  - [ ] Acceptance criteria: invalid domains fail before daemon spawn, return
        all three valid domains in the error body, and include a single
        "did you mean" suggestion when edit distance is 2 or less.
- [ ] 5.2.2. Include valid operation alternatives for unknown operations.
      See
      [Level 4](docs/ui-gap-analysis.md#level-4--unknown-operation-weaver-observe-nonexistent)
      and
      [Level 10](docs/ui-gap-analysis.md#level-10--error-messages-and-exit-codes)
      (10c).
  - [ ] Extend daemon and CLI error payloads to include known operations for
        the domain.
  - [ ] Acceptance criteria: unknown-operation errors in both JSON and
        human-readable output include the full known-operation set for the
        domain, with a count equal to the router's `known_operations` length.
- [ ] 5.2.3. Standardize actionable guidance in startup and routing errors.
      See
      [Level 10](docs/ui-gap-analysis.md#level-10--error-messages-and-exit-codes)
      (10a-10e).
  - [ ] Apply a single error template: problem statement, valid alternatives,
        and explicit next command.
  - [ ] Add startup failure guidance for `WEAVERD_BIN` and installation checks.
  - [ ] Acceptance criteria: each Level 10 path (10a through 10e) renders the
        same three-part template (error, alternatives, next command), and
        preserves stable non-zero exit-code semantics.
- [ ] 5.2.4. Return complete argument requirements for `act refactor`.
      See
      [Gap 5b](docs/ui-gap-analysis.md#gap-5b--act-refactor-without-arguments).
  - [ ] List all required flags, valid provider names, and known refactoring
        operations.
  - [ ] Acceptance criteria: `weaver act refactor` without arguments reports
        all three required flags (`--provider`, `--refactoring`, `--file`) in
        one response, plus at least one valid provider and refactoring value.

### 5.3. Expose configuration and operation-level help surfaces

- [ ] 5.3.1. Surface configuration flags in clap help output.
      See
      [Gap 1c](docs/ui-gap-analysis.md#gap-1c--configuration-flags-invisible)
      and
      [Level 6](docs/ui-gap-analysis.md#level-6--configuration-flags-invisible-in-help).
  - [ ] Register `--config-path`, `--daemon-socket`, `--log-filter`,
        `--log-format`, and `--capability-overrides` as visible global flags.
  - [ ] Acceptance criteria: all five flags appear in both `weaver --help` and
        `weaver daemon start --help`, and existing precedence tests
        (file < env < CLI) continue to pass.
- [ ] 5.3.2. Extend `daemon start` help with config and environment guidance.
      See
      [Level 8](docs/ui-gap-analysis.md#level-8--daemon-subcommand-help).
  - [ ] Document `WEAVERD_BIN` and `WEAVER_FOREGROUND` in `long_about` or
        `after_help`.
  - [ ] Acceptance criteria: `weaver daemon start --help` documents both
        environment variables and includes at least one startup example using
        an override.
- [ ] 5.3.3. Re-enable and extend the `help` subcommand.
      See
      [Gap 1f](docs/ui-gap-analysis.md#gap-1f--help-subcommand-disabled)
      and
      [Level 12](docs/ui-gap-analysis.md#level-12--weaver-help-subcommand).
  - [ ] Remove `disable_help_subcommand = true`.
  - [ ] Support topic help for domains and operations (`weaver help <topic>`).
  - [ ] Acceptance criteria: `weaver help`, `weaver help observe`, and
        `weaver help act refactor` all exit 0 and return topic-specific help
        with no fallback to generic top-level output.
- [ ] 5.3.4. Deliver operation-level help for required arguments.
      Requires 5.3.3. See
      [Gap 5a](docs/ui-gap-analysis.md#gap-5a--observe-get-definition-without-arguments).
  - [ ] Implement nested clap subcommands, or an equivalent schema-backed help
        pipeline, so `weaver <domain> <operation> --help` is operation-specific.
  - [ ] Acceptance criteria: every exposed operation supports
        `weaver <domain> <operation> --help` and each help screen includes
        required flags, argument types, and at least one concrete invocation.
- [ ] 5.3.5. Document ortho-config v0.6.0 behaviour in 5.3 guidance. See
      [ortho-config v0.6.0 migration guide](docs/ortho-config-v0-6-0-migration-guide.md).
  - [ ] Document the new dependency-graph model used by configuration loading
        and precedence resolution.
  - [ ] Document fail-fast discovery behaviour when configuration files exist
        but are invalid.
  - [ ] Document YAML 1.2 parsing semantics via `SaphyrYaml`, including known
        compatibility warnings.
  - [ ] Update internal runbooks and user-facing documentation to reflect
        `ortho-config` v0.6.0 operational behaviour.
  - [ ] Validate documentation quality gates and docs tests after updates.
  - [ ] Acceptance criteria: migration guide, runbooks, and user docs are
        updated with explicit sections for dependency graph, fail-fast
        discovery, and YAML 1.2 semantics; and `make markdownlint`,
        `make fmt`, `make nixie`, and documentation tests pass.

### 5.4. Deliver plugin and manpage discoverability coverage

- [ ] 5.4.1. Add plugin introspection commands.
      See
      [Gap 1g](docs/ui-gap-analysis.md#gap-1g--plugin-listing-absent)
      and
      [Level 7](docs/ui-gap-analysis.md#level-7--plugin-discoverability).
  - [ ] Implement `weaver list-plugins` with `--kind` and `--language`
        filters.
  - [ ] Show plugin name, kind, language support, version, and timeout data.
  - [ ] Acceptance criteria: users can discover valid `act refactor`
        providers from CLI output alone, and table output includes the five
        fields `NAME`, `KIND`, `LANGUAGES`, `VERSION`, and `TIMEOUT`.
- [ ] 5.4.2. Wire plugin introspection into refactor guidance paths.
      Requires 5.4.1. See
      [Gap 5b](docs/ui-gap-analysis.md#gap-5b--act-refactor-without-arguments)
      and
      [Level 7](docs/ui-gap-analysis.md#level-7--plugin-discoverability).
  - [ ] Reference `weaver list-plugins` in refactor-related help and errors.
  - [ ] Acceptance criteria: every provider-related error points users to a
        discoverability command by including the exact string
        `weaver list-plugins`.
- [ ] 5.4.3. Regenerate and validate the manpage from the improved clap model.
      Requires 5.1.2, 5.3.1, and 5.3.3. See
      [Level 11](docs/ui-gap-analysis.md#level-11--manpage).
  - [ ] Verify that domain listings, operation listings, global config flags,
        and help-topic text render in troff output.
  - [ ] Acceptance criteria: generated manpage includes all updated help
        surfaces with no manual post-processing, including three domain
        listings and all five global config flags.

### 5.5. Complete capability probe discoverability

- [ ] 5.5.1. Clarify current `--capabilities` output semantics.
      See
      [Level 9](docs/ui-gap-analysis.md#level-9----capabilities-output).
  - [ ] Annotate output and help text that current data represents overrides
        unless runtime capability data is merged.
  - [ ] Acceptance criteria: users can distinguish override configuration from
        runtime-negotiated capability support via an explicit output marker and
        matching help-text note.
- [ ] 5.5.2. Merge runtime capability negotiation into the capabilities probe.
      Requires daemon capability query support. See
      [Level 9](docs/ui-gap-analysis.md#level-9----capabilities-output).
  - [ ] Query daemon-supported capabilities and combine them with configured
        overrides into one matrix.
  - [ ] Acceptance criteria: `weaver --capabilities` returns a complete matrix
        for each configured language and operation, and includes source labels
        for runtime capability versus override values.
