# Architectural decision record (ADR) 011: Sempai query input syntax

## Status

Proposed.

## Date

2026-08-16.

## Context and problem statement

ADR 007 makes Sempai one-liners first-class selectors under the noun-verb
`weaver symbols list` command. The Sempai technical design currently presents
the rich expression form as the ordinary one-liner:

```text
pattern("foo($X)") and not(regex("test"))
```

That syntax is appropriate when a query combines operators. It is needlessly
ceremonial for the dominant command-line case, where the entire query is one
positive host-language structural pattern. Requiring
`pattern("fn $NAME($...ARGS)")` introduces nested shell and DSL quotation before
the user can perform the smallest useful search.

One flag cannot safely auto-detect both forms. Host-language patterns may
contain words such as `pattern`, `and`, or `not`, while future expression syntax
may grow new prefixes. Auto-detection would make diagnostics and backwards
compatibility depend on heuristics.

File and standard-input forms also need explicit ownership so multiline and
heredoc input do not become a second parser path.

[RFC 0003](rfcs/0003-sempai-query-to-selector.md) contains the complete
vertical-slice proposal.

## Decision drivers

- Keep the common positive query concise.
- Preserve an explicit rich expression language.
- Avoid syntax auto-detection.
- Keep noun-verb command grammar from ADR 007.
- Make argument, file, and standard-input sources equivalent.
- Give parser diagnostics an unambiguous source kind.
- Preserve YAML rule-file interoperability.
- Make mutually exclusive inputs mechanically discoverable.
- Avoid shell-specific heredoc features in the parser.

## Options considered

### Option A: Bare `--query`, explicit `--expr`, and parallel file forms

Use `--query` for one positive host-language pattern. Use `--expr` for the rich
Sempai expression DSL. Add `--query-file` and `--expr-file`; `-` means standard
input. Keep `--rule` and `--rule-file` for YAML.

### Option B: Rich expressions under `--query`

Treat every query as an expression and require positive patterns to use
`pattern("...")`.

### Option C: Auto-detect pattern and expression text

Use one `--query` flag and infer the grammar from prefixes or parse success.

### Option D: Use `--pattern` for bare patterns and `--query` for expressions

This is explicit, but it makes the temporary structural-search noun the
shortest path and weakens ADR 007's decision that Sempai query is the public
selector abstraction.

| Topic | Option A | Option B | Option C | Option D |
| ----- | -------- | -------- | -------- | -------- |
| Common-case quoting | Minimal | Nested | Minimal | Minimal |
| Grammar ownership | Explicit | Explicit | Heuristic | Explicit |
| Backwards-compatible growth | Strong | Strong | Weak | Strong |
| Sempai as public abstraction | Strong | Strong | Strong | Weaker |
| Heredoc path | Explicit | Explicit | Ambiguous | Explicit |

_Table 1: Query input syntax alternatives._

## Decision statement

In the context of a resource-first CLI where positive structural patterns are
the common selector and rich Sempai formulas remain necessary, facing nested
quotation, ambiguous auto-detection, and future grammar growth, Weaver decides
for bare positive patterns under `--query`, rich formulas under `--expr`, and
parallel file or standard-input forms, and against mandatory pattern wrappers,
auto-detection, or a public `--pattern` primary path, to achieve concise
one-liners, deterministic parsing, and stable diagnostics, accepting that users
must choose between two explicit query flags.

## Decision outcome and proposed direction

Adopt option A.

The canonical forms are:

```sh
weaver symbols list \
  --lang rust \
  --query 'fn $NAME($...ARGS) { ... }'

weaver symbols list \
  --lang rust \
  --expr 'pattern("foo($X)") and not(regex("test"))'

weaver symbols list \
  --lang rust \
  --query-file queries/public-functions.sempai

weaver symbols list \
  --lang rust \
  --expr-file - <<'SEMPAI'
pattern("foo($X)")
and not(
  inside(pattern("tests::$X"))
)
SEMPAI

weaver symbols list --rule-file rules.yml
weaver symbols list --rule 'rules: [...]'
```

Exactly one query source is accepted:

- `--query <pattern>`;
- `--query-file <path|->`;
- `--expr <expression>`;
- `--expr-file <path|->`;
- `--rule <inline-yaml>`; or
- `--rule-file <path|->`.

`--query` and `--query-file` lower directly to a positive pattern atom.
`--expr` and `--expr-file` use the recovering Sempai DSL parser. `--rule` and
`--rule-file` use the YAML rule parser.

A value of `-` assigns standard input to that source. The command rejects more
than one standard-input owner and enumerates valid alternatives.

The parser does not interpret shell quoting, variable expansion, command
substitution, or heredoc delimiters. The shell removes those before Weaver
receives the resulting bytes.

## Relationship to ADR 007

This ADR refines the Sempai selector form described by ADR 007. It does not
change the resource-first command path, universal `--json`, or
capability-first provider model.

The prototype `weaver symbols list --pattern` remains an implementation pilot
only. If its engine graduates, it does so behind `--query`; its internal
spelling does not become the public contract.

## Parser and diagnostic consequences

The input kind is carried into the compilation request and diagnostic source
metadata. An error can therefore say whether it occurred in:

- a host-language pattern;
- a Sempai expression;
- inline YAML;
- a query file; or
- a rule file.

Bare patterns receive host-language snippet and metavariable diagnostics. Rich
expressions receive Logos and Chumsky token, delimiter, precedence, and semantic
diagnostics.

Recovering expression parsing may retain partial nodes for subsequent
diagnostics, but any error-severity diagnostic prevents execution.

## Compatibility and migration

Weaver is pre-0.1.0, so the proposed flags do not require a deprecated alias.
Documentation and generated command metadata must stop presenting wrapped
`pattern("...")` text as the ordinary positive query example.

The stable Sempai library may expose one `QuerySource` enum or separate
`compile_pattern`, `compile_dsl`, and `compile_yaml` methods. The CLI distinction
must not be discarded inside a stringly typed adapter.

## Consequences

- Common positive queries have one layer of shell quotation.
- Rich expressions remain explicit and extensible.
- Generated help has more mutually exclusive flags to explain.
- File and heredoc input use the same parser as argument input.
- Auto-detection cannot make a future keyword reinterpret an existing pattern.

## Known risks and limitations

- Users may initially expect `--query` to accept the complete expression
  language.
- Copying a rich expression into `--query` produces host-language diagnostics,
  not expression diagnostics.
- Standard input cannot simultaneously carry query text and another payload.

Generated help and error guidance must show both forms side by side and suggest
the alternate flag when the received shape strongly resembles the other input
kind.

## Outstanding decisions

- Decide whether file extensions should influence syntax highlighting in human
  diagnostics without influencing parser selection.
- Decide whether `--rule -` should remain invalid in favour of the unambiguous
  `--rule-file -`.
