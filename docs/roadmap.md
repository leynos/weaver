# Roadmap

## Phase 0: Foundation & Tooling (Complete)

- [x] Set up the project workspace, CI/CD pipeline, and core dependencies.

## Phase 1: Core MVP & Safety Harness Foundation

*Goal: Establish the core client/daemon architecture, basic LSP integration,
and the foundational security and verification mechanisms. The MVP must be safe
for write operations from day one.*

### Step: Deliver CLI and daemon foundation

*Outcome: Ship a pair of crates (`weaver-cli`, `weaverd`) that honour the
design contract in `docs/weaver-design.md` and expose the lifecycle expected by
`docs/documentation-style-guide.md`.*

- [x] Define the shared configuration schema for `weaver-cli` and `weaverd`
      in `weaver-config`, using `ortho-config` to merge config files,
      environment overrides, and CLI flags for daemon sockets, logging, and the
      capability matrix defaults.
      - Acceptance criteria: Schema documented in crate docs, integration tests
        demonstrate precedence order (file < env < CLI), and default sockets
        align with the design doc.
- [x] Implement the `weaver-cli` executable as the thin JSON Lines (JSONL)
      client that
      initializes configuration via `ortho-config`, exposes the
      `--capabilities` probe, and streams requests to a running daemon over
      standard IO.
      - Acceptance criteria: CLI command surface mirrors the design table,
        capability probe outputs the negotiated matrix, and JSONL framing is
        validated with golden tests.
- [x] Implement the `weaverd` daemon bootstrap that consumes the shared
      configuration, starts the Semantic Fusion backends lazily, and supervises
      them with structured logging and error reporting.
      - Acceptance criteria: Bootstrap performs health reporting hooks,
        backends start only on demand, and failures propagate as structured
        events.
- [x] Implement robust daemonisation and process management for `weaverd`,
      including backgrounding with `daemonize-me`, PID/lock file handling,
      health checks, and graceful shutdown on signals.
      - Acceptance criteria: Background start creates PID and lock files,
        duplicate starts fail fast, and signal handling shuts down within the
        timeout budget.
- [x] Provide lifecycle commands in `weaver-cli` (for example, `daemon start`,
      `daemon stop`, `daemon status`) that manage the daemon process, verify
      socket availability, and surface actionable errors when start-up fails.
      - Acceptance criteria: Lifecycle commands call into shared helper logic,
        refuse to start when sockets are bound, and emit recovery guidance for
        the operator.

- [x] Implement the socket listener in `weaverd` to accept client connections
      on the configured Unix domain socket (or TCP socket on non-Unix
      platforms).
      - Acceptance criteria: Daemon binds to the socket path from configuration,
        accepts concurrent connections, and gracefully handles connection errors
        without crashing the daemon.

- [x] Implement the JSONL request dispatch loop in `weaverd` that reads
      `CommandRequest` messages from connected clients, routes them to the
      appropriate domain handler, and streams `CommandResponse` messages back.
      - Acceptance criteria: Request parsing rejects malformed JSONL with
        structured errors, domain routing covers `observe` and `act` commands,
        and responses include the terminal `exit` message with appropriate
        status codes.

- [x] Wire end-to-end domain command execution from CLI through daemon to
      backend, starting with `observe get-definition` as the first complete
      path.
      - Acceptance criteria: `weaver observe get-definition` with a running
        daemon returns LSP definition results, errors propagate with structured
        messages, and the CLI exits with the daemon-provided status code.

- [x] Build the `weaver-lsp-host` crate with support for initialization,
    capability detection, and core LSP features (definition, references,
    diagnostics) for Rust, Python, and TypeScript.

- [x] Implement process-based language server adapters for `weaver-lsp-host`.
    The `LspHost` currently requires external callers to register
    `LanguageServer` implementations via `register_language()`. This step adds
    concrete adapters that spawn real language server processes (e.g.,
    `rust-analyzer`, `pyrefly`, `tsgo`).
  - Acceptance criteria: `SemanticBackendProvider::start_backend()` registers
    adapters for configured languages, adapters spawn server processes and
    communicate via stdio, server shutdown is handled gracefully on daemon
    stop, and missing server binaries produce clear diagnostic errors.

