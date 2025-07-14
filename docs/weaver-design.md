<!-- markdownlint-disable MD013 MD033 MD001 -->
# weaver: A Composable, Semantics‑Aware Interface for AI Coding Agents

*Tag‑line: Bridging the semantic gap between text‑only agents and real codebases, one thread at a time.*

## Introduction: Why another tool?

Large‑language‑model (LLM) agents can already write and edit files, but they do so blind to the structure that keeps real software alive. Without symbol tables, call graphs, or project‑specific conventions, they hallucinate edits that compile locally, break somewhere else, and leave humans to pick up the pieces.

`weaver` is a command‑line interface (CLI) and long‑running daemon that lets an agent work at the level of semantics instead of bytes. It sits on top of the Serana toolkit and its multi‑language LSP bridge, turning LSP spaghetti into a crisp, UNIX-native interface.

This document synthesizes the original design sketch with subsequent reviews to clarify the implementation path, rename the tool, and weave together a complete specification.

### Guiding Philosophy: The Agent's OODA Loop

The design of `weaver` is framed by a coherent philosophy aimed at empowering an agent's cognitive cycle. This cycle is modelled on the Observe-Orient-Decide-Act (OODA) loop, a strategic framework for decision-making in dynamic environments. This structure provides a clear rationale for each command, aligning the toolset with the fundamental processes of an intelligent agent's workflow.

<table class="not-prose border-collapse table-auto w-full" style="min-width: 75px">
<colgroup><col style="min-width: 25px"><col style="min-width: 25px"><col style="min-width: 25px"></colgroup><tbody><tr><td class="border border-neutral-300 dark:border-neutral-600 p-1.5" colspan="1" rowspan="1"><p><strong>OODA Phase</strong></p></td><td class="border border-neutral-300 dark:border-neutral-600 p-1.5" colspan="1" rowspan="1"><p><strong>CLI Verbs</strong></p></td><td class="border border-neutral-300 dark:border-neutral-600 p-1.5" colspan="1" rowspan="1"><p><strong>Purpose</strong></p></td></tr><tr><td class="border border-neutral-300 dark:border-neutral-600 p-1.5" colspan="1" rowspan="1"><p><strong>Observe</strong></p></td><td class="border border-neutral-300 dark:border-neutral-600 p-1.5" colspan="1" rowspan="1"><p><code class="code-inline">project-status</code>, <code class="code-inline">list-diagnostics</code>, <code class="code-inline">onboard-project</code></p></td><td class="border border-neutral-300 dark:border-neutral-600 p-1.5" colspan="1" rowspan="1"><p>Sense the current workspace and its health.</p></td></tr><tr><td class="border border-neutral-300 dark:border-neutral-600 p-1.5" colspan="1" rowspan="1"><p><strong>Orient</strong></p></td><td class="border border-neutral-300 dark:border-neutral-600 p-1.5" colspan="1" rowspan="1"><p><code class="code-inline">find-symbol</code>, <code class="code-inline">get-definition</code>, <code class="code-inline">list-references</code>, <code class="code-inline">summarise-symbol</code>, <code class="code-inline">get-call-graph</code>, <code class="code-inline">get-type-hierarchy</code>, <code class="code-inline">list-memories</code></p></td><td class="border border-neutral-300 dark:border-neutral-600 p-1.5" colspan="1" rowspan="1"><p>Build deep context.</p></td></tr><tr><td class="border border-neutral-300 dark:border-neutral-600 p-1.5" colspan="1" rowspan="1"><p><strong>Decide</strong></p></td><td class="border border-neutral-300 dark:border-neutral-600 p-1.5" colspan="1" rowspan="1"><p><code class="code-inline">analyse-impact</code>, <code class="code-inline">get-code-actions</code>, <code class="code-inline">test</code>, <code class="code-inline">build</code>, <code class="code-inline">with-transient-edit</code></p></td><td class="border border-neutral-300 dark:border-neutral-600 p-1.5" colspan="1" rowspan="1"><p>Simulate &amp; verify changes before touching the disk.</p></td></tr><tr><td class="border border-neutral-300 dark:border-neutral-600 p-1.5" colspan="1" rowspan="1"><p><strong>Act</strong></p></td><td class="border border-neutral-300 dark:border-neutral-600 p-1.5" colspan="1" rowspan="1"><p><code class="code-inline">rename-symbol</code>, <code class="code-inline">apply-edits</code>, <code class="code-inline">format-code</code>, <code class="code-inline">set-active-project</code>, <code class="code-inline">reload-workspace</code></p></td><td class="border border-neutral-300 dark:border-neutral-600 p-1.5" colspan="1" rowspan="1"><p>Execute changes safely and atomically.</p></td></tr></tbody>
</table>

