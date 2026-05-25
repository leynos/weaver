# Weaver `act refactor` rename position migration guide

## What changed

`weaver act refactor` rename now uses `--position LINE:COL` as the supported
way to identify the rename target. The deprecated trailing
`offset=<BYTE_OFFSET>` form is still accepted when `--position` is absent, but
it emits this warning on stderr:

```text
Warning: 'offset=' is deprecated; use '--position LINE:COL' instead.
```

## Upgrade examples

Replace the trailing byte offset with a line and column pair:

```bash
weaver act refactor --provider rope --refactoring rename --file src/lib.rs offset=128 new_name=renamed
```

becomes:

```bash
weaver act refactor --provider rope --refactoring rename --file src/lib.rs --position 12:9 new_name=renamed
```

When a line and column are available from editor or source context, pass them
with `--position`. Values are one-indexed.

## Notes

- Do not pass both `--position` and `offset=` together; that combination is
  rejected.
- Any remaining `offset=` use should be treated as transitional only.
