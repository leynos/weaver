Feature: list diagnostics command
  Scenario: daemon auto-start
    Given a temporary runtime dir
    When I invoke the list-diagnostics command
    Then the output includes a diagnostic line

  Scenario: daemon already running
    Given a temporary runtime dir
    And the daemon is already running
    When I invoke the list-diagnostics command
    Then the output includes a diagnostic line

  Scenario: missing serena-agent dependency
    Given a temporary runtime dir
    And serena-agent is missing
    When I invoke the list-diagnostics command
    Then the daemon is not ready

  Scenario: unknown tool attribute
    Given a temporary runtime dir
    And the tool attribute is unknown
    When I invoke the list-diagnostics command
    Then the tool attribute is reported missing

  Scenario: malformed output
    Given a temporary runtime dir
    And the server returns malformed output
    When I invoke the list-diagnostics command
    Then the output is malformed

  Scenario: filter by severity
    Given a temporary runtime dir
    When I invoke the list-diagnostics command with severity "Error"
    Then the output includes a diagnostic line

  Scenario: filter by file
    Given a temporary runtime dir
    When I invoke the list-diagnostics command for file "foo.py"
    Then the output includes a diagnostic line