All output is JSON Lines (JSONL) for true UNIX composability, streamability, and fault containment.

## I. Core Architectural Principles

The foundation of `weaver` rests on architectural decisions designed to create a tool that is not only powerful but also performant, robust, and philosophically consistent with the UNIX environment in which the AI agent operates.

### 1.1 The Client-Daemon Split

To address the prohibitive startup latency of language servers, `weaver` employs a client-server model.

- **The** `weaverd` **Daemon:** This is a long-running background process, one per user, listening on a UNIX socket (e.g., `$XDG_RUNTIME_DIR/weaverd-$USER.sock`).

  - It starts and supervises language servers via `multilspy`.

  - It holds an in-memory semantic index, a file-content overlay cache for transient edits, and a msgspec-validated RPC dispatcher.

  - It enforces resource caps (e.g., `SERANA_MAX_RAM_MB`, `SERANA_MAX_CPUS`), returning structured errors when limits are reached.

  - It serialises concurrent requests using `asyncio` tasks, with support for cancelling long-running jobs.

- **The** `weaver` **Client:** This is a lightweight, stateless, and fast-executing binary (e.g., a static executable built with PyOxidiser for a &lt;50ms cold start).

  - It discovers the daemon's socket, automatically launching `weaverd` if it is not already running.

  - It performs a version-handshake RPC to ensure compatibility.

  - It streams JSONL responses from the daemon directly to `stdout`.

  - It exits with a code of `0` on success, or a non-zero code plus a final `Error` JSON object on controlled failure.

### 1.2 Atomicity & Safety

- The `apply-edits` command ensures atomicity by first writing all changes to temporary files within the target directory and then, only upon successful completion of all writes, executing a batch of atomic `rename` operations. If any write fails, no files in the workspace are mutated.

- Every mutating command (e.g., `rename-symbol`, `format-code`) supports a `--dry-run` flag. When used, the command will output the `CodeEdit` objects to `stdout` without modifying the filesystem, enabling bespoke audit and verification pipelines.

### 1.3 Workspace & Multi-Project Support

`weaver` is designed to manage multiple projects seamlessly.

- `weaver list-projects`: Shows all projects registered in a central configuration file (e.g., `~/.config/weaver/projects.yml`).

- `weaver set-active-project <name>`: Switches the daemon's context to a different registered project.

- Each project can have a local `.weaver/project.yml` file specifying its unique configuration: required language servers, environment variables, custom `test` and `build` commands, and paths to memory bundles.

- `weaver reload-workspace`: Instructs the daemon to force a re-index of the active project. This is crucial after significant changes to dependency files like `pyproject.toml` or `Cargo.toml`.

### 1.4 Memories & Onboarding

To align an agent's behaviour with project-specific conventions, `weaver` introduces a "memory" system.

- `weaver onboard-project`: Performs a first-time static analysis of a new project. It identifies conventions (e.g., coding style, testing patterns) and stores these findings as a "memory bundle" in `.weaver/memories/`.

- `weaver list-memories`: Streams the stored memory snippets as JSON objects. The agent can use this output to seed its prompt, ensuring its subsequent actions are consistent with the project's established patterns.

### 1.5 Transient Edits for Speculative Analysis

A cornerstone of the "Decide" phase is the ability to analyse changes without committing them to disk.