- [x] Add human-readable output rendering for commands that return code
    locations or diagnostics, using `miette` or a compatible renderer to
    show context blocks.
  - Acceptance criteria: Definition, reference, diagnostics, and safety
    harness failure outputs include file headers, line-numbered source
    context, and caret spans in human-readable mode; JSONL output remains
    unchanged; missing source content falls back to path-and-range with a
    clear explanation.

- [x] Implement the initial version of the `weaver-sandbox` crate, using
    `birdcage` for its focused scope and production usage, prioritising robust
    Linux support via namespaces and seccomp-bpf.

- [x] Implement the full "Double-Lock" safety harness logic in `weaverd`.
    This is a critical, non-negotiable feature for the MVP. All `act` commands
    must pass through this verification layer before committing to the
    filesystem.
  - Acceptance criteria: Edit transactions pass through syntactic and semantic
    lock validation before commit, failures leave the filesystem untouched,
    and behaviour-driven development (BDD) scenarios cover success, syntactic
    failure, semantic failure, and backend unavailable error paths.

- [x] Implement atomic edits to ensure that multi-file changes either succeed
    or fail as a single transaction.
  - Acceptance criteria: Two-phase commit with prepare (temp files) and commit
    (atomic renames) phases, rollback restores original content on partial
    failure, and new file creation properly tracks file existence for
    rollback.

## Phase 2: Syntactic & Relational Intelligence

*Goal: Add the Tree-sitter and call graph layers to provide deeper structural
and relational understanding of code.*

- [x] Create the `weaver-syntax` crate and implement the structural search
    engine for `observe grep` and `act apply-rewrite`, drawing inspiration from
    ast-grep's pattern language.

- [x] Integrate the "Syntactic Lock" from `weaver-syntax` into the
    "Double-Lock" harness.

- [x] Extend the `LanguageServer` trait with document sync methods
    (`did_open`, `did_change`, `did_close`) to enable semantic validation
    of modified content at real file paths without writing to disk.

- [x] Create the `weaver-graph` crate and implement the LSP Provider for call
    graph generation, using the `textDocument/callHierarchy` request as the
    initial data source.

### Step: Deliver `act apply-patch` command

*Outcome: Provide a safety-locked patch application path that mirrors the
`apply_patch` semantics for agents and integrates with the Double-Lock harness.*

- [x] Add JSONL request/response types and a `weaver act apply-patch` command
    that reads the patch stream from standard input (STDIN) and forwards it to
    the daemon.
  - Acceptance criteria: CLI streams raw patch input, returns non-zero exit
    codes on failure, and surfaces structured errors.
- [x] Implement the patch parser and matcher in `weaverd` to support modify,
    create, and delete operations, including fuzzy matching, line-ending
    normalization, and path traversal checks.
  - Acceptance criteria: patch application is atomic per command, missing
    hunks are rejected, and parent directories are created for new files.
- [x] Integrate apply-patch with the safety harness using syntactic and
    semantic locks, ensuring no on-disk writes on lock failure.
  - Acceptance criteria: Tree-sitter validates modified/new files, LSP
    diagnostics are compared against the pre-edit baseline, and failures
    leave the filesystem untouched.
- [x] Add unit, BDD, and end-to-end tests covering create/modify/delete and
    failure paths (missing hunk, invalid header, traversal attempt).
  - Acceptance criteria: tests pass under `make test` and error messaging is
    asserted for each failure mode.

## Phase 3: Plugin Ecosystem & Specialist Tools

*Goal: Build the plugin architecture to enable orchestration of best-in-class,
language-specific tools.*

- [x] Design and implement the `weaver-plugins` crate, including the secure
    IPC protocol between the `weaverd` broker and sandboxed plugin processes.
    *(Phase 3.1.1 — see `docs/execplans/3-1-1-weaver-plugins-crate.md`)*

- [ ] Develop the first set of actuator plugins:

  - [x] A plugin for `rope` to provide advanced Python refactoring.

  - [x] A plugin for `rust-analyzer` to provide advanced Rust refactoring.

  - [ ] A plugin for `srgn` to provide high-performance, precision
        syntactic editing.

- [ ] Develop the first specialist sensor plugin:

  - [ ] A plugin for `jedi` to provide supplementary static analysis for
        Python.

