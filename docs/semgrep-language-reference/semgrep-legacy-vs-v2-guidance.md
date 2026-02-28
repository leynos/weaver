# Semgrep legacy versus v2 guidance

This guide supports two audiences:

- rule authors deciding which syntax to use, and
- parser implementers deciding whether to support legacy syntax, v2 syntax, or
  both.

## Direct answers

### Is v2 finalized?

Not fully, if "finalized" means complete parser-path parity and full schema
coverage.

Current evidence indicates a mixed state:

- search parsing prefers `match` and then falls back to legacy `pattern*`
  formula parsing,
- extract mode still uses legacy formula parsing in some paths,
- some schema constructs are still marked `EXPERIMENTAL`, and
- some v2 or schema-level constructs are not represented uniformly across
  parser branches.

v2 is usable for core cases, but it is not complete across every path.

### Is legacy support needed for a greenfield parser?

For a strictly greenfield environment with controlled rule authoring, legacy
support is not required.

For interoperability with existing Semgrep rule corpora, legacy support remains
important unless a deterministic legacy-to-v2 normalization pass is provided.

Recommended sequencing:

- Minimum Viable Product (MVP): implement the v2 search subset first (`match`
  with `pattern`, `regex`, `all`, `any`, `not`, `inside`, and `anywhere`), then
- compatibility: add legacy `pattern*` parsing and normalize both syntaxes into
  one internal formula representation.

### Does the provided grammar fully cover v2?

No. It defines a practical core, not a full v2 specification.

The current grammar covers:

- core v2 search expression operators,
- lexical Semgrep tokens such as `$X`, `$_`, `$...ARGS`, and `...`, and
- host-language embedding profiles for Rust, Python, TypeScript, Go, and
  HashiCorp Configuration Language (HCL).

It does not fully cover:

- full host-language grammars,
- every mode-specific top-level contract, and
- every schema-level corner case or experimental variant.

## Legacy versus v2 comparison

| Topic                       | Legacy                                            | v2 (`match`)                           | Guidance                                |
| --------------------------- | ------------------------------------------------- | -------------------------------------- | --------------------------------------- |
| Search formula entry        | `pattern*` keys                                   | `match` object or string               | Prefer v2 for new rules                 |
| Negation and context        | `pattern-not`, `pattern-inside`, and related keys | `not`, `inside`, `anywhere`            | v2 is clearer                           |
| Conjunction and disjunction | `patterns`, `pattern-either`                      | `all`, `any`                           | v2 names align with formula AST         |
| Decorators and attachments  | Scattered across legacy fields                    | `where`, `as`, `fix` on `match` object | Prefer v2                               |
| Parser parity               | Broad historical coverage                         | Partial parity                         | Keep compatibility path as needed       |
| Ecosystem compatibility     | Highest today                                     | Growing                                | Support both for broad interoperability |

_Table 1: Legacy and v2 comparison for parser and rule-authoring decisions._

## Implementation guidance for a new parser

### Option A: Greenfield-only parser

Implement only the v2 search subset and reject legacy rules with explicit
diagnostics.

Use this option when all rule authors are controlled and no existing Semgrep
rule corpus must be ingested.

### Option B: Compatibility parser

Implement both syntaxes and normalize to a shared formula AST:

1. Parse v2 `match`.
2. Parse legacy `pattern*` forms.
3. Lower both to shared constructors such as `P`, `Not`, `Inside`, `Anywhere`,
   `And`, and `Or`.
4. Apply shared semantic checks such as `InvalidNotInOr` and
   `MissingPositiveTermInAnd`.

Use this option when interoperability with existing Semgrep rules is required.

## Source pointers

The following pointers reference upstream Semgrep implementation files and line
ranges at the time this guide was compiled.[^1][^2][^3][^4]

[^1]: `src/parsing/Parse_rule.ml` — lines `648`, `1011`, and `1026`.
[^2]: `src/parsing/Parse_rule_formula.ml` — lines `315`, `739`, and `954`.
[^3]: `src/rule/Rule.ml` — line `63`.
[^4]: <https://raw.githubusercontent.com/semgrep/semgrep-interfaces/7e509db48c700cae49fe0372e2aa0410fa86d867/rule_schema_v1.yaml>

## Related documents in this repository

- [Semgrep query language grammar](semgrep-query-language.ebnf)
- [Semgrep rule schema](semgrep-rule-schema.yaml)
- [Semgrep operator precedence](semgrep-operator-precedence.md)
