Feature: Daemon bootstrap

  Scenario: Successful bootstrap defers backend start
    Given a healthy configuration loader
    When the daemon bootstrap runs
    Then bootstrap succeeds
    And the reporter recorded bootstrap start
    And the reporter recorded bootstrap success
    And no backend was started eagerly

  Scenario: Backend failure surfaces structured events
    Given a healthy configuration loader
    And a backend provider that fails for semantic
    When the daemon bootstrap runs
    And the semantic backend is requested
    Then bootstrap succeeds
    And starting the backend fails
    And the reporter recorded backend failure for semantic

  Scenario: Configuration failures are reported
    Given a failing configuration loader
    When the daemon bootstrap runs
    Then bootstrap fails
    And the reporter recorded bootstrap failure
