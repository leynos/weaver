Feature: Process-based language server adapter

  Scenario: Missing binary produces clear error
    Given a process adapter for rust with a nonexistent binary
    When the adapter is initialized
    Then the error indicates binary not found
    And the error message contains the command path

  Scenario: Adapter uses default configuration for each language
    Given a default rust adapter
    Then the adapter command is rust-analyzer
    And a default python adapter
    Then the python adapter command is pyrefly
    And a default typescript adapter
    Then the typescript adapter command is tsgo

  Scenario: Adapter accepts custom configuration
    Given a rust adapter with custom command my-rust-analyzer
    Then the adapter command is my-rust-analyzer