- [ ] Refine the graceful degradation logic to suggest specific plugin-based
    solutions when core LSP features are missing.

- [ ] Implement the Static Analysis Provider for `weaver-graph` (e.g.,
    wrapping PyCG) as the first major graph plugin.

## Phase 4: Advanced Agent Support & RAG

*Goal: Introduce features specifically designed to support advanced agent
planning and human-in-the-loop workflows.*

- [ ] Implement the `onboard-project` command based on the "Meta-RAG" design,
    orchestrating other Weaver components to generate the `PROJECT.dna` summary
    file.

- [ ] Implement a hybrid interactive mode (`--interactive`) that, in case of
    a "Double-Lock" verification failure, presents the proposed diff and the
    resulting errors to a human user for manual review, approval, or rejection.

- [ ] Begin research and development for the Dynamic Analysis Ingestion
    provider for `weaver-graph`, allowing it to consume and merge profiling
    data from tools like `gprof` and `callgrind`.

## Phase 5: CLI discoverability and help completion

*Goal: Close every discoverability and help-surface gap identified in
`docs/ui-gap-analysis.md` so users can discover domains, operations, plugins,
and arguments without reading source code.*

*In scope: command help surfaces, argument discoverability, actionable errors,
plugin introspection, and capability introspection messaging.*

*Out of scope: new semantic editing capabilities unrelated to discoverability,
new plugin runtime engines, and unrelated daemon orchestration changes.*

### 5.1. Deliver baseline guidance and top-level discoverability (P0)

- [ ] 5.1.1. Show short help when `weaver` is invoked without arguments.
      See `docs/ui-gap-analysis.md` §Level 0 and §Level 10 (10d).
  - [ ] Replace bare missing-domain output with short help and a clear next
        step.
  - [ ] Acceptance criteria: `weaver` with no arguments prints usage, lists
        valid domains, and includes a pointer to `weaver --help`.
- [ ] 5.1.2. List all domains and operations in top-level help output.
      See `docs/ui-gap-analysis.md` §Level 1a and §Level 1b.
  - [ ] Add an `after_help` catalogue covering `observe`, `act`, and `verify`
        operations.
  - [ ] Acceptance criteria: `weaver --help` contains complete domain and
        operation listings without requiring daemon startup.
- [ ] 5.1.3. Add top-level version output and long-form CLI description.
      See `docs/ui-gap-analysis.md` §Level 1d and §Level 1e.
  - [ ] Enable clap-provided `--version` and `-V` support.
  - [ ] Add a `long_about` quick-start block aligned with `docs/users-guide.md`.
  - [ ] Acceptance criteria: `weaver --version` succeeds and help output
        includes purpose plus a quick-start example.
- [ ] 5.1.4. Provide contextual guidance when a domain is supplied without an
      operation. See `docs/ui-gap-analysis.md` §Level 2 and §Level 10 (10e).
  - [ ] Print available operations for the provided domain and a follow-up help
        command.
  - [ ] Acceptance criteria: `weaver observe` lists observe operations and
        points users to operation-level help.

### 5.2. Enrich validation and actionable error responses (P1)

- [ ] 5.2.1. Validate domains client-side before daemon startup.
      See `docs/ui-gap-analysis.md` §Level 3 and §Level 10 (10b).
  - [ ] Reject unknown domains with a valid-domain list.
  - [ ] Add edit-distance suggestions for close typos.
  - [ ] Acceptance criteria: invalid domains fail fast without daemon
        auto-start and include recovery guidance.
- [ ] 5.2.2. Include valid operation alternatives for unknown operations.
      See `docs/ui-gap-analysis.md` §Level 4 and §Level 10 (10c).
  - [ ] Extend daemon and CLI error payloads to include known operations for
        the domain.
  - [ ] Acceptance criteria: unknown operation errors include domain-scoped
        alternatives in both JSON and human-readable output.
- [ ] 5.2.3. Standardize actionable guidance in startup and routing errors.
      See `docs/ui-gap-analysis.md` §Level 10 (10a-10e).
  - [ ] Apply a single error template: problem statement, valid alternatives,
        and explicit next command.
  - [ ] Add startup failure guidance for `WEAVERD_BIN` and installation checks.
  - [ ] Acceptance criteria: all top-level CLI error paths include actionable
        next steps and stable exit-code semantics.