- `weaver with-transient-edit --file <path> --stdin <command...>`: This meta-command pipes new file content from `stdin`, instructs the daemon to apply it as a temporary in-memory overlay, runs the specified inner `weaver` command against this speculative state, and finally discards the overlay. This is perfect for "what-if" scenarios, such as checking for compilation errors before saving a file.

## II. The `weaver` Command Suite

The command suite is the heart of `weaver`, providing the agent with the tools necessary to execute its cognitive loop. Each command emits one or more JSON objects conforming to the schemas defined in Appendix A.

### 2.1 Observe

<table class="not-prose border-collapse table-auto w-full" style="min-width: 50px">
<colgroup><col style="min-width: 25px"><col style="min-width: 25px"></colgroup><tbody><tr><td class="border border-neutral-300 dark:border-neutral-600 p-1.5" colspan="1" rowspan="1"><p><strong>Command</strong></p></td><td class="border border-neutral-300 dark:border-neutral-600 p-1.5" colspan="1" rowspan="1"><p><strong>Synopsis</strong></p></td></tr><tr><td class="border border-neutral-300 dark:border-neutral-600 p-1.5" colspan="1" rowspan="1"><p><code class="code-inline">project-status</code></p></td><td class="border border-neutral-300 dark:border-neutral-600 p-1.5" colspan="1" rowspan="1"><p>Health of daemon &amp; language servers; RAM/CPU usage; protocol version.</p></td></tr><tr><td class="border border-neutral-300 dark:border-neutral-600 p-1.5" colspan="1" rowspan="1"><p><code class="code-inline">list-diagnostics</code></p></td><td class="border border-neutral-300 dark:border-neutral-600 p-1.5" colspan="1" rowspan="1"><p><code class="code-inline">[--severity S] [&lt;files…&gt;]</code> Stream <code class="code-inline">Diagnostic</code>s for whole workspace or subset.</p></td></tr><tr><td class="border border-neutral-300 dark:border-neutral-600 p-1.5" colspan="1" rowspan="1"><p><code class="code-inline">onboard-project</code></p></td><td class="border border-neutral-300 dark:border-neutral-600 p-1.5" colspan="1" rowspan="1"><p>First-run analysis, populates memories &amp; returns <code class="code-inline">OnboardingReport</code>.</p></td></tr></tbody>
</table>

### 2.2 Orient

<table class="not-prose border-collapse table-auto w-full" style="min-width: 50px">
<colgroup><col style="min-width: 25px"><col style="min-width: 25px"></colgroup><tbody><tr><td class="border border-neutral-300 dark:border-neutral-600 p-1.5" colspan="1" rowspan="1"><p><strong>Command</strong></p></td><td class="border border-neutral-300 dark:border-neutral-600 p-1.5" colspan="1" rowspan="1"><p><strong>Synopsis</strong></p></td></tr><tr><td class="border border-neutral-300 dark:border-neutral-600 p-1.5" colspan="1" rowspan="1"><p><code class="code-inline">find-symbol</code></p></td><td class="border border-neutral-300 dark:border-neutral-600 p-1.5" colspan="1" rowspan="1"><p><code class="code-inline">[--kind K] &lt;pattern&gt;</code> Search workspace symbols.</p></td></tr><tr><td class="border border-neutral-300 dark:border-neutral-600 p-1.5" colspan="1" rowspan="1"><p><code class="code-inline">get-definition</code></p></td><td class="border border-neutral-300 dark:border-neutral-600 p-1.5" colspan="1" rowspan="1"><p><code class="code-inline">&lt;file&gt; &lt;line&gt; &lt;char&gt;</code> Locate definitive declaration.</p></td></tr><tr><td class="border border-neutral-300 dark:border-neutral-600 p-1.5" colspan="1" rowspan="1"><p><code class="code-inline">list-references</code></p></td><td class="border border-neutral-300 dark:border-neutral-600 p-1.5" colspan="1" rowspan="1"><p><code class="code-inline">[--include-definition] &lt;file&gt; &lt;line&gt; &lt;char&gt;</code> All uses of symbol at cursor.</p></td></tr><tr><td class="border border-neutral-300 dark:border-neutral-600 p-1.5" colspan="1" rowspan="1"><p><code class="code-inline">summarise-symbol</code></p></td><td class="border border-neutral-300 dark:border-neutral-600 p-1.5" colspan="1" rowspan="1"><p><code class="code-inline">&lt;file&gt; &lt;line&gt; &lt;char&gt;</code> Aggregate hover, docstring, type info.</p></td></tr><tr><td class="border border-neutral-300 dark:border-neutral-600 p-1.5" colspan="1" rowspan="1"><p><code class="code-inline">get-call-graph</code></p></td><td class="border border-neutral-300 dark:border-neutral-600 p-1.5" colspan="1" rowspan="1"><p>`--direction &lt;in</p></td></tr><tr><td class="border border-neutral-300 dark:border-neutral-600 p-1.5" colspan="1" rowspan="1"><p><code class="code-inline">get-type-hierarchy</code></p></td><td class="border border-neutral-300 dark:border-neutral-600 p-1.5" colspan="1" rowspan="1"><p>`--direction &lt;super</p></td></tr><tr><td class="border border-neutral-300 dark:border-neutral-600 p-1.5" colspan="1" rowspan="1"><p><code class="code-inline">list-memories</code></p></td><td class="border border-neutral-300 dark:border-neutral-600 p-1.5" colspan="1" rowspan="1"><p>Stream previously stored memory snippets.</p></td></tr></tbody>
</table>

