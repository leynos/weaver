Feature: get definition command
  Scenario: daemon auto-start
    Given a temporary runtime dir
    When I invoke the get-definition command
    Then the output includes a symbol line

  Scenario: no symbol at position
    Given a temporary runtime dir with no symbols
    When I invoke the get-definition command
    Then no output is produced

  Scenario: missing serena-agent dependency
    Given a temporary runtime dir
    And serena-agent is missing
    When I invoke the get-definition command
    Then the daemon is not ready

  Scenario: file not found
    Given a temporary runtime dir
    When I invoke the get-definition command with a missing file
    Then the command fails with a missing file error
