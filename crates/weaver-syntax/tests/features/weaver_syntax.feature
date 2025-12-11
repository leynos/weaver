Feature: Syntactic analysis and pattern matching

  The weaver-syntax crate provides Tree-sitter powered syntactic analysis
  including validation, pattern matching, and code rewriting.

  # Syntactic Validation Scenarios

  Scenario: Valid Rust code passes syntactic validation
    Given a file "main.rs" with content "fn main() {}"
    When the syntactic lock validates the file
    Then validation passes with no failures

  Scenario: Invalid Rust code fails with error location
    Given a file "broken.rs" with content "fn broken() {"
    When the syntactic lock validates the file
    Then validation fails
    And the failure includes line number 1

  Scenario: Valid Python code passes syntactic validation
    Given a file "script.py" with content "def hello(): pass"
    When the syntactic lock validates the file
    Then validation passes with no failures

  Scenario: Invalid Python code fails with error location
    Given a file "broken.py" with content "def broken("
    When the syntactic lock validates the file
    Then validation fails

  Scenario: Unknown file extensions are skipped
    Given a file "data.json" with content "{invalid json"
    When the syntactic lock validates the file
    Then validation passes with no failures

  Scenario: Multiple files validated together
    Given a file "valid.rs" with content "fn valid() {}"
    And a file "invalid.rs" with content "fn invalid() {"
    When the syntactic lock validates all files
    Then validation fails
    And only "invalid.rs" has failures

  # Pattern Matching Scenarios

  Scenario: Pattern matches function definitions
    Given Rust source code "fn hello() {} fn world() {}"
    And a pattern "fn $NAME() {}"
    When the pattern is matched against the source
    Then at least 1 match is found

  Scenario: Pattern captures metavariable values
    Given Rust source code "fn greet() {}"
    And a pattern "fn $FUNC() {}"
    When the pattern is matched against the source
    Then the capture "FUNC" contains "greet"

  Scenario: Pattern with no matches returns empty
    Given Rust source code "fn main() {}"
    And a pattern "struct $NAME {}"
    When the pattern is matched against the source
    Then no matches are found

  # Rewrite Scenarios

  Scenario: Rewrite transforms matching code
    Given Rust source code "fn main() { let x = 1; }"
    And a rewrite rule from "let $VAR = $VAL" to "const $VAR: _ = $VAL"
    When the rewrite is applied
    Then the output contains "const"
    And the rewrite made changes

  Scenario: Rewrite with no matches leaves code unchanged
    Given Rust source code "fn main() {}"
    And a rewrite rule from "struct $NAME {}" to "enum $NAME {}"
    When the rewrite is applied
    Then the rewrite made no changes