### 2.3 Decide

<table class="not-prose border-collapse table-auto w-full" style="min-width: 50px">
<colgroup><col style="min-width: 25px"><col style="min-width: 25px"></colgroup><tbody><tr><td class="border border-neutral-300 dark:border-neutral-600 p-1.5" colspan="1" rowspan="1"><p><strong>Command</strong></p></td><td class="border border-neutral-300 dark:border-neutral-600 p-1.5" colspan="1" rowspan="1"><p><strong>Synopsis</strong></p></td></tr><tr><td class="border border-neutral-300 dark:border-neutral-600 p-1.5" colspan="1" rowspan="1"><p><code class="code-inline">analyse-impact</code></p></td><td class="border border-neutral-300 dark:border-neutral-600 p-1.5" colspan="1" rowspan="1"><p><code class="code-inline">--edit &lt;json&gt;</code> Dry-run a single <code class="code-inline">CodeEdit</code>; returns <code class="code-inline">ImpactReport</code>.</p></td></tr><tr><td class="border border-neutral-300 dark:border-neutral-600 p-1.5" colspan="1" rowspan="1"><p><code class="code-inline">get-code-actions</code></p></td><td class="border border-neutral-300 dark:border-neutral-600 p-1.5" colspan="1" rowspan="1"><p><code class="code-inline">&lt;file&gt; &lt;line&gt; &lt;char&gt;</code> Available quick-fixes/refactors.</p></td></tr><tr><td class="border border-neutral-300 dark:border-neutral-600 p-1.5" colspan="1" rowspan="1"><p><code class="code-inline">test</code></p></td><td class="border border-neutral-300 dark:border-neutral-600 p-1.5" colspan="1" rowspan="1"><p><code class="code-inline">[--changed-files | --all]</code> Wrapper for project test command; same output contract.</p></td></tr><tr><td class="border border-neutral-300 dark:border-neutral-600 p-1.5" colspan="1" rowspan="1"><p><code class="code-inline">build</code></p></td><td class="border border-neutral-300 dark:border-neutral-600 p-1.5" colspan="1" rowspan="1"><p>Wrapper for project build command; same output contract.</p></td></tr><tr><td class="border border-neutral-300 dark:border-neutral-600 p-1.5" colspan="1" rowspan="1"><p><code class="code-inline">with-transient-edit</code></p></td><td class="border border-neutral-300 dark:border-neutral-600 p-1.5" colspan="1" rowspan="1"><p><code class="code-inline">--file &lt;f&gt; --stdin &lt;cmd …&gt;</code> Overlay speculative content, run another weaver command.</p></td></tr></tbody>
</table>

