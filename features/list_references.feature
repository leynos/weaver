Feature: list references command
  Scenario: daemon auto-start
    Given a temporary runtime dir
    When I invoke the list-references command
    Then the output includes a reference line

  Scenario: no references at position
    Given a temporary runtime dir with no references
    When I invoke the list-references command
    Then no output is produced

  Scenario: include definition
    Given a temporary runtime dir
    When I invoke the list-references command with include-definition
    Then the output includes a definition reference

  Scenario: missing serena-agent dependency
    Given a temporary runtime dir
    And serena-agent is missing
    When I invoke the list-references command
    Then the daemon is not ready

  Scenario: file not found
    Given a temporary runtime dir
    When I invoke the list-references command with a missing file
    Then the command fails with a missing file error
