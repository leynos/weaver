# Illustrative Pratt parser example for DDlog expressions

This document is an illustrative example of how a Pratt parser for DDlog-style
expressions can be structured. It is reference material for parser authors and
is not a design proposal for a Weaver runtime component or roadmap task.

The example leverages the `chumsky` crate's Pratt parsing capabilities, because
they provide a concise way to express precedence-driven grammars.

______________________________________________________________________

## 1. Core concepts and illustrative integration approach

The parser will be implemented as a `chumsky` parser combinator that consumes
the token stream produced by the existing `logos`-based tokenizer. It will
produce a structured expression Abstract Syntax Tree (AST), which will then be
integrated as a node (`N_EXPR_NODE`) within a larger `rowan` Concrete Syntax
Tree (CST).

The key steps are:

1. **Define the Expression AST**: Create Rust `enum`s and `struct`s to
   represent the structure of all possible DDlog expressions.

2. **Define Operator Precedence and Associativity**: Translate the operator
   table from the Haskell parser analysis into a `chumsky` Pratt parser
   definition.

3. **Implement the Parser**: Build the `chumsky` parser using `chumsky::pratt`.

4. **Integrate with the CST**: Ensure that when the expression parser is
   invoked, the resulting AST is correctly represented within a `rowan`
   `GreenNode`.

### 1.1 Decision: qualified calls versus unresolved applications

To align with the updated DDlog syntax specification, the Pratt parser now
distinguishes between parse-time qualified function calls and unresolved
applications:

- Only fully scoped, lower-case terminal names such as `pkg::math::sum(...)`
  parse as `Expr::Call`.
- Bare or otherwise non-qualified forms such as `sum(...)`, `Foo(...)`, and
  `(f)(x)` parse as `Expr::Apply`.

This keeps parse-time behaviour deterministic while deferring semantic
disambiguation of bare names to the later name-resolution phase.

______________________________________________________________________

## 2. Expression AST definition

First, the parser needs a data structure to represent the parsed expressions.
This will live alongside the other AST definitions in `src/parser/ast/`.

```rust,no_run
// In a new file, e.g., src/parser/ast/expr.rs

#[derive(Debug, Clone, PartialEq)]
pub enum Expr {
    Literal(Literal),
    Variable(String),
    UnaryOp {
        op: UnaryOp,
        expr: Box<Expr>,
    },
    BinaryOp {
        op: BinaryOp,
        lhs: Box<Expr>,
        rhs: Box<Expr>,
    },
    Call {
        callee: Box<Expr>,
        args: Vec<Expr>,
    },
    Tuple(Vec<Expr>),
    // … other expression types like Struct, Match, If-Else, etc.
}

#[derive(Debug, Clone, PartialEq)]
pub enum Literal {
    String(String),
    Number(String), // Keep as string to preserve original format
    Bool(bool),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnaryOp {
    Not,
    Neg,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinaryOp {
    // Arithmetic
    Add, Sub, Mul, Div, Mod,
    // Comparison
    Eq, Neq, Lt, Lte, Gt, Gte,
    // Logical
    And, Or,
    // Bitwise
    BitAnd, BitOr, Shl, Shr,
    // Assignment
    Assign,
    // Type
    Ascribe, Cast,
    // Sequencing and control
    Seq, Imply,
    // Other
    Concat, // '++' in DDlog
}
```

______________________________________________________________________

## 3. Pratt parser implementation with `chumsky`

The heart of the implementation uses `chumsky::pratt`. This requires defining
the atoms—the simplest parts of an expression such as literals or variables—and
then defining the operators and their binding power (precedence).

This logic would be added to the `span_scanner.rs` or a new module it delegates
to.

