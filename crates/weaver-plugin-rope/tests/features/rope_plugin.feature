Feature: Rope actuator plugin

  Scenario: Rename-symbol succeeds with diff output
    Given a rename-symbol request with required arguments
    When the plugin executes the request
    Then the plugin returns successful diff output

  Scenario: Rename-symbol fails when position is missing
    Given a rename-symbol request missing position
    When the plugin executes the request
    Then the plugin returns failure diagnostics
    And the failure message contains "position"

  Scenario: Unsupported operation fails with diagnostics
    Given an unsupported extract method request
    When the plugin executes the request
    Then the plugin returns failure diagnostics
    And the failure message contains "unsupported"

  Scenario: Adapter failures are surfaced with reason code
    Given a rename-symbol request with required arguments
    And a rope adapter that fails
    When the plugin executes the request
    Then the plugin returns failure diagnostics
    And the failure message contains "rope engine failed"
    And the failure has reason code "symbol_not_found"

  Scenario: Unchanged output is treated as failure
    Given a rename-symbol request with required arguments
    And a rope adapter that returns unchanged content
    When the plugin executes the request
    Then the plugin returns failure diagnostics
    And the failure message contains "no content changes"
