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
