# Weaver command-line interface (CLI) user-interface gap analysis

This document records a comprehensive audit of the `weaver` CLI's
discoverability and help surfaces, identifies concrete gaps, and proposes
remedies for each. The guiding principle is that **a user must never feel at a
loss as to what to do next**: every level of the command hierarchy must
advertise the available subcommands, domains, operations, plugins, and
parameters so that the tool is self-documenting.

## Methodology

The analysis was conducted by:

1. Reading every help surface the binary exposes today (`--help`, `-h`,
   `help`, bare invocation, per-subcommand help).
2. Tracing the clap definition in
   [`crates/weaver-cli/src/cli.rs`](../crates/weaver-cli/src/cli.rs) and the
   domain router in
   [`crates/weaverd/src/dispatch/router.rs`](../crates/weaverd/src/dispatch/router.rs).
3. Exercising error paths for unknown domains, unknown operations,
   missing arguments, and missing operations.
4. Reviewing the plugin registry, the daemon dispatch table, and the
   output-rendering pipeline.
5. Cross-referencing against the user's guide
   ([`docs/users-guide.md`](users-guide.md)) and the generated manual page.

The sections below are ordered from the outermost user interaction (bare
invocation) inward to the deepest (individual operation arguments).

______________________________________________________________________

## Level 0 — bare invocation (`weaver`)

Running `weaver` with no arguments currently produces:

```plaintext
the command domain must be provided
```

Exit code 1. No further guidance.

A newcomer receives only a terse error message. There is no hint that `--help`
is available, no listing of domains, no mention of `daemon`, and no pointer to
documentation. The message does not explain what a "domain" is.

**Recommended remedy.** When neither a domain nor a structured subcommand is
supplied, emit the short help text (`-h` form) automatically instead of an
unadorned error string. This is the behaviour users expect from every
mainstream CLI tool.

*Alternative:* print a purpose-built "getting started" block that lists
domains, the `daemon` subcommand, and the `--help` flag.

______________________________________________________________________

## Level 1 — top-level help (`weaver --help` / `weaver -h`)

Running `weaver --help` currently produces:

```plaintext
Command-line interface for the Weaver semantic code tool

Usage: weaver [OPTIONS] [DOMAIN] [OPERATION] [ARG]...

Commands:
  daemon  Runs daemon lifecycle commands

Arguments:
  [DOMAIN]     The command domain (for example `observe`)
  [OPERATION]  The command operation (for example `get-definition`)
  [ARG]...     Additional arguments passed to the daemon

Options:
      --capabilities     Prints the negotiated capability matrix …
      --output <OUTPUT>  Controls how daemon output is rendered …
  -h, --help             Print help
```

### Gap 1a — domains not enumerated

The help text gives one example (`observe`) but never lists all three domains
(`observe`, `act`, `verify`). The user must already know the domain names or
consult external documentation.

**Remedy.** Add a `long_about` or `after_help` block to `Cli` that lists all
three domains with a one-line description for each.

### Gap 1b — operations not enumerated

Only `get-definition` appears as a parenthetical example. The user cannot
discover that `find-references`, `apply-patch`, `refactor`, etc. exist.

**Recommended remedy.** Include the full operation list per domain in the same
after-help block used for gap 1a.

*Alternative:* add a `weaver list-operations` introspection command.

### Gap 1c — configuration flags invisible

