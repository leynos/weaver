# Weaver user's guide

This guide summarizes the behaviour exposed to operators by the initial CLI and
daemon foundation. Configuration is currently the primary focus because both
binaries share the same loading pipeline and rely on the `weaver-config` crate
to merge settings from files, environment variables, and command-line arguments.

## Configuration layering

Configuration is layered using `ortho-config` with the following precedence
order:

1. built-in defaults,
2. configuration files discovered via `--config-path` and the XDG search path,
3. environment variables, and
4. CLI flags.

Each successive layer overrides earlier sources. This guarantees that a
parameter passed through the CLI is honoured even when a configuration file or
environment variable also supplies the same field.

### CLI flags

The CLI exposes the following configuration flags today:

- `--config-path <PATH>` — reads an explicit configuration file.
- `--daemon-socket <ENDPOINT>` — overrides the daemon transport. Accepts
  values such as `unix:///run/user/1000/weaver.sock` or `tcp://127.0.0.1:9779`.
- `--log-filter <FILTER>` — sets the tracing filter (defaults to `info`).
- `--log-format <FORMAT>` — selects the log output format (`json` or `compact`
  only).
- `--capability-overrides <DIRECTIVE>` — appends a directive of the form
  `language:capability=directive`. Directives may be repeated to accumulate
  overrides. Duplicate entries are resolved by keeping the last directive for
  each language and capability pair, and lookups ignore case and surrounding
  whitespace.

### Environment variables

The same options are available through environment variables. They follow the
`WEAVER_*` naming convention:

- `WEAVER_CONFIG_PATH`
- `WEAVER_DAEMON_SOCKET`
- `WEAVER_LOG_FILTER`
- `WEAVER_LOG_FORMAT`

Environment variables override files, but remain lower priority than CLI flags.

### Configuration file example

Configuration files are written in TOML. The following snippet demonstrates how
to pin the daemon socket, switch to a compact log format, and force the call
hierarchy capability for Python:

```toml
daemon_socket = { transport = "tcp", host = "127.0.0.1", port = 9779 }
log_filter = "info"
log_format = "compact"

[[capability_overrides]]
language = "python"
capability = "observe.call-hierarchy"
directive = "force"
```

### Validation and error reporting

`weaver` now uses `ortho-config` v0.6.0, which treats invalid configuration
files as fatal. When `--config-path` points at a broken file—or when discovery
finds a malformed `weaver.toml`/`.weaver.toml`—, both the CLI and daemon abort
with a `LoadConfiguration` error that lists every offending path. Remove or fix
the reported files before retrying. If no configuration files exist at all the
loader still falls back to the built-in defaults described below.

Operators will see aggregated errors enumerated in the order discovery
encounters them. For example:

```text
failed to load configuration: multiple configuration errors:
1: Configuration file error in '/etc/weaver/weaver.toml': expected `}`
2: Configuration file error in '/home/alex/.weaver.toml': invalid type:
string "yes", expected a boolean
```

## Defaults

- **Daemon socket:** On Unix-like targets, the daemon listens on
  `$XDG_RUNTIME_DIR/weaver/weaverd.sock`. When the runtime directory is
  unavailable, the default falls back to a per-user namespace under the system
  temporary directory (for example `/tmp/weaver/uid-1000/weaverd.sock`). Other
  platforms default to `tcp://127.0.0.1:9779`.
- **Logging:** The default filter is `info` and the default format is `json`.
- **Capability overrides:** No overrides are applied unless provided via one of
  the mechanisms above. Each directive is treated independently, so multiple
  overrides may be supplied to tailor the capability matrix for different
  languages.

When `weaverd` starts, it ensures the parent directory for the configured Unix
socket exists, returning a descriptive error if the directory cannot be
created. This prevents silent failures later when the daemon attempts to bind
the socket.

## Daemon lifecycle

`weaverd` backgrounds itself using `daemonize-me` and manages runtime artefacts
under the same directory as the Unix socket (for example
`$XDG_RUNTIME_DIR/weaver`). Launching the daemon creates a lock file
(`weaverd.lock`), a PID file (`weaverd.pid`), and a health snapshot
(`weaverd.health`). PID and health files are written atomically, so observers
never see a partially written payload. Attempts to start a second copy while
one is running fail fast with an "already running" error that reports the
existing PID. When the original launch is still initialising and has not yet
published a PID, the second invocation now reports "launch already in progress"
instead of removing the lock. If the daemon exited uncleanly, the new instance
removes the stale files before continuing.