```rust,no_run
// In a parser module, e.g., src/parser/expression_parser.rs

use chumsky::prelude::*;
use chumsky::pratt::*;
use crate::{SyntaxKind, ast}; // Assuming ast::Expr is defined

pub fn expression_parser() -> impl Parser<SyntaxKind, ast::Expr, Error = Simple<SyntaxKind>> {
    // 1. Define the parser for atomic expressions (the base cases)
    let atom = select! {
        SyntaxKind::T_NUMBER => ast::Expr::Literal(ast::Literal::Number),
        SyntaxKind::T_STRING => ast::Expr::Literal(ast::Literal::String(
            ast::StringLiteral {
                body: String::new(), // populate from the token span
                kind: ast::StringKind::Standard { interpolated: false },
                interned: false,
            },
        )),
        SyntaxKind::K_TRUE => ast::Expr::Literal(ast::Literal::Bool(true)),
        SyntaxKind::K_FALSE => ast::Expr::Literal(ast::Literal::Bool(false)),
        SyntaxKind::T_IDENT => ast::Expr::Variable,
    }.or(recursive(|expr| { // For parenthesised expressions
        expr.delimited_by(just(SyntaxKind::T_LPAREN), just(SyntaxKind::T_RPAREN))
    }));

    // 2. Define the operator table using chumsky::pratt
    pratt((
        // == Prefix operators (e.g., -, !) ==
        Operator::new(
            just(SyntaxKind::T_MINUS),
            7,
            |rhs| ast::Expr::UnaryOp {
                op: ast::UnaryOp::Neg,
                expr: Box::new(rhs),
            },
        ),
        Operator::new(
            just(SyntaxKind::K_NOT),
            7,
            |rhs| ast::Expr::UnaryOp {
                op: ast::UnaryOp::Not,
                expr: Box::new(rhs),
            },
        ),

        // == Infix operators by precedence (binding power) ==
        // Precedence level 6: *, /, % (Left-associative)
        Operator::new(
            just(SyntaxKind::T_STAR),
            6,
            |lhs, rhs| ast::Expr::BinaryOp {
                op: ast::BinaryOp::Mul,
                lhs: Box::new(lhs),
                rhs: Box::new(rhs),
            },
        )
        .left(),
        Operator::new(
            just(SyntaxKind::T_SLASH),
            6,
            |lhs, rhs| ast::Expr::BinaryOp {
                op: ast::BinaryOp::Div,
                lhs: Box::new(lhs),
                rhs: Box::new(rhs),
            },
        )
        .left(),
        Operator::new(
            just(SyntaxKind::T_PERCENT),
            6,
            |lhs, rhs| ast::Expr::BinaryOp {
                op: ast::BinaryOp::Mod,
                lhs: Box::new(lhs),
                rhs: Box::new(rhs),
            },
        )
        .left(),

        // Precedence level 5: +, - (Left-associative)
        Operator::new(
            just(SyntaxKind::T_PLUS),
            5,
            |lhs, rhs| ast::Expr::BinaryOp {
                op: ast::BinaryOp::Add,
                lhs: Box::new(lhs),
                rhs: Box::new(rhs),
            },
        )
        .left(),
        Operator::new(
            just(SyntaxKind::T_MINUS),
            5,
            |lhs, rhs| ast::Expr::BinaryOp {
                op: ast::BinaryOp::Sub,
                lhs: Box::new(lhs),
                rhs: Box::new(rhs),
            },
        )
        .left(),

        // Precedence level 4: ==, !=, <, <=, >, >= (Comparison)
        Operator::new(
            just(SyntaxKind::T_EQEQ),
            4,
            |lhs, rhs| ast::Expr::BinaryOp {
                op: ast::BinaryOp::Eq,
                lhs: Box::new(lhs),
                rhs: Box::new(rhs),
            },
        )
        .left(),
        Operator::new(
            just(SyntaxKind::T_NEQ),
            4,
            |lhs, rhs| ast::Expr::BinaryOp {
                op: ast::BinaryOp::Neq,
                lhs: Box::new(lhs),
                rhs: Box::new(rhs),
            },
        )
        .left(),
        // … other comparison operators …

        // Precedence level 3: && (Logical AND)
        Operator::new(
            just(SyntaxKind::K_AND),
            3,
            |lhs, rhs| ast::Expr::BinaryOp {
                op: ast::BinaryOp::And,
                lhs: Box::new(lhs),
                rhs: Box::new(rhs),
            },
        )
        .left(),

        // Precedence level 2: || (Logical OR)
        Operator::new(
            just(SyntaxKind::K_OR),
            2,
            |lhs, rhs| ast::Expr::BinaryOp {
                op: ast::BinaryOp::Or,
                lhs: Box::new(lhs),
                rhs: Box::new(rhs),
            },
        )
        .left(),

        // Precedence level 1: = (Assignment, Right-associative)
        Operator::new(
            just(SyntaxKind::T_EQ),
            1,
            |lhs, rhs| ast::Expr::BinaryOp {
                op: ast::BinaryOp::Assign,
                lhs: Box::new(lhs),
                rhs: Box::new(rhs),
            },
        )
        .right(),

        // Function calls would be handled as a postfix operator or within the 'atom' parser
    ))
}
```

**Note**: The binding power values (1 through 7) are illustrative. They must be
carefully chosen to exactly match the precedence rules specified in the Haskell
updated syntax specification
(`docs/differential-datalog-parser-syntax-spec-updated.md`).

### Handling `if`/`else` expressions

