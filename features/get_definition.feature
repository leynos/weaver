Feature: get definition command
  Scenario: daemon auto-start
    Given a temporary runtime dir
    When I invoke the get-definition command
    Then the output includes a symbol line

  Scenario: missing serena-agent dependency
    Given a temporary runtime dir
    And serena-agent is missing
    When I invoke the get-definition command
    Then the daemon is not ready