### 2.4 Act

<table class="not-prose border-collapse table-auto w-full" style="min-width: 50px">
<colgroup><col style="min-width: 25px"><col style="min-width: 25px"></colgroup><tbody><tr><td class="border border-neutral-300 dark:border-neutral-600 p-1.5" colspan="1" rowspan="1"><p><strong>Command</strong></p></td><td class="border border-neutral-300 dark:border-neutral-600 p-1.5" colspan="1" rowspan="1"><p><strong>Synopsis</strong></p></td></tr><tr><td class="border border-neutral-300 dark:border-neutral-600 p-1.5" colspan="1" rowspan="1"><p><code class="code-inline">rename-symbol</code></p></td><td class="border border-neutral-300 dark:border-neutral-600 p-1.5" colspan="1" rowspan="1"><p><code class="code-inline">&lt;file&gt; &lt;line&gt; &lt;char&gt; &lt;new&gt;</code> Generate safe rename plan.</p></td></tr><tr><td class="border border-neutral-300 dark:border-neutral-600 p-1.5" colspan="1" rowspan="1"><p><code class="code-inline">apply-edits</code></p></td><td class="border border-neutral-300 dark:border-neutral-600 p-1.5" colspan="1" rowspan="1"><p><code class="code-inline">[--atomic]</code> Read <code class="code-inline">CodeEdit</code> stream from <code class="code-inline">stdin</code>, write to disk.</p></td></tr><tr><td class="border border-neutral-300 dark:border-neutral-600 p-1.5" colspan="1" rowspan="1"><p><code class="code-inline">format-code</code></p></td><td class="border border-neutral-300 dark:border-neutral-600 p-1.5" colspan="1" rowspan="1"><p><code class="code-inline">[--stdin] [&lt;files…&gt;]</code> Emit formatting edits via language server.</p></td></tr><tr><td class="border border-neutral-300 dark:border-neutral-600 p-1.5" colspan="1" rowspan="1"><p><code class="code-inline">set-active-project</code></p></td><td class="border border-neutral-300 dark:border-neutral-600 p-1.5" colspan="1" rowspan="1"><p><code class="code-inline">&lt;name&gt;</code> Point daemon at another registered project.</p></td></tr><tr><td class="border border-neutral-300 dark:border-neutral-600 p-1.5" colspan="1" rowspan="1"><p><code class="code-inline">reload-workspace</code></p></td><td class="border border-neutral-300 dark:border-neutral-600 p-1.5" colspan="1" rowspan="1"><p>Force re-index after dependency file change.</p></td></tr></tbody>
</table>

## III. Implementation Roadmap

1. **Phase 0 – Scaffolding**:

   - Define all I/O schemas as msgspec models (see Appendix A).
   - Use [uv](monorepo-development-with-astral-uv.md) for environment and dependency management.

   - Implement the `asyncio` JSON-RPC router for `weaverd`.

   - Build the socket discovery and daemon auto-start logic in the `weaver` client.

2. **Phase 1 – Observe/Orient Verbs**:

   - Map commands directly onto existing `serena.tools` wrappers.

   - Validate functionality with a large, polyglot smoke-test repository.

3. **Phase 2 – Decide Verbs**:

   - Implement `analyse-impact` by replaying transient edits into the LSP and diffing the resulting diagnostics.

   - Implement `build` and `test` wrappers that parse tool output into the standard `Diagnostic` schema via regex patterns defined in `project.yml`.

4. **Phase 3 – Act Verbs**:

   - Implement atomised file writes for `apply-edits`.

   - Integrate with Git to ensure operations do not corrupt the work-tree.

   - Use the LSP `workspace/applyEdit` request to obtain the refactoring plan for `rename-symbol`.

5. **Phase 4 – Memories & Onboarding**:

   - Implement the persistence layer for memories as JSONL files under `.weaver/memories/`.

   - Develop the static analysis logic for `onboard-project`.

