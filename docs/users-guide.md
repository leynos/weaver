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
