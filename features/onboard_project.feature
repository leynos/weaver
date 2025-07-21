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
