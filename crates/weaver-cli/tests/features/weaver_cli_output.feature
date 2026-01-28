Feature: Weaver CLI human-readable output

  Scenario: Rendering definition output with context
    Given a source file named "example.rs"
    And a running fake daemon emitting definition output
    When the operator runs the definition command
    Then stdout contains "example.rs"
    And stdout contains "^ definition"
    And the CLI exits with code 0

  Scenario: Rendering diagnostics output with context
    Given a source file named "example.rs"
    And a running fake daemon emitting diagnostics output
    When the operator runs the diagnostics command
    Then stdout contains "example.rs"
    And stdout contains "^ boom"
    And the CLI exits with code 0

  Scenario: Falling back when source content is missing
    Given a missing source file named "missing.rs"
    And a running fake daemon emitting diagnostics output
    When the operator runs the diagnostics command
    Then stdout contains "source unavailable"
    And the CLI exits with code 0

  Scenario: JSON output passes through raw payloads
    Given a source file named "example.rs"
    And a running fake daemon emitting definition output
    When the operator runs the json definition command
    Then stdout contains "\"uri\""
    And stdout contains "\"line\":2"
    And stdout does not contain "^ definition"
    And the CLI exits with code 0