Control-flow expressions rely on prefix parsing rather than the Pratt operator
table. The parser consumes the `if` keyword and delegates to
`parse_if_expression`, which parses the condition, `then` branch, and an
optional `else` branch using the existing `parse_expr` entry point. When the
`else` clause is omitted the parser produces the unit tuple `()`, represented
internally as `Expr::Tuple(vec![])`, matching the semantics of the reference
Haskell implementation. In the AST this omission appears as `Tuple([])`,
ensuring that downstream analyses always observe a concrete expression tree.

Chained `else if` clauses become nested `Expr::IfElse` nodes so that each
predicate retains its own branch:

```text
if a { x } else if b { y } else { z } ->
    IfElse(cond=a, then=x, else=IfElse(cond=b, then=y, else=z))
```

To guard against stack exhaustion the parser caps expression nesting at 256
levels. Inputs that exceed this limit raise an `"expression nesting too deep"`
diagnostic anchored at the token that would push the depth over the limit.

Error recovery follows the same pattern as other prefix forms: if a branch is
missing, the parser emits a targeted diagnostic at the offending token (or the
end of the input) while preserving the token stream for subsequent parsing. The
helper guards against the "dangling else" scenario by treating a bare `else`
token as evidence that the `then` branch was absent, producing a clear
`expected expression for 'then' branch of 'if'` message.

A subtle ambiguity arises from the shared `IDENT {` token sequence used by
struct literals. To resolve this, the implementation activates a struct-literal
guard for the duration of the condition parse. While active, it interprets
`IDENT {` as a variable followed by the branch, preventing the condition from
consuming the branch braces. The guard automatically suspends inside
parentheses, brace groups, and closure bodies so expressions such as
`if (Point { x: 1 }) { … }` or `if cond { Point { x: 1 } }` continue to parse
as intended. This strategy eliminates spurious `expected T_COLON` diagnostics
without restricting legitimate struct literal usage.

### Handling `match` expressions

`match` expressions reuse a dedicated prefix parser. The Rust implementation
mirrors the Haskell grammar, expecting `match (expr) { pattern -> expr, … }`.
The scrutinee must appear within parentheses; `parse_match_expression`
temporarily suspends the struct-literal guard, so constructs like
`match (Point { x: 1 }) { … }` remain valid.

Arms are parsed in `parse_match_arms`, which requires at least one arm and
accepts an optional trailing comma, matching `commaSepEnd1` from the reference
parser. Each arm delegates to `collect_match_pattern`, which slices the source
text between the arm start and the top-level `->`, balancing parentheses,
braces, and brackets. The captured substring is trimmed so nested patterns such
as `Point { field: Some(x) }` round-trip without losing formatting.

If the parser encounters a comma or closing brace before a top-level `->`, it
emits `expected '->' in match arm` and stops, preventing the following body
from being misinterpreted as part of the pattern. Empty bodies (for example,
`match (x) { }`) raise `expected at least one match arm`, with the diagnostic
anchored on the closing brace. Arm bodies execute under
`with_struct_literals_suspended`, so users can build struct literals inside
arms without triggering the guard.

### Handling `for` loop expressions

Rules may contain `for` loops with optional guards. The Pratt parser recognizes
the `for` keyword as another prefix construct that yields an `Expr::ForLoop`
node. The header is handled in three parts:

- **Pattern extraction:** tokens before `in` are sliced directly from the
  source, so destructuring patterns remain verbatim. The range is trimmed to
  remove surrounding whitespace, but the inner formatting is untouched.
- **Iterable expression:** parsed with struct literals temporarily re-enabled,
  so constructs like `for (row in Rows { … })` continue to work.
- **Guard:** if the header contains `if`, the guard expression reuses
  `parse_if_clause` which already implements precise diagnostics for missing or
  malformed expressions. Guards are stored as `Option<Box<Expr>>` and omitted
  when absent.

The loop body is parsed using the standard expression entry point. That parser
handles both single atoms and grouped blocks. Treating loops as expressions
means `rule_body_span` can continue validating rule bodies by invoking the
Pratt parser; control-flow errors surface alongside other expression
diagnostics.

______________________________________________________________________

## 4. Illustrative CST integration

The `chumsky` expression parser produces a structured `ast::Expr`. However, the
core of `ddlint` is the `rowan` CST, which must remain lossless. The expression
parser will not run in isolation; it will be a component of the larger
statement-parsing logic.

The strategy is as follows:

1. When the main parser (in `span_scanner.rs`) encounters a context where an
   expression is expected (e.g., in a rule body or a `return` statement), it
   will invoke `expression_parser()`.

2. The `expression_parser()` will consume tokens from the stream.