6. **Phase 5 – Polishing**:

   - Implement resource limit guardrails in `weaverd`.

   - Add cancellation support for long-running jobs.

   - Add a `weaver --json-schema` command to print the msgspec definitions, for embedding in agent prompts.

## Design Decisions

The project uses [uv](monorepo-development-with-astral-uv.md) for dependency
management and virtual environment creation. The CLI is built with `typer`
because it offers a clean API and automatic help text generation. Communication
between the client and daemon will use `anyio` for async socket operations.
`msgspec` is employed for fast, schema‑validated JSON serialization.

## IV. Advanced Workflows

The following examples demonstrate how the composable command set enables resilient, multi-step agentic workflows.

### 4.1 Safe Project-Wide Rename (Python)

- **Task:** The agent receives the high-level instruction: "In the Python project, rename the function `calculate_total_price` found in `app/logic.py` to `compute_final_price`."

```shell
# 1. Orient - Locate the canonical symbol
DEF=$(weaver get-definition app/logic.py 15 8)

# 2. Decide - Analyse the impact of the rename
# Construct a CodeEdit object for just the definition site
EDIT=$(echo "$DEF" | jq '{file: .location.file, range: .location.range, new_text:"compute_final_price"}')
IMPACT=$(weaver analyse-impact --edit "$EDIT")

# Abort if the initial change introduces any new errors
[[ $(echo "$IMPACT" | jq '.diagnostics | length') -eq 0 ]] || exit 1

# 3. Act (Plan) - Generate the full set of edits
weaver rename-symbol app/logic.py 15 8 compute_final_price > edits.jsonl

# 4. Act (Execute) - Apply the plan atomically
cat edits.jsonl | weaver apply-edits --atomic

# 5. Decide (Verify) - Run tests against the changed files
# The agent checks for any objects with severity "Error" in the output stream
weaver test --changed-files | jq -e 'select(.severity=="Error")' || echo "All good!"
```

### 4.2 First-Time Project Onboarding

- **Task:** An agent is encountering a new codebase for the first time.

```shell
# 1. Observe & Orient - Run onboarding and capture the output
weaver onboard-project | tee onboarding.jsonl

# 2. Ingest - The agent parses onboarding.jsonl
# It extracts key conventions, test commands, and architectural notes
# to use in the context for all subsequent interactions with this project.
```

## V. Future Directions

### 5.1 Remote Mode

For scenarios where the agent's CLI and the codebase are on different hosts (e.g., local VS Code to a remote build farm), `weaver` could expose its RPC interface via gRPC over an SSH-tunnelled TCP connection, allowing for secure, location-transparent operation.

### 5.2 Semantic Patch Review

A future command, `weaver diff --previous <rev>`, could use the semantic index to compare two Git revisions. Instead of a textual diff, it would output a stream of semantic changes: symbols added, functions whose signatures have changed, classes removed. This would be a powerful tool for generating automated, high-level pull request summaries.

### 5.3 AI-Assisted Human Prompts

A command like `weaver request-user-input <prompt>` could enable hybrid human-in-the-loop workflows. It would write a JSON-formatted question to `stdout` and pause execution, waiting for a corresponding JSON answer on `stdin`. This allows an agent to request clarification or approval from a human supervisor without breaking out of a scripted pipeline.

## Appendix A. Data Schemas (Excerpt)

The following msgspec types are the single source of truth for all JSONL I/O. Each has a `type` discriminator for effortless streaming introspection.

```python
from typing import Literal, Optional
from msgspec import Struct

class Position(Struct):
    line: int  # 0-indexed
    character: int  # UTF-16 code units, 0-indexed

class Range(Struct):
    start: Position
    end: Position

class Location(Struct):
    file: str  # Absolute or workspace-relative path
    range: Range

class Diagnostic(Struct):
    type: Literal['diagnostic'] = 'diagnostic'
    location: Location
    severity: Literal['Error', 'Warning', 'Info', 'Hint']
    code: Optional[str]
    message: str

# ... and so on for Symbol, Reference, CodeEdit, ImpactReport,
# TestResult, OnboardingReport, Error, etc.

```