The daemon now binds a socket listener as part of startup. It binds to the
configured `--daemon-socket` endpoint and accepts multiple client connections
concurrently. On Unix targets, stale socket files are removed only after
confirming no listener responds, while actively used sockets cause the daemon
to fail fast with a clear error. The listener removes the Unix socket file on
shutdown to avoid lingering bind failures. While the request loop is still a
placeholder, the daemon replies with a minimal JSONL exit message and closes
the connection so clients do not block waiting for a response.

The health snapshot is a single-line JSON document describing the current
state, enabling operators and automation to poll readiness without speaking the
daemon protocol. Example:

```json
{"status":"ready","pid":12345,"timestamp":1713356400}
```

The `status` transitions through `starting`, `ready`, and `stopping` before the
files are removed on shutdown. Sending `SIGTERM`, `SIGINT`, `SIGQUIT`, or
`SIGHUP` prompts the daemon to log the request and complete its shutdown
sequence within a ten-second budget. For interactive debugging or CI jobs, set
`WEAVER_FOREGROUND=1` to keep the daemon attached to the terminal while
preserving the same lock, PID, and health semantics.

## Sandbox defaults

External tools launched by the daemon now run inside the `weaver-sandbox`
wrapper around `birdcage` 0.8.1. Linux namespaces and `seccomp-bpf` filters are
applied automatically; networking is disabled by default; and only a small set
of standard library directories are readable to keep dynamically linked
executables functioning. Commands must be provided as absolute paths and added
to the sandbox allowlist before launch; requests made from multithreaded
contexts return a `MultiThreaded` error rather than panicking the process. The
sandbox strips the environment unless specific variables are explicitly
whitelisted, so callers should pass configuration via the broker rather than
relying on inherited host state.

### Lifecycle commands

`weaver` now exposes explicit lifecycle commands so operators do not need to
manage the daemon manually. All three commands share the same helper logic and
therefore honour the configuration flags supplied to the CLI, including
`--config-path` and `--daemon-socket`.

- `weaver daemon start` verifies that the configured socket is free, spawns the
  `weaverd` binary (the path can be overridden via `WEAVERD_BIN`), and waits
  for the health snapshot to report `ready`. The command refuses to start when
  the socket already accepts connections and prints the runtime directory that
  now holds the lock, PID, and health files.
- `weaver daemon stop` reads the PID file, sends `SIGTERM`, and waits for the
  runtime artefacts and socket to disappear. If the socket is reachable but the
  PID file is missing, the command surfaces an error rather than blindly
  killing a process. Successful stops report the PID that was terminated and
  confirm the runtime directory was cleaned up.
- `weaver daemon status` inspects the JSON health snapshot when present, falling
  back to the PID file and socket reachability. When no runtime artefacts exist
  the command prints a short reminder that `daemon start` can be used to launch
  a new instance.

Lifecycle commands never contact the daemon's JSONL transport. They operate on
shared runtime files from `weaver-config`, so the CLI and daemon use the same
directory layout even when the daemon socket is overridden.

### Automatic daemon startup

When a domain command is issued and the daemon is not running, the CLI
automatically attempts to start the daemon rather than failing immediately. The
message `Waiting for daemon start...` appears on stderr while the CLI waits for
the daemon to become ready. The timeout for automatic startup is 30 seconds; if
the daemon fails to start within this period, the CLI reports the failure and
exits.

This behaviour allows operators to run commands without explicitly starting the
daemon first:

```sh
weaver observe get-definition --uri file:///src/main.rs --position 10:5
```

If the daemon is not running, it will be started automatically before the
command executes. The automatic startup uses the same configuration flags
(`--config-path`, `--daemon-socket`, etc.) passed to the command.

Errors that prevent connection but are not related to the daemon being offline
(such as permission denied or network timeouts) bypass automatic startup and
are reported immediately.

## Command reference

`weaver` exposes three command families: the `--capabilities` probe, daemon
lifecycle commands, and domain operations (`observe`, `act`, `verify`). Domain
commands are sent to the daemon as JSONL; any arguments after the operation are
forwarded verbatim without CLI validation.

