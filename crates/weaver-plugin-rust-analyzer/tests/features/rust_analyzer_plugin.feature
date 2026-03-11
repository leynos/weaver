Feature: rust-analyzer actuator plugin

  Scenario: rename-symbol succeeds with diff output
    Given a rename-symbol request with required arguments
    When the plugin executes the request
    Then the plugin returns successful diff output

  Scenario: rename-symbol fails when position is missing
    Given a rename-symbol request missing position
    When the plugin executes the request
    Then the plugin returns failure diagnostics
    And the failure message contains "position"
    And the failure reason code is "incomplete_payload"

  Scenario: rename-symbol fails when uri is missing
    Given a rename-symbol request missing uri
    When the plugin executes the request
    Then the plugin returns failure diagnostics
    And the failure message contains "uri"
    And the failure reason code is "incomplete_payload"

  Scenario: Unsupported operation fails with diagnostics
    Given an unsupported extract method request
    When the plugin executes the request
    Then the plugin returns failure diagnostics
    And the failure message contains "unsupported"
    And the failure reason code is "operation_not_supported"

  Scenario: Adapter failures are surfaced
    Given a rename-symbol request with required arguments
    And a rust analyzer adapter that fails
    When the plugin executes the request
    Then the plugin returns failure diagnostics
    And the failure message contains "rust-analyzer adapter failed"

  Scenario: Unchanged output is treated as failure
    Given a rename-symbol request with required arguments
    And a rust analyzer adapter that returns unchanged content
    When the plugin executes the request
    Then the plugin returns failure diagnostics
    And the failure message contains "no content changes"
    And the failure reason code is "symbol_not_found"
