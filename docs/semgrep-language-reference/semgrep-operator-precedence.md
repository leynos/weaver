# Semgrep query operator precedence and Pratt binding powers

This document records a parser-backed precedence normalization model for
Semgrep query expressions.

Important scope note:

- Semgrep formulas are parsed from structured YAML keys, not from a classical
  infix token stream.
- Explicit nesting is therefore authoritative.
- The ladder and Pratt table below are a normalization model over formula
  constructors such as `P`, `Not`, `Inside`, `Anywhere`, `And`, and `Or`.

## Precedence ladder

Highest to lowest in the normalization model:

1. Atomics: `pattern`, `regex` (legacy: `pattern`, `pattern-regex`).
2. Context wrappers: `inside`, `anywhere` (legacy: `pattern-inside`,
   `semgrep-internal-pattern-anywhere`).
3. Negation: `not` (legacy: `pattern-not`; composite forms include
   `pattern-not-regex` and `pattern-not-inside`).
4. Conjunction: `all` (legacy: `patterns`).
5. Disjunction: `any` (legacy: `pattern-either`).
6. Decorator layer: `where`, `as`, `fix` (post-parse attachments).

Parser-enforced semantic constraints:

- `pattern-either` rejects direct `Not` children (`InvalidNotInOr`).
- `patterns` and `all` reject conjunctions with no positive terms
  (`MissingPositiveTermInAnd`) except in metavariable-pattern contexts.

## Pratt binding powers

The table fields follow `{token, fixity, precedence_level, associativity}`.

| token                               | fixity                                | precedence_level | associativity |
| ----------------------------------- | ------------------------------------- | ---------------- | ------------- |
| `pattern`                           | atom (`nud`)                          | 90               | n/a           |
| `regex`                             | atom (`nud`)                          | 90               | n/a           |
| `pattern-regex`                     | atom (`nud`)                          | 90               | n/a           |
| `inside`                            | prefix                                | 80               | right         |
| `pattern-inside`                    | prefix                                | 80               | right         |
| `anywhere`                          | prefix                                | 80               | right         |
| `semgrep-internal-pattern-anywhere` | prefix                                | 80               | right         |
| `not`                               | prefix                                | 70               | right         |
| `pattern-not`                       | prefix                                | 70               | right         |
| `pattern-not-regex`                 | prefix composite (`not(regex(...))`)  | 70 / 90          | right         |
| `pattern-not-inside`                | prefix composite (`not(inside(...))`) | 70 / 80          | right         |
| `all`                               | n-ary conjunction                     | 40               | associative   |
| `patterns`                          | n-ary conjunction                     | 40               | associative   |
| `any`                               | n-ary disjunction                     | 30               | associative   |
| `pattern-either`                    | n-ary disjunction                     | 30               | associative   |
| `where`                             | decorator attachment                  | 20               | n/a           |
| `as`                                | decorator attachment                  | 20               | n/a           |
| `fix`                               | decorator attachment                  | 20               | n/a           |

_Table 1: Binding-power normalization model for Semgrep formula operators._

Suggested Pratt numeric mapping for implementations:

- atoms: `lbp=0`, `rbp=0`,
- prefix wrappers (`inside`, `anywhere`, `not`): `lbp=0`, `rbp` at their
  precedence level,
- conjunction: `lbp=40`, `rbp=41`, and
- disjunction: `lbp=30`, `rbp=31`.

## DOT graph

See [Semgrep operator precedence DOT graph](semgrep-operator-precedence.dot).

## Source evidence

The following pointers reference upstream Semgrep parser and rule files:

- `src/parsing/Parse_rule.ml` around line `648`.
- `src/parsing/Parse_rule_formula.ml` around lines `367`, `399`, `483`,
  `739`, and `969`.
- `src/rule/Rule.ml` around lines `63` and `897`.
