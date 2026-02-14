Feature: Rope actuator plugin

  Scenario: Rename succeeds with diff output
    Given a rename request with required arguments
    When the plugin executes the request
    Then the plugin returns successful diff output

  Scenario: Rename fails when offset is missing
    Given a rename request missing offset
    When the plugin executes the request
    Then the plugin returns failure diagnostics
    And the failure message contains "offset"

  Scenario: Unsupported operation fails with diagnostics
    Given an unsupported extract method request
    When the plugin executes the request
    Then the plugin returns failure diagnostics
    And the failure message contains "unsupported"

  Scenario: Adapter failures are surfaced
    Given a rename request with required arguments
    And a rope adapter that fails
    When the plugin executes the request
    Then the plugin returns failure diagnostics
    And the failure message contains "rope engine failed"

  Scenario: Unchanged output is treated as failure
    Given a rename request with required arguments
    And a rope adapter that returns unchanged content
    When the plugin executes the request
    Then the plugin returns failure diagnostics
    And the failure message contains "no content changes"
