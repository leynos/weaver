# Architectural decision record (ADR) 002: Query language for feature extraction in Weaver

## Status

Proposed.

## Date

2026-02-28.

## Context and problem statement

Weaver needs a query language for extracting syntactic features from source
code. Typical use cases include locating decorated functions, enumerating call
sites, and capturing identifiers with their surrounding structure.

Weaver already uses Tree-sitter in its syntax layer. For maintainability, the
chosen query model should either:

- execute directly over Tree-sitter syntax trees, or
- compile predictably into Tree-sitter-backed matching operations.

The primary target languages are Rust, Python, Go, and TypeScript. Query
support must be practical across these languages, including common constructs
such as Rust attributes and Python decorators.

The system also needs to support both short, one-off queries and longer,
documented query files that can be reviewed and version-controlled.

## Decision drivers

- Multi-language coverage for Rust, Python, Go, and TypeScript.
- Predictable integration with existing Tree-sitter infrastructure.
- Query authoring ergonomics for both human authors and LLM-assisted authors.
- Expressiveness for captures, composition, and contextual constraints.
- Operational simplicity for an in-process Rust implementation.

## Requirements

### Functional requirements

- Match structural code patterns across all mandatory target languages.
- Capture substructures (for example names, literals, and subexpressions).
- Support composition patterns such as conjunction, disjunction, and negation.
- Support contextual constraints such as inside/not-inside style matching.

### Technical requirements

- Integrate with Weaver's Tree-sitter pipeline.
- Keep runtime overhead low enough for CLI and daemon usage.
- Provide deterministic behaviour suitable for tests and automation.
- Allow an escape hatch for direct Tree-sitter query use when required.

## Options considered

### Option A: Tree-sitter query language as primary surface

Tree-sitter queries provide direct AST pattern matching with captures,
quantifiers, and predicates. This option offers maximal implementation
alignment with current infrastructure.

Main trade-off: authoring ergonomics are weaker because authors must reason in
terms of grammar node kinds and field names.

### Option B: Semgrep-style syntax with Weaver-native execution

Semgrep-style syntax is code-shaped and easier to author for many users. Weaver
can implement a constrained Semgrep-like surface and execute it against
Tree-sitter-backed structures.

Main trade-off: semantics must be defined clearly to avoid drift from user
expectations associated with Semgrep.

### Option C: ast-grep syntax and semantics as primary surface

ast-grep offers a mature Rust implementation and code-like matching model.

Main trade-off: adopting ast-grep syntax as the primary surface would diverge
from the Semgrep-compatible direction already scoped in this repository.

### Option D: CodeQL-style relational query model

CodeQL-style querying is highly expressive for semantic and relational analysis.

Main trade-off: operational complexity and indexing/database workflow are much
heavier than required for Weaver's near-term feature extraction goals.

## Option comparison

| Option | Ergonomics | Integration fit | Semantic scope | Operational cost |
| ------ | ---------- | --------------- | -------------- | ---------------- |
| A      | Low        | High            | Medium         | Low              |
| B      | High       | High            | Medium         | Medium           |
| C      | High       | Medium          | Medium         | Low              |
| D      | Medium     | Low             | High           | High             |

_Table 1: High-level trade-offs across candidate query approaches._

## Decision outcome / proposed direction

Adopt a Semgrep-style query surface as Weaver's primary user-facing language,
implemented natively against Weaver's Tree-sitter-backed matching pipeline.

Retain direct Tree-sitter queries as an explicit advanced escape hatch for
cases that need grammar-level precision beyond the Semgrep-style subset.

## Goals and non-goals

### Goals

- Provide an approachable, code-like query syntax for extraction tasks.
- Keep execution deterministic and integrable in `weaverd` workflows.
- Support a practical subset that covers the most common extraction scenarios.

### Non-goals

- Full semantic program analysis equivalent to CodeQL.
- Full compatibility with every historical Semgrep feature on day one.
- A new standalone SQL-like AST query engine.

## Migration plan

1. Define the Semgrep-style subset grammar and AST in `weaver-syntax` docs.
2. Implement parsing and normalization to Weaver internal query operators.
3. Add execution over Tree-sitter-backed structures with capture support.
4. Add conformance tests for mandatory language constructs.
5. Provide explicit diagnostics for unsupported features with escape-hatch
   guidance to direct Tree-sitter queries.

## Known risks and limitations

- Semantic drift risk if behaviour diverges from Semgrep user expectations.
- Coverage risk when upstream grammars lag language evolution.
- Complexity risk if too many advanced features are added prematurely.

## Outstanding decisions

- Which advanced predicates are included in the first public subset?
- Will deep-matching semantics be included in the first release?
- How should capability flags per language be represented in user-facing
  diagnostics?

## Architectural rationale

This direction aligns with Weaver's architecture by combining:

- a user-facing syntax optimized for practical authoring, and
- an execution substrate that fits existing Tree-sitter infrastructure.

The escape-hatch model keeps the implementation scoped while preserving
expressive power for advanced cases.
