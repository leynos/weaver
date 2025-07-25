Feature: onboard project command
  Scenario: onboarding via the daemon
    Given a temporary runtime dir
    When I invoke the onboard-project command
  Then the output includes onboarding details

  Scenario: onboarding with invalid project
    Given a temporary runtime dir
    And an invalid project structure
    When I invoke the onboard-project command
    Then the command fails with an error message

  Scenario: onboarding tool failure
    Given a temporary runtime dir
    And the onboarding tool raises an error
    When I invoke the onboard-project command
    Then an error report is produced

  Scenario: missing serena-agent dependency
    Given a temporary runtime dir
    And serena-agent is missing
    When I invoke the onboard-project command
    Then the command fails with a missing dependency message

  Scenario: server unavailable
    Given a temporary runtime dir
    And the server is unavailable
    When I invoke the onboard-project command
    Then the output indicates the server is unavailable

  Scenario: malformed output
    Given a temporary runtime dir
    And the server returns malformed output
    When I invoke the onboard-project command
    Then the output is malformed
