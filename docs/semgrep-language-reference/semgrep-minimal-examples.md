# Semgrep minimal examples pack

This guide curates a small, high-signal set of examples and maps each to the
reference assets in this directory:

- [Semgrep query language grammar](semgrep-query-language.ebnf)
- [Semgrep rule schema](semgrep-rule-schema.yaml)

## Recommended eight-file pack

| Example file                                                                | What it demonstrates                                                          | Maps to                                                                |
| --------------------------------------------------------------------------- | ----------------------------------------------------------------------------- | ---------------------------------------------------------------------- |
| `tests/syntax_v2/match.yaml`                                                | v2 `match` formula with `all`, `any`, `not`, and `inside`                     | Extended Backus–Naur Form (EBNF) `match-selector` and schema `match`   |
| `tests/syntax_v2/taint.yaml`                                                | New taint syntax with `sources`, `sinks`, `propagators`, and `sanitizers`     | Schema `taintObject`                                                   |
| `tests/syntax_v2/extract.yaml`                                              | Extract mode with `extract`, `dest-language`, and `reduce`                    | Schema extract-mode constraints                                        |
| `cli/tests/default/e2e/rules/match_based_id/join.yaml`                      | Join mode with inline rules and `on` predicates                               | Schema `joinObject`                                                    |
| `cli/tests/default/e2e/rules/message_interpolation/pattern-not-inside.yaml` | Legacy `patterns` with `pattern-not-inside` and `pattern`                     | EBNF legacy operators and schema `legacyFormulaObject`                 |
| `cli/tests/default/e2e/rules/metavariable-regex/metavariable-regex.yaml`    | `metavariable-regex` constraint under `patterns`                              | EBNF metavariable constraints and schema `metavariableRegex`           |
| `tests/patterns/rust/metavar_ellipsis_args.sgrep`                           | Rust host snippet with metavariable ellipsis `$...ARGS`                       | EBNF lexical tokens and Rust profile                                   |
| `tests/patterns/terraform/deep_expr_operator.sgrep`                         | HashiCorp Configuration Language (HCL) or Terraform deep ellipsis `<... ...>` | EBNF deep ellipsis and HCL profile                                     |

_Table 1: Recommended examples for validating v2 and legacy Semgrep features._

## Optional companion examples

- `tests/patterns/python/deep_cond.sgrep` for Python deep ellipsis patterns.
- `tests/patterns/ts/type_assert.sgrep` for TypeScript typed metavariables.
- `tests/patterns/go/deep_expr_lambda.sgrep` for Go deep-ellipsis statements.

## Quick inspection command

```bash
sed -n '1,120p' tests/syntax_v2/match.yaml \
  tests/syntax_v2/taint.yaml \
  tests/syntax_v2/extract.yaml \
  cli/tests/default/e2e/rules/match_based_id/join.yaml
```
