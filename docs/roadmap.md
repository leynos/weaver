<!-- markdownlint-disable MD013 MD033 MD001 -->
# Weaver: Development Roadmap

This document provides a detailed, task-oriented development roadmap for
building the `weaver` CLI and its associated `weaverd` daemon, based on the
established design specification. It is intended for the implementation team to
track progress and coordinate efforts.

### **Phase 0: Core Scaffolding & Project Setup**

*This phase establishes the foundational architecture, project structure, and
communication protocols. Completing this phase means the* `weaver` *client can
successfully connect to the* `weaverd` *daemon, but no actual LSP functionality
will be implemented yet.*

- [ ] **Finalise Project Structure & Dependencies:**

  - [x] Decide on a monorepo and use
    [uv](monorepo-development-with-astral-uv.md) for package and environment
    management.
  - [x] Initialise the project with
    [uv](monorepo-development-with-astral-uv.md) and add core dependencies:
    `msgspec`, `typer` (for the CLI), `anyio` (for async socket communication),
    and `multilspy`.

- [x] **Define the API Contract with msgspec:**

  - [x] Create a shared internal package (`weaver-schemas` or similar)
    containing msgspec `Struct` definitions for every JSON object specified in
    Appendix A of the design document (`Location`, `Diagnostic`, `CodeEdit`,
    `ImpactReport`, etc.).

  - [x] Ensure all models include the `type` discriminator field
    (`type: Literal['diagnostic'] = 'diagnostic'`) to facilitate easy parsing
    of the JSONL stream on the client side. These models are the single source
    of truth for the API.

- [x] **Implement the** `weaverd` **Daemon Skeleton:**

  - [x] Create the main `asyncio` entry point for the daemon.

  - [x] Implement an RPC router that listens on a UNIX domain socket (e.g.,
    `$XDG_RUNTIME_DIR/weaverd-$USER.sock`). Use a library like `jsonrpc-py` or
    build a simple dispatcher that maps method names to handler functions.

  - [x] The dispatcher should accept JSON requests, validate them against the
    msgspec models, call the corresponding (stubbed) handler, and serialize the
    msgspec response model back to JSONL.

  - [x] Implement a basic `ping` or `project-status` RPC endpoint that returns
    a hardcoded success response.

- [ ] **Implement the** `weaver` **Client Skeleton:**

  - [ ] Set up the CLI using `typer`. Create a stub for each command defined in
    the design document.

  - [ ] Implement the socket discovery logic. The client must locate the
    `weaverd` socket at its well-known path.

  - [ ] Implement the daemon auto-start logic. If the client cannot connect to
    the socket, it should attempt to spawn the `weaverd` process in the
    background (`subprocess.Popen` with appropriate flags to detach it).

  - [ ] Implement the core RPC client function that connects to the socket,
    sends a msgspec-serialised request, and streams the JSONL response directly
    to `stdout`.

  - [ ] Implement the `weaver project-status` command to call the `ping`
    endpoint on the daemon. A successful run of this command validates the
    entire communication pipeline.

### **Phase 1: Read-Only Verbs (Observe & Orient)**

*This phase brings the core read-only LSP functionality to life. The goal is to
enable the agent to inspect and understand a codebase without modifying it.
This phase focuses on wrapping existing* `multilspy` *capabilities.*

- [ ] **Integrate** `multilspy` **into** `weaverd`**:**

  - [ ] Implement the logic within `weaverd` to initialise the
    `multilspy.LanguageServerManager` on startup.

  - [ ] Implement the `onboard-project` command logic to register a new
    workspace root with the manager and trigger the initial indexing.

  - [ ] The daemon must manage the lifecycle of the language servers, passing
    the correct initialisation options.

- [ ] **Implement Observe Commands:**

  - [ ] `project-status`: Enhance the stub to query the `LanguageServerManager`
    for the actual status of each language server (PID, memory, readiness
    state).

  - [ ] `list-diagnostics`: Implement the handler to call `multilspy`'s
    `get_diagnostics` method and stream the results, converting them to the
    `weaver` `Diagnostic` msgspec model.