3. Crucially, the main parser will continue to feed every single token
   (including whitespace and comments) into the `rowan::GreenNodeBuilder`.

4. The main parser will wrap the sequence of tokens that were successfully
   parsed by `expression_parser()` in an `N_EXPR_NODE` `SyntaxKind`.

This approach delivers the best of both worlds:

- A structured `ast::Expr` for immediate semantic validation or interpretation.

- A full-fidelity `N_EXPR_NODE` in the CST, containing all original tokens,
  which can be used for linting rules that care about formatting, or for
  reliable autofixing.

### Public AST helpers

Integration and fixture code build Pratt parser expectations through the
`parser::ast` façade. The module now re-exports `MatchArm` alongside `Expr`, so
callers can construct match expressions without reaching into the private
layout of `ast::expr`. Tests should continue using the helpers in `test_util`
for common patterns, but direct use of `MatchArm` is available whenever bespoke
arm construction is clearer than chaining builders.

A typed AST wrapper describing this new node would look something like this:

```rust,no_run
// In src/parser/ast/mod.rs or a new file

pub struct Expression(rowan::SyntaxNode<DdlogLanguage>);

impl AstNode for Expression {
    fn syntax(&self) -> &rowan::SyntaxNode<DdlogLanguage> {
        &self.0
    }
}

impl Expression {
    /// This method would re-run the chumsky parser on the tokens
    /// within this CST node to produce the structured AST on-demand.
    /// This is a common and efficient pattern in CST-based architectures.
    pub fn to_structured_ast(&self) -> Result<ast::Expr, Vec<Simple<SyntaxKind>>> {
        let tokens = self.syntax().children_with_tokens().filter_map(|elem| {
            if let rowan::NodeOrToken::Token(token) = elem {
                // Reconstruct the (SyntaxKind, Span) stream at this point
                // This part needs careful implementation.
            }
            // …
        });

        // expression_parser().parse(tokens)
        todo!()
    }
}
```

This example provides a clear and robust reference for implementing a complete
expression parser that integrates with a `rowan`-based architecture. It should
be read as an educational implementation sketch, not a committed Weaver design
decision.

______________________________________________________________________

## 5. Implementation notes

These notes describe one illustrative codebase layout and are included as
worked examples. They are not normative requirements for Weaver components.

In the Weaver codebase, the parser implementation lives in
`crates/weaver-syntax/src/parser.rs`. It wraps `tree_sitter::Parser`, sets the
language grammar through `SupportedLanguage::tree_sitter_language()`, and
returns a `ParseResult` for parsed source text.

The workspace and crate manifests currently depend on Tree-sitter crates
(`tree-sitter`, `tree-sitter-rust`, `tree-sitter-python`, and
`tree-sitter-typescript`) and do not include a `chumsky` dependency. Unit tests
in `crates/weaver-syntax/src/parser.rs` invoke this Tree-sitter-based parser
via `Parser::new(...).parse(...)` to validate both successful parses and syntax
error detection.

Literal tokens are normalized via a dedicated helper, keeping prefix parsing
readable. The parser maps `T_NUMBER`, `T_STRING`, `K_TRUE`, and `K_FALSE` to
`ast::Literal` variants, ensuring numbers, strings, and booleans appear
directly in the resulting AST. String handling now classifies standard, raw,
and raw-interpolated forms (plus their interned variants) into a
`StringLiteral` that records the surface form and whether interpolation is
present. The same helper powers pattern collection, so interpolated strings in
`match` arms or `for` bindings are rejected in line with the updated syntax
specification.

Operator precedence is centralized in `src/parser/ast/precedence.rs`. Both the
Pratt parser and any future grammar extensions reference this table, ensuring
consistent binding power definitions across the codebase.

Type and control operators follow this precedence, from highest to lowest: `:`,
`as`, `=`, `=>`, `;`. Logical `and` and `or` outrank `=`, so `a and b = c`
parses as `(a and b) = c`. Ascription and cast bind more tightly than
assignment but looser than arithmetic operators.

Variable references are parsed by interpreting identifier tokens as
`Expr::Variable`. Postfix operators such as calls, field access, method
invocations, bit slices, and tuple indexing are handled in a loop at the
highest precedence. Function calls produce `Expr::Call { callee, args }`, while
method calls, field access, bit slices, and tuple indices map to dedicated AST
variants. This design allows chaining like `foo.bar(x).0` without extra
precedence rules.

