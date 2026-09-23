# Architectural decision record (ADR) 013: Linux runtime library mounts

## Status

Accepted.

## Date

2026-09-23.

## Context and problem statement

On Linux, dynamically linked commands need their runtime libraries and loader
inside the sandbox. Mounting these directories with Birdcage's `Read`
permission blocks executable memory mappings. The loader then fails when it
tries to map a library with `mmap(PROT_EXEC)`. Canonicalizing aliases such as
`/lib64` before Birdcage creates the sandbox also loses the path spelling the
loader uses to find its interpreter on merged-usr systems.

Runtime support must let an explicitly authorized command start without
authorizing arbitrary initial commands or broadening caller-selected read paths.

## Decision drivers

- Preserve the caller's exact executable authorization boundary.
- Permit dynamic loaders to map trusted runtime libraries as executable.
- Preserve the path aliases used by the host's loader.
- Keep the defaults platform-specific and narrow.

## Decision

Keep Linux runtime roots in a private profile field, separate from caller
read-only paths and the exact executable allowlist. Preserve each existing
root's original spelling and pass it to Birdcage as `ExecuteAndRead`. Birdcage
canonicalizes the bind source and recreates the requested alias inside the
sandbox. Authorization of the initial command continues to compare its
canonical path only with caller-supplied executable paths.

The runtime-root list is owned by `weaver-sandbox` and initialized by
`SandboxProfile::new`. It composes only into sandbox exception construction; it
is not exposed as a caller policy builder or merged into the executable
allowlist. Non-Linux defaults, including macOS, remain unchanged.

## Consequences

- Linux dynamically linked commands can map runtime libraries as executable.
- The default profile grants execute-and-read access to existing standard
  runtime roots, including their original aliases.
- A runtime root does not authorize it as the initial command; callers must
  still allow the command explicitly.
- Caller read-only paths remain non-writable. When they overlap a runtime
  root, Birdcage unions the grants and keeps executable mappings enabled. An
  explicit overlapping write grant also follows Birdcage's permission union;
  runtime roots are read-only only in the default profile.
- macOS sandbox defaults are unaffected.