- [ ] **Implement Orient Commands:**

  - [ ] `get-definition`: Implement the handler to call
    `textDocument/definition`.

  - [ ] `list-references`: Implement the handler to call
    `textDocument/references`.

  - [ ] `find-symbol`: Implement the handler to call `workspace/symbol`.

  - [ ] `summarise-symbol`: Implement the handler to call `textDocument/hover`
    and synthesise the response into the `SymbolSummary` model.

  - [ ] `get-call-graph` **/** `get-type-hierarchy`: Implement the handlers for
    `callHierarchy/incomingCalls`, `callHierarchy/outgoingCalls`,
    `typeHierarchy/supertypes`, and `typeHierarchy/subtypes`. This may require
    recursive calls to build out the graph to a specified depth.

- [ ] **Testing Strategy:**

  - [ ] Create a dedicated, multi-language test repository (containing Python,
    Rust, and TypeScript files with known symbols, errors, and references).

  - [ ] Write an integration test suite that runs `weaver` commands against
    this repository and validates the resulting JSONL output against expected
    snapshots.

### **Phase 2: Simulation & Analysis Verbs (Decide)**

*This phase implements the "semantic firewall" — the ability to simulate
changes and analyse their impact. This is the most complex part of the
read-only functionality and is critical for agent safety.*

- [ ] **Implement the Transient Edit Cache in** `weaverd`**:**

  - [ ] The daemon needs an in-memory dictionary that maps file paths to their
    transient content (`Dict[str, str]`).

  - [ ] Before making an LSP request, the daemon must check if any files
    involved in the request have a transient version. If so, it must send a
    `textDocument/didChange` notification to the LSP server with the transient
    content.

  - [ ] After the request is complete, it must send another
    `textDocument/didChange` notification to revert the file to its on-disk
    state, ensuring the overlay is temporary.

- [ ] **Implement** `with-transient-edit`**:**

  - [ ] This meta-command in the client will orchestrate the process. It will
    first send a custom RPC call to the daemon (`_set_transient_overlay`) with
    the content from `stdin`.

  - [ ] It will then execute the inner command as a separate RPC call.

  - [ ] Finally, it will use a `finally` block to guarantee it sends a
    `_clear_transient_overlay` RPC call to the daemon.

- [ ] **Implement** `analyse-impact`**:**

  - [ ] This command will use the `with-transient-edit` flow. It will apply the
    proposed edit as an overlay and then run the `list-diagnostics` logic
    against the entire workspace, diffing the result against the baseline
    diagnostics to report only *new* errors.

- [ ] **Implement Build/Test Wrappers:**

  - [ ] Implement the `test` and `build` commands. These will execute the
    project-specific shell commands defined in `.weaver/project.yml`.

  - [ ] They will capture the `stdout` and `stderr` of the subprocess and parse
    them using regex patterns (also defined in `project.yml`) to convert the
    human-readable output into `weaver` `Diagnostic` objects.

### **Phase 3: Mutable Verbs (Act)**

*This phase grants the agent the ability to safely modify the filesystem. The
key principles are atomicity and decoupling planning from execution.*

- [ ] **Implement** `apply-edits`**:**

  - [ ] The client-side command will read a stream of `CodeEdit` objects from
    `stdin`.

  - [ ] For each unique file in the edit set, it will write the modified
    content to a temporary file (e.g., `main.py.weaver-tmp`).

  - [ ] **Atomicity:** Only after *all* temporary files have been successfully
    written will it perform atomic `os.rename` operations to replace the
    original files. If any write fails, it will clean up all temporary files
    and exit with an error, leaving the workspace untouched.

- [ ] **Implement Plan-Generating Commands:**

  - [ ] `rename-symbol`: Implement the RPC handler to call the LSP's
    `textDocument/rename` request. This request returns a `WorkspaceEdit`
    object, which must be converted into a stream of `weaver` `CodeEdit`
    objects.

  - [ ] `format-code`: Implement the RPC handler to call
    `textDocument/formatting` and convert the result into a `CodeEdit` stream.

- [ ] **Implement Workspace Management:**

  - [ ] Implement the central project configuration
    (`~/.config/weaver/projects.yml`).

  - [ ] `list-projects` **/** `set-active-project`: Implement the client
    commands and daemon handlers to read the config and switch the
    `LanguageServerManager`'s active workspace.

  - [ ] `reload-workspace`: Implement the handler to trigger a `shutdown` and
    `initialize` sequence for the language servers in the active workspace.

### **Phase 4: Intelligence & Persistence (Memories)**

*This phase adds long-term memory and project-specific intelligence, allowing
the agent to adapt to local conventions.*

- [ ] **Implement** `onboard-project` **Analysis:**

  - [ ] Develop the static analysis logic for the onboarding process. This will
    involve running a series of `weaver` commands (e.g., `find-symbol` for test
    files, `list-diagnostics` to find linting rules) and heuristics to identify
    patterns.

  - [ ] For example, to find the testing framework, it could look for imports
    of `pytest` or `unittest`.

- [ ] **Implement Memory Persistence:**

  - [ ] Create the `.weaver/memories/` directory structure.

  - [ ] The `onboard-project` command will write its findings as a JSONL file
    in this directory.

  - [ ] `list-memories`: This command will simply read and stream the contents
    of the memory files to `stdout`.

### **Phase 5: Polishing & Productionisation**

*This final phase focuses on robustness, usability, and distribution.*

- [ ] **Implement Resource Management in** `weaverd`**:**

  - [ ] Add logic to monitor the memory and CPU usage of the daemon and its
    child LSP processes.

  - [ ] If the limits defined by `WEAVER_MAX_RAM_MB` or `WEAVER_MAX_CPUS` are
    exceeded, the daemon should gracefully reject new requests with a
    structured `Error` object.

- [ ] **Implement Job Cancellation:**

  - [ ] The `asyncio` tasks in the daemon that handle long-running requests
    (e.g., `list-references` in a huge project) should be cancellable.

  - [ ] The client should handle `SIGINT` (Ctrl+C) and send a custom
    `_cancel_request` RPC call to the daemon.

- [ ] **Implement Schema Generation:**

  - [ ] Add the `weaver --json-schema` command. This will iterate through all
    msgspec models and call `msgspec.json.schema(Model)` to generate a complete
    JSON Schema for the entire API, emitting it via `json.dumps`.

- [ ] **Package for Distribution:**

  - [ ] Set up a build process using **PyOxidiser** to package the `weaver`
    client into a single, static executable for easy distribution and minimal
    startup overhead.

  - [ ] Create installation scripts or packages for `weaverd` to be run as a
    systemd/launchd service.

- [ ] **Documentation:**

  - [ ] Write comprehensive documentation for each command, including examples
    of its JSONL output.

  - [ ] Document the configuration file formats (`projects.yml` and
    `.weaver/project.yml`).