Struct literals, tuple literals, and closures extend the prefix grammar. Struct
construction recognizes `Ident { field: expr, … }` and records field order in
the AST. Tuple literals are distinguished from grouped expressions by the
presence of a comma or an empty pair of parentheses. Both structs and tuples
accept trailing commas. Closure literals parse a pipe-delimited parameter list
(allowing a trailing comma) followed by the body expression. Each feature is
implemented with small helper routines to keep the main parser readable.

Imperative control-flow tokens (`break`, `continue`, and `return`) now slot
into the same prefix dispatch. `break` and `continue` build dedicated AST
markers, so later passes can detect loop termination without re-reading the
source text. The `return` parser accepts an optional expression: when the next
token is a terminator such as `)`, `}`, `,`, `;`, or `->`, it emits a unit
tuple to mirror the Haskell parser's default. Otherwise, it parses a full
expression, while respecting the struct-literal suppression guard, surfacing a
targeted diagnostic if the expression is missing. This keeps imperative
statements usable inside expression contexts and aligns the behaviour with
upstream DDlog semantics.

### 5.4 Rule body integration

Rule bodies now reuse the Pratt parser end-to-end. The span scanner slices the
token stream between `:-` and the terminating `.` into comma-separated literal
spans using delimiter-aware depth tracking. Each span becomes its own
`N_EXPR_NODE`, so the CST stores one expression node per clause instead of a
single blob.

Every literal span is validated by `parse_expression`, which surfaces syntax
errors from control-flow constructs (e.g., `if` or `for`) alongside standard
expression diagnostics. When a literal uses an assignment form
(`Pattern = Expr`), the span validator also parses and validates the left-hand
pattern, ensuring FlatMap-style binds fail early with precise spans.

The `Rule` AST wrapper exposes the resulting nodes via
`body_expression_nodes()` and a convenience `body_expressions()` helper that
re-parses the stored text into structured `Expr` values. Assignment literals
are surfaced via `body_terms()` as `RuleBodyTerm::Assignment`, which stores a
dedicated `Pattern` AST node rather than raw pattern text. Downstream analyses
can therefore reason about rule bodies without rebuilding bespoke parsers or
retokenising the source.

### 5.4.1 Atom adornments (`'` and `-<N>`)

The updated DDlog syntax allows rule atoms to carry two additional markers that
are not ordinary infix operators:

- **Diff:** `Rel'(args)` marks a difference atom. The Pratt parser represents
  this as `Expr::AtomDiff { expr }` wrapped around the underlying atom
  expression.
- **Delay:** `Atom -<N>` applies an unsigned delay to an atom. The Pratt parser
  represents this as `Expr::AtomDelay { delay: u32, expr }` and reports an
  error when `N` does not fit in `u32`.

Head locations (`@ expr`) are parsed by the rule head layer (`Rule::heads()`)
rather than by the Pratt expression parser because `@` is only valid in rule
heads.

### 5.5 Operator table completion

The precedence table now includes all operators from the updated syntax
specification (`docs/differential-datalog-parser-syntax-spec-updated.md`
section 4). The following operators are now included:

**Binary operators:**

| Operator | `BinaryOp` variant | Binding power        | Associativity |
| -------- | ------------------ | -------------------- | ------------- |
| `++`     | `Concat`           | 60 (same as `+`/`-`) | left          |
| `<<`     | `Shl`              | 55                   | left          |
| `>>`     | `Shr`              | 55                   | left          |
| `&`      | `BitAnd`           | 45                   | left          |
| `^`      | `BitXor`           | 40                   | left          |
| &#124;   | `BitOr`            | 35                   | left          |
| `==`     | `Eq`               | 30                   | left          |
| `!=`     | `Neq`              | 30                   | left          |
| `<`      | `Lt`               | 30                   | left          |
| `<=`     | `Lte`              | 30                   | left          |
| `>`      | `Gt`               | 30                   | left          |
| `>=`     | `Gte`              | 30                   | left          |

**Unary (prefix) operators:**

| Operator | `UnaryOp` variant | Binding power |
| -------- | ----------------- | ------------- |
| `~`      | `BitNot`          | 80            |
| `&`      | `Ref`             | 80            |

These binding powers mirror the constants in `src/parser/ast/precedence.rs`.
When adjusting operator precedence in code, update this table in the same
change so the documentation and implementation stay aligned.

The `T_CARET` token was added to the tokenizer to recognize the `^` operator.
The `&` token serves dual purposes: as a prefix operator, it represents
reference/address-of (`UnaryOp::Ref`), and as an infix operator, it represents
bitwise AND (`BinaryOp::BitAnd`). The Pratt parser's separate prefix and infix
tables naturally handle this overloading.

The `=>` operator was already present in the table with right-associativity at
binding power 5, correctly positioned between logical operators and assignment.