### Output formats

Daemon responses are JSON objects with `kind` set to `stream` or `exit`. Stream
messages include a `stream` field (`stdout` or `stderr`) plus a `data` payload;
exit messages contain a numeric `status`. The CLI writes each `data` payload to
the matching host stream and terminates using the exit status provided by the
final exit message. The `data` payload can be plain text (human-readable) or a
JSON document (machine-readable). Example JSONL envelope:

```json
{"kind":"stream","stream":"stdout","data":"definition: file:///path/main.rs:42:17\n"}
{"kind":"exit","status":0}
```

Daemon connections time out after five seconds. The CLI aborts after ten
consecutive blank lines and treats missing exit messages as failures.

### Capability probe

Syntax:

```sh
weaver --capabilities
```

Output is always JSON (pretty-printed for humans). Example:

```json
{
  "languages": {
    "python": {
      "overrides": {
        "observe.call-hierarchy": "force"
      }
    }
  }
}
```

### Daemon lifecycle commands

Syntax:

```sh
weaver daemon start
weaver daemon stop
weaver daemon status
```

Example human-readable output (`daemon start`):

```text
daemon ready (pid 12345) on unix:///tmp/weaver/uid-1000/weaverd.sock
runtime artefacts stored under /tmp/weaver/uid-1000
```

Example JSON output written by the daemon health snapshot file
(`weaverd.health`):

```json
{"status":"ready","pid":12345,"timestamp":1713356400}
```

### Domain commands (`observe`, `act`, `verify`)

Syntax:

```sh
weaver <domain> <operation> [ARG ...]
```

Current capability keys used for LSP-backed operations:

- `observe.get-definition`
- `observe.find-references`
- `observe.call-hierarchy`
- `verify.diagnostics`

Syntactic operations provided by `weaver-syntax` use the same domain/operation
shape (`observe grep` and `act apply-rewrite`) once they are wired into the
daemon request loop. The examples below are illustrative; the daemon defines
the exact payload schema.

#### observe get-definition

Syntax:

```sh
weaver observe get-definition --uri <URI> --position <LINE:COL>
```

Human output:

```text
definition: <URI>:<LINE>:<COL>
```

JSON payload:

```json
{"uri":"<URI>","line":42,"column":17}
```

#### observe find-references

Syntax:

```sh
weaver observe find-references --uri <URI> --position <LINE:COL>
```

Human output:

```text
reference: <URI>:<LINE>:<COL>
```

JSON payload:

```json
{"references":[{"uri":"<URI>","line":12,"column":3}]}
```

#### observe call-hierarchy

Syntax:

```sh
weaver observe call-hierarchy --uri <URI> --position <LINE:COL>
```

Human output:

```text
call hierarchy: <SYMBOL> (direction outgoing, depth 2)
```

JSON payload:

Call hierarchy responses return a call graph. Each node includes its stable
identifier, symbol name, kind, location, and optional container. Each edge
captures the caller, callee, provenance, and optional call-site position.

```json
{
  "nodes": [
    {
      "id": "/src/lib.rs:10:0:main",
      "name": "main",
      "kind": "function",
      "uri": "file:///src/lib.rs",
      "line": 10,
      "column": 0,
      "container": null
    }
  ],
  "edges": [
    {
      "caller": "/src/lib.rs:10:0:main",
      "callee": "/src/lib.rs:42:0:helper",
      "source": "lsp",
      "call_site": { "line": 12, "column": 4 }
    }
  ]
}
```

#### observe grep

Syntax:

```sh
weaver observe grep --pattern <PATTERN> --path <PATH>
```

Optional flags:

```text
--language <LANG>
```

Human output:

```text
match: <PATH>:<LINE>:<COL> "$NAME"
```

JSON payload:

```json
{"matches":[{"start":[1,1],"captures":{"NAME":"foo"}}]}
```

#### verify diagnostics

Syntax:

```sh
weaver verify diagnostics --uri <URI>
```

Human output:

```text
diagnostic: <URI>:<LINE>:<COL> <MESSAGE>
```

JSON payload:

```json
{"diagnostics":[{"line":12,"column":5,"message":"..."}]}
```

