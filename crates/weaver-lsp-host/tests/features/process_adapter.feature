Feature: Process-based language server adapter

  Scenario: Missing binary produces clear error
    Given a process adapter for rust with a nonexistent binary
    When the adapter is initialized
    Then the error indicates binary not found
    And the error message contains the command path

  Scenario Outline: Adapter uses default configuration for each language
    Given a default <language> adapter
    Then the <language> adapter command is <command>

    Examples:
      | language   | command        |
      | rust       | rust-analyzer  |
      | python     | pyrefly        |
      | typescript | tsgo           |

  Scenario: Adapter accepts custom configuration
    Given a rust adapter with custom command my-rust-analyzer
    Then the adapter command is my-rust-analyzer
