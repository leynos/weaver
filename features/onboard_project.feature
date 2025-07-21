Feature: onboard project command
  Scenario: onboarding via the daemon
    Given a temporary runtime dir
    When I invoke the onboard-project command
    Then the output includes onboarding details