The five `ortho-config` flags (`--config-path`, `--daemon-socket`,
`--log-filter`, `--log-format`, `--capability-overrides`) are stripped before
clap parses and therefore never appear in help output. See
[Level 6](#level-6--configuration-flags-invisible-in-help) for the full
analysis and remedy.

### Gap 1d — no `--version` flag

The `Cli` struct does not derive or declare a version. Running
`weaver --version` produces:

```plaintext
error: unexpected argument '--version' found
```

Standard CLI expectation violated; packaging and bug-reporting workflows are
harder.

**Remedy.** Add `version` to the `#[command(...)]` attribute on `Cli` so clap
auto-generates `--version` / `-V`.

### Gap 1e — no long description or after-help text

There is no `about`, `long_about`, or `after_help` on the top-level `Cli`
struct that would describe Weaver's purpose, architecture, or give a
quick-start example.

**Remedy.** Add `about` and `long_about` attributes describing Weaver's purpose
and a one-line quick-start example.

### Gap 1f — `help` subcommand disabled

`disable_help_subcommand = true` means `weaver help` is parsed as
`DOMAIN = "help"`, which fails with "the command operation must be provided".
Users accustomed to `<tool> help <topic>` patterns get a confusing error.

**Recommended remedy.** Re-enable the `help` subcommand (remove
`disable_help_subcommand = true`).

*Alternative:* intercept the `help` domain token in the CLI and print
contextual help.

### Gap 1g — plugin listing absent

There is no mechanism to list registered plugins (actuators, sensors), their
supported languages, or their available refactoring operations. Users must
guess provider names (e.g. `rope`, `rust-analyzer`) or read the source.

**Remedy.** Add a `weaver list-plugins` (or `weaver plugins`) introspection
subcommand. It should query the daemon (or a static registry) and print plugin
name, kind, languages, and version.

______________________________________________________________________

## Level 2 — domain without operation (`weaver observe`)

Running `weaver observe`, `weaver act`, or `weaver verify` without an operation
currently produces:

```plaintext
the command operation must be provided
```

Exit code 1. No further guidance.

The user has correctly identified a domain but receives no indication of which
operations exist within it. The error is generated client-side in
[`command.rs`](../crates/weaver-cli/src/command.rs)
(`AppError::MissingOperation`) before any daemon communication occurs, so the
daemon's knowledge of valid operations is not surfaced.

**Recommended remedy.** When a domain is provided without an operation, emit a
contextual help block listing the valid operations for that domain. The
recommended approach is a hard-coded table in the CLI mirroring the daemon's
`DomainRoutingContext::known_operations`, since this avoids a daemon
round-trip. The message should follow the pattern:

```plaintext
error: operation required for domain 'observe'

Available operations:
  get-definition   Retrieve the definition location for a symbol
  find-references  Find all references to a symbol
  grep             Structural pattern search
  diagnostics      Retrieve compiler diagnostics
  call-hierarchy   Show the call graph for a symbol

Run 'weaver observe <operation> --help' for operation details.
```

______________________________________________________________________

## Level 3 — unknown domain (`weaver bogus something`)

The CLI sends the request to the daemon (or attempts auto-start). If the daemon
is not running, the user sees:

```plaintext
Waiting for daemon start...
failed to spawn weaverd binary '"weaverd"': No such file or directory
```

If the daemon is running, it returns:

```plaintext
unknown domain: bogus
```

The unknown-domain error does not suggest valid domains. A typo (e.g. `observe`
vs `obsrve`) produces an opaque rejection with no "did you mean?" hint.
Additionally, the error is only available at the daemon layer — the CLI could
validate the domain before attempting a daemon connection or auto-start.

**Recommended remedy.** Validate the domain client-side before connecting to
the daemon and include the list of valid domains in the error output:

```plaintext
error: unknown domain 'obsrve'

Valid domains: observe, act, verify
```

This avoids unnecessary auto-start attempts for clearly invalid input.

*Alternative:* additionally apply edit-distance matching to suggest the closest
valid domain (e.g. "did you mean 'observe'?").

______________________________________________________________________

## Level 4 — unknown operation (`weaver observe nonexistent`)

When the daemon is running, it returns:

```plaintext
unknown operation 'nonexistent' for domain 'observe'
```

Exit code 1. The error does not list valid operations for the domain. The
daemon has all the information needed (it holds the `known_operations` arrays)
but does not include it in the error response.

**Remedy.** Extend the `DispatchError::UnknownOperation` path to include the
known operations list:

```plaintext
error: unknown operation 'get-def' for domain 'observe'

Available operations: get-definition, find-references, grep,
diagnostics, call-hierarchy
```

______________________________________________________________________

## Level 5 — operation without required arguments

### Gap 5a — `observe get-definition` without arguments

Running `weaver observe get-definition` (without `--uri` and `--position`)
attempts a daemon connection or auto-start. If the daemon is available, it
returns:

```plaintext
observe get-definition requires --uri and --position arguments
```

Two sub-problems exist:

- **No client-side pre-validation.** The CLI does not know what
  arguments each operation needs, so it cannot catch missing parameters before
  contacting the daemon.
- **No `--help` at operation level.** Running
  `weaver observe get-definition --help` prints the *top-level* help because
  `--help` is consumed by clap before the trailing arguments reach the daemon.
  The user has no way to discover operation parameters from the CLI.

**Recommended remedy.** Model each domain as a clap subcommand containing its
own subcommands (one per operation). This gives full clap-generated help at
every level — including `weaver observe get-definition --help` — and is the
most thorough approach, though it requires significant restructuring of
[`cli.rs`](../crates/weaver-cli/src/cli.rs).

*Alternatives (lighter-weight):*

- **Daemon-side `--help` interception.** When the daemon receives an
  operation with `--help` in the arguments, respond with a help payload instead
  of executing the operation.
- **Introspection subcommand.** A `weaver help observe get-definition`
  command that queries the daemon (or a static schema) and prints the expected
  arguments, types, and examples.

### Gap 5b — `act refactor` without arguments

Running `weaver act refactor` (without flags) produces:

```plaintext
act refactor requires --provider <plugin-name>
```

The error identifies the first missing flag but not the full set of required
flags (`--provider`, `--refactoring`, `--file`), nor does it list valid
providers or refactoring operations.

**Remedy.** Return a comprehensive error listing all required parameters, valid
provider names, and valid refactoring operations:

```plaintext
error: act refactor requires the following arguments:

  --provider <PLUGIN>        Registered plugin name (rope, rust-analyzer)
  --refactoring <OPERATION>  Refactoring to perform (rename)
  --file <PATH>              Workspace-relative file path

  KEY=VALUE...               Extra arguments forwarded to the plugin

Run 'weaver list-plugins' to see registered plugins.
```

______________________________________________________________________

## Level 6 — configuration flags invisible in help

The five configuration flags (`--config-path`, `--daemon-socket`,
`--log-filter`, `--log-format`, `--capability-overrides`) are consumed by
`split_config_arguments` before the remaining tokens reach clap. They work
correctly at runtime, but are completely absent from all help output.

An operator who runs `weaver --help` to discover how to connect to a
non-default daemon socket finds no relevant flag listed. The flags are
documented only in [`docs/users-guide.md`](users-guide.md) and the source code.

**Recommended remedy.** Register the five flags as clap arguments on `Cli` so
they appear in `--help`. They do not need to participate in clap's parsing
pipeline (they can be `global = true, hide = false` arguments that are read by
the config splitter), but they must be visible in the help output.

*Alternative:* document them in an `after_help` block if registering them as
clap arguments would conflict with the `ortho-config` loader.

______________________________________________________________________

## Level 7 — plugin discoverability

There is no command to list plugins. The user must know the provider name
(`rope`, `rust-analyzer`) from the documentation or source code. There is no
way to discover which plugins are registered, which languages each plugin
supports, which refactoring operations a plugin offers, or the version of each
plugin.

The plugin registry (`PluginRegistry`) has the application programming
interface (API) surface to answer all of these queries (`find_by_kind`,
`find_for_language`, `find_actuator_for_language`), but this information is not
exposed through the CLI.

**Remedy.** Add one or more introspection commands:

```plaintext
weaver list-plugins               List all registered plugins
weaver list-plugins --kind actuator   Filter by kind
weaver list-plugins --language python  Filter by language
```

Example output:

```plaintext
NAME             KIND      LANGUAGES  VERSION  TIMEOUT
rope             actuator  python     0.1.0    30s
rust-analyzer    actuator  rust       0.1.0    60s
```

**Recommended implementation:** add a new top-level clap subcommand
`list-plugins` alongside `daemon` that constructs the same static registry the
daemon uses and queries it locally (no daemon round-trip required).

*Alternative:* implement `list-plugins` as a daemon operation, so the output
always reflects the live registry.

______________________________________________________________________

## Level 8 — `daemon` subcommand help

`weaver daemon --help` is adequate — it lists `start`, `stop`, and `status`
with one-line descriptions.

`weaver daemon start --help` shows only `Usage: weaver daemon start` and the
`-h` / `--help` option. It does not mention the configuration flags that affect
startup (`--daemon-socket`, `--log-filter`, `--config-path`), nor does it
describe the `WEAVERD_BIN` or `WEAVER_FOREGROUND` environment variable
overrides.

**Remedy.** Once configuration flags are surfaced in help (level 6 remedy),
they will naturally appear in the `daemon start` help if marked as
`global = true`. Additionally, mention `WEAVERD_BIN` and `WEAVER_FOREGROUND` in
the `long_about` or `after_help` text for `daemon start`.

______________________________________________________________________

## Level 9 — `--capabilities` output

`weaver --capabilities` prints a JavaScript Object Notation (JSON) document
showing capability overrides. When no overrides are configured, it prints:

```json
{
  "languages": {}
}
```

The capabilities probe shows overrides only, not the full negotiated matrix. A
user cannot determine which operations are actually available for which
languages without starting a daemon and exercising each operation.

**Remedy (lower priority).** Consider extending the capabilities probe to merge
the server-reported capabilities with the overrides, producing a complete
available-capabilities matrix. This requires daemon interaction and is a larger
change, so it may be deferred. At minimum, the current output should include a
note explaining that the matrix shows overrides only and that actual capability
negotiation occurs at runtime.

______________________________________________________________________

## Level 10 — error messages and exit codes

| #   | Scenario                             | Current message                                                         | Missing guidance                                                |
| --- | ------------------------------------ | ----------------------------------------------------------------------- | --------------------------------------------------------------- |
| 10a | Daemon not running, auto-start fails | `failed to spawn weaverd binary '"weaverd"': No such file or directory` | Does not suggest installing `weaverd` or setting `WEAVERD_BIN`. |
| 10b | Unknown domain (daemon)              | `unknown domain: bogus`                                                 | Does not list valid domains.                                    |
| 10c | Unknown operation (daemon)           | `unknown operation 'x' for domain 'y'`                                  | Does not list valid operations.                                 |
| 10d | Missing domain                       | `the command domain must be provided`                                   | Does not list domains or point to `--help`.                     |
| 10e | Missing operation                    | `the command operation must be provided`                                | Does not list operations or point to domain help.               |

**Remedy.** Each error message should include actionable next steps. A
consistent pattern would be:

```plaintext
error: <what went wrong>

<list of valid alternatives or required arguments>

Run 'weaver --help' for more information.
```

______________________________________________________________________

## Level 11 — manpage

A manual page is auto-generated via `clap_mangen` during the build. It reflects
the same clap-derived content that `--help` shows.

Because the manpage is generated from the same clap model that lacks domain
enumeration, operation listing, configuration flags, and per-operation help,
the manpage inherits all the same deficiencies.

**Remedy.** Fixing the clap model (levels 1–6 above) will automatically improve
the manpage. No separate manpage-specific work is required beyond ensuring
`after_help` content renders correctly in troff.

______________________________________________________________________

## Level 12 — `weaver help` subcommand

`weaver help` is parsed as `DOMAIN = "help"` because
`disable_help_subcommand = true` is set. This produces:

```plaintext
the command operation must be provided
```

The `help` subcommand is a universal CLI convention. Disabling it creates a
trap for users who reflexively type `weaver help`.

**Recommended remedy.** Re-enable the help subcommand by removing
`disable_help_subcommand = true`.

*Alternative (enhanced):* extend the re-enabled subcommand to support
topic-based help:

```plaintext
weaver help                 Show general help
weaver help observe         List operations in the observe domain
weaver help act refactor    Show act refactor parameter reference
weaver help plugins         List registered plugins
```

______________________________________________________________________

## Summary of gaps and priority

| Priority | Level | Gap summary                                  | Effort         |
| -------- | ----- | -------------------------------------------- | -------------- |
| P0       | 0     | Bare invocation gives no guidance            | Small          |
| P0       | 1a    | Domains not listed in help                   | Small          |
| P0       | 1b    | Operations not listed in help                | Small          |
| P0       | 2     | Missing operation gives no alternatives      | Small          |
| P0       | 1d    | No `--version` flag                          | Trivial        |
| P1       | 1c    | Config flags invisible in help               | Medium         |
| P1       | 3     | Unknown domain gives no suggestions          | Small          |
| P1       | 4     | Unknown operation gives no suggestions       | Small          |
| P1       | 5a    | No operation-level help                      | Medium–Large   |
| P1       | 5b    | Refactor error lists only first missing flag | Small          |
| P1       | 10    | Error messages lack actionable guidance      | Medium         |
| P1       | 12    | `weaver help` broken                         | Small          |
| P2       | 1e    | No long description or after-help            | Small          |
| P2       | 1f    | `help` subcommand disabled                   | Small          |
| P2       | 7     | No plugin listing command                    | Medium         |
| P2       | 8     | Daemon start help lacks config/env detail    | Small          |
| P3       | 9     | Capabilities probe shows overrides only      | Medium         |

______________________________________________________________________

## Recommended implementation order

1. **Quick wins (P0):** Add `version` to `Cli`. List domains and
   operations in `after_help`. Improve bare-invocation and missing-operation
   error messages. These changes touch only
   [`cli.rs`](../crates/weaver-cli/src/cli.rs), [`command.rs`](../crates/weaver-cli/src/command.rs),
    and [`errors.rs`](../crates/weaver-cli/src/errors.rs).

2. **Error message enrichment (P1):** Add valid-alternative listings
   to unknown-domain and unknown-operation errors in both the CLI and the
   daemon router. Surface all required arguments in `act refactor` errors.

3. **Configuration flag visibility (P1):** Register the five config
   flags as clap arguments (even if parsing remains in `ortho-config`) so they
   appear in help output.

4. **Help subcommand and operation-level help (P1–P2):** Re-enable
   the help subcommand and implement topic-based help. Optionally restructure
   the clap model to use nested subcommands for full per-operation `--help`.

5. **Plugin introspection (P2):** Add a `list-plugins` command.

6. **Capability matrix enrichment (P3):** Extend `--capabilities` to
   merge runtime capabilities.