#### act apply-rewrite

Syntax:

```sh
weaver act apply-rewrite --pattern <PATTERN> --replacement <REPL> --path <PATH>
```

Human output:

```text
rewrite: <PATH> (replacements 2)
```

JSON payload:

```json
{"path":"<PATH>","replacements":2,"changed":true}
```

## Language server capability detection

The new `weaver-lsp-host` crate initialises the LSP servers for Rust, Python,
and TypeScript and records which core requests each server advertises:
`textDocument/definition`, `textDocument/references`, diagnostics, and call
hierarchy (`textDocument/prepareCallHierarchy` plus incoming/outgoing calls).
These advertised capabilities are merged with any overrides provided via
`capability_overrides` in `weaver-config`. `force` directives allow a request
even when the server claims not to support it, while `deny` directives block
the request regardless of the server report. When a request is rejected, the
error explains whether the feature was disabled by configuration or simply
absent from the server so operators and agents can adjust their plans without
guesswork.

## Double-Lock safety harness

All `act` commands pass through a "Double-Lock" safety harness before any
changes are committed to the filesystem. This verification layer ensures that
agent-generated modifications do not corrupt the codebase by introducing syntax
errors or type mismatches.

### Two-phase verification

The harness validates proposed edits in two sequential phases:

1. **Syntactic Lock**: Each modified file is parsed to ensure it produces a
   valid syntax tree. Structural errors such as unbalanced braces, missing
   semicolons, or malformed declarations are caught at this stage. Files that
   fail parsing are rejected immediately, and the filesystem remains untouched.

2. **Semantic Lock**: If the syntactic lock passes, the modified content is
   submitted to the configured language server. The daemon requests fresh
   diagnostics and compares them against the pre-edit baseline. Any new errors
   or high-severity warnings cause the semantic lock to fail. Only when both
   locks pass are the changes atomically written to disk.

### In-memory application

Edits are first applied to in-memory copies of the affected files. The original
content is preserved until both verification phases succeed. This allows the
harness to reject problematic changes without leaving partially written files
on disk.

### Document sync notifications

The semantic lock now opens in-memory documents on the language server using
`textDocument/didOpen`, applies updates with `textDocument/didChange`, and
closes them with `textDocument/didClose` once diagnostics are collected. This
lets the server validate the modified content at the real file URI without
writing temporary files, so cross-file imports resolve as usual.

### Atomic commits

When both locks pass, the harness writes each modified file atomically by
creating a temporary file and renaming it into place. This guarantees that a
crash or power loss during the commit phase does not leave files in a corrupted
intermediate state.

### Error reporting

When verification fails, the harness returns a structured error describing:

- **Lock phase**: Whether the failure occurred during syntactic or semantic
  validation.
- **Affected files**: Paths to the files that triggered the failure.
- **Locations**: Optional line and column numbers pinpointing each issue.
- **Messages**: Human-readable descriptions of what went wrong.

Agents can use this information to diagnose problems and regenerate corrected
edits. The structured format also enables tooling to present failures in IDE
integrations or CI pipelines.

### Tree-sitter syntactic lock

The syntactic lock is powered by the `weaver-syntax` crate, which integrates
Tree-sitter parsers for Rust, Python, and TypeScript. When validating a file,
the lock parses the content and inspects the resulting syntax tree for ERROR
nodes. Files containing structural errors—such as unbalanced braces, missing
semicolons, or malformed declarations—are rejected before the semantic lock
runs. Files with extensions not recognised by any configured parser are skipped
(pass through) to avoid blocking edits to configuration files, documentation,
or other non-code artefacts.

The validation reports each failure with:

- **Path**: The file that failed validation.
- **Line and column**: The position of the first syntax error.
- **Message**: A human-readable description (typically "syntax error").

This fast, local check catches many common agent mistakes without needing to
contact a language server.

### Pattern matching and rewriting

The `weaver-syntax` crate also provides a structural pattern matching engine
inspired by ast-grep. Patterns use metavariables (`$VAR` for single captures,
`$$$VAR` for multiple) to match and capture portions of the syntax tree. This
enables the future `observe grep` and `act apply-rewrite` commands to perform
precise, AST-aware search and transformation across the codebase. The engine
currently supports Rust, Python, and TypeScript.
