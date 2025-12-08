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
- `--daemon-socket <ENDPOINT>` — overrides the daemon transport. Accepts values
  such as `unix:///run/user/1000/weaver.sock` or `tcp://127.0.0.1:9779`.
- `--log-filter <FILTER>` — sets the tracing filter (defaults to `info`).
- `--log-format <FORMAT>` — selects the log output format (`json` or `compact`).
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
to pin the daemon socket, switch to a compact log format, and force the rename
capability for Python:

```toml
daemon_socket = { transport = "tcp", host = "127.0.0.1", port = 9779 }
log_filter = "info"
log_format = "compact"

[[capability_overrides]]
language = "python"
capability = "act.rename-symbol"
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

During bootstrap, the daemon emits structured telemetry using `tracing`. Events
named `bootstrap_starting`, `bootstrap_succeeded`, and `bootstrap_failed` make
the lifecycle visible in observability pipelines. The output format (JSON or
compact) respects the shared configuration, so operators see consistent logs
between the CLI and daemon. Semantic Fusion backends are started lazily: the
first request for a backend triggers a `backend_starting` event, followed by
either `backend_ready` or `backend_failed`. This keeps the daemon lightweight
until specific capabilities are required, while still surfacing failures as
structured events.

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

Lifecycle commands never contact the daemon's JSONL transport. They operate
solely on the shared runtime files exported by `weaver-config`, ensuring the
CLI and daemon use the exact same directory layout even when the daemon socket
is overridden.

## Command usage

`weaver` expects commands to be specified as a two-level verb pair. The first
argument selects the domain (`observe`, `act`, or `verify`), while the second
argument names the operation. All subsequent tokens are forwarded verbatim to
the daemon and are encoded into the JSONL request without interpretation. For
example:

```sh
weaver observe get-definition --uri file:///workspace/main.rs --position 42:17
```

The CLI serialises this invocation as:

```json
{"command":{"domain":"observe","operation":"get-definition"},"arguments":["--uri","file:///workspace/main.rs","--position","42:17"]}
```

Responses from the daemon are emitted as JSON objects, each tagged with a
`stdout` or `stderr` stream. The CLI writes the payload to the corresponding
host stream and terminates using the exit status provided by the final
`{"kind":"exit","status":...}` message. Errors encountered while loading
configuration or parsing the command are written to standard error before the
process exits with status `1`.

Daemon connections are attempted with a five-second timeout. When the daemon
does not accept a request within that window, the CLI aborts with a descriptive
error instead of hanging. Likewise, if the daemon sends ten consecutive blank
lines, the CLI emits a warning, stops reading further, and reports failure
unless an exit status was already observed. Any session that ends without an
explicit exit message is treated as an error, so callers do not misinterpret a
partial response as success.

## Language server capability detection

The new `weaver-lsp-host` crate initialises the LSP servers for Rust, Python,
and TypeScript and records which core requests each server advertises:
`textDocument/definition`, `textDocument/references`, and diagnostics. These
advertised capabilities are merged with any overrides provided via
`capability_overrides` in `weaver-config`. `force` directives allow a request
even when the server claims not to support it, while `deny` directives block
the request regardless of the server report. When a request is rejected, the
error explains whether the feature was disabled by configuration or simply
absent from the server so operators and agents can adjust their plans without
guesswork.

### Capability probe

The capability matrix negotiated through configuration overrides can be
inspected without starting the daemon:

```sh
weaver --capabilities
```

The CLI loads the shared configuration, applies any override directives, and
prints the resulting matrix as pretty-printed JSON. The probe does not contact
`weaverd`, making it safe to run during planning stages or health checks.

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

### Backend unavailability

If a language server is not running or crashes mid-request, the semantic lock
returns a backend-unavailable error rather than silently passing. Operators
should ensure the appropriate language servers are healthy before executing
`act` commands.

### Placeholder implementation note

The current syntactic lock uses a placeholder implementation that always passes.
Full Tree-sitter integration will be delivered in a future phase. The semantic
lock relies on the `weaver-lsp-host` infrastructure, which requires language
servers to be registered and initialised for the relevant languages.