- [ ] 5.2.4. Return complete argument requirements for `act refactor`.
      See `docs/ui-gap-analysis.md` §Level 5b.
  - [ ] List all required flags, valid provider names, and known refactoring
        operations.
  - [ ] Acceptance criteria: `weaver act refactor` without required flags
        reports the full requirement set in one response.

### 5.3. Expose configuration and operation-level help surfaces (P1-P2)

- [ ] 5.3.1. Surface configuration flags in clap help output.
      See `docs/ui-gap-analysis.md` §Level 1c and §Level 6.
  - [ ] Register `--config-path`, `--daemon-socket`, `--log-filter`,
        `--log-format`, and `--capability-overrides` as visible global flags.
  - [ ] Acceptance criteria: the five flags appear in `weaver --help` and
        remain compatible with `ortho-config` precedence handling.
- [ ] 5.3.2. Extend `daemon start` help with config and environment guidance.
      See `docs/ui-gap-analysis.md` §Level 8.
  - [ ] Document `WEAVERD_BIN` and `WEAVER_FOREGROUND` in `long_about` or
        `after_help`.
  - [ ] Acceptance criteria: `weaver daemon start --help` includes relevant
        flag and environment override guidance.
- [ ] 5.3.3. Re-enable and extend the `help` subcommand.
      See `docs/ui-gap-analysis.md` §Level 1f and §Level 12.
  - [ ] Remove `disable_help_subcommand = true`.
  - [ ] Support topic help for domains and operations (`weaver help <topic>`).
  - [ ] Acceptance criteria: `weaver help`, `weaver help observe`, and
        `weaver help act refactor` all return contextual help.
- [ ] 5.3.4. Deliver operation-level help for required arguments.
      Requires 5.3.3. See `docs/ui-gap-analysis.md` §Level 5a.
  - [ ] Implement nested clap subcommands, or an equivalent schema-backed help
        pipeline, so `weaver <domain> <operation> --help` is operation-specific.
  - [ ] Acceptance criteria: operation help includes required flags, argument
        types, and one concrete invocation example per operation.

### 5.4. Deliver plugin and manpage discoverability coverage (P2)

- [ ] 5.4.1. Add plugin introspection commands.
      See `docs/ui-gap-analysis.md` §Level 1g and §Level 7.
  - [ ] Implement `weaver list-plugins` with `--kind` and `--language`
        filters.
  - [ ] Show plugin name, kind, language support, version, and timeout data.
  - [ ] Acceptance criteria: users can discover valid `act refactor`
        providers from CLI output alone.
- [ ] 5.4.2. Wire plugin introspection into refactor guidance paths.
      Requires 5.4.1. See `docs/ui-gap-analysis.md` §Level 5b and §Level 7.
  - [ ] Reference `weaver list-plugins` in refactor-related help and errors.
  - [ ] Acceptance criteria: every provider-related error points users to a
        discoverability command.
- [ ] 5.4.3. Regenerate and validate the manpage from the improved clap model.
      Requires 5.1.2, 5.3.1, and 5.3.3. See `docs/ui-gap-analysis.md` §Level
      11.
  - [ ] Verify that domain listings, operation listings, global config flags,
        and help-topic text render in troff output.
  - [ ] Acceptance criteria: generated manpage includes all updated help
        surfaces with no manual post-processing.

### 5.5. Complete capability probe discoverability (P3)

- [ ] 5.5.1. Clarify current `--capabilities` output semantics.
      See `docs/ui-gap-analysis.md` §Level 9.
  - [ ] Annotate output and help text that current data represents overrides
        unless runtime capability data is merged.
  - [ ] Acceptance criteria: users can distinguish override configuration from
        runtime-negotiated capability support.
- [ ] 5.5.2. Merge runtime capability negotiation into the capabilities probe.
      Requires daemon capability query support. See `docs/ui-gap-analysis.md`
      §Level 9.
  - [ ] Query daemon-supported capabilities and combine them with configured
        overrides into one matrix.
  - [ ] Acceptance criteria: `weaver --capabilities` returns a complete matrix
        for each configured language and operation.
