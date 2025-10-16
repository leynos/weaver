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
explicit exit message is treated as an error so callers do not misinterpret a
partial response as success.

### Capability probe

The capability matrix negotiated through configuration overrides can be
inspected without starting the daemon:

```sh
weaver --capabilities
```

The CLI loads the shared configuration, applies any override directives, and
prints the resulting matrix as pretty-printed JSON. The probe does not contact
`weaverd`, making it safe to run during planning stages or health checks.
