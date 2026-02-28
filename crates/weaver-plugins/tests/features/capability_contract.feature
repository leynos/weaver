Feature: Capability contract validation

  Scenario: Valid rename request passes validation
    Given a rename-symbol contract
    And a plugin request with operation "rename" and arguments uri="file:///src/main.py" position="10:5" new_name="foo"
    When the request is validated
    Then validation succeeds

  Scenario: Missing required field rejects request
    Given a rename-symbol contract
    And a plugin request with operation "rename" and arguments uri="file:///src/main.py" position="10:5"
    When the request is validated
    Then validation fails with "new_name"

  Scenario: Empty new_name rejects request
    Given a rename-symbol contract
    And a plugin request with operation "rename" and arguments uri="file:///src/main.py" position="10:5" new_name=""
    When the request is validated
    Then validation fails with "new_name"

  Scenario: Successful diff response passes validation
    Given a rename-symbol contract
    And a successful diff response
    When the response is validated
    Then validation succeeds

  Scenario: Successful non-diff response fails validation
    Given a rename-symbol contract
    And a successful analysis response
    When the response is validated
    Then validation fails with "Diff"

  Scenario: Failed response with reason code passes validation
    Given a rename-symbol contract
    And a failure response with reason code "symbol_not_found"
    When the response is validated
    Then validation succeeds

  Scenario: Actuator manifest with capabilities validates
    Given an actuator manifest with capability "rename-symbol"
    When the manifest is validated
    Then validation succeeds

  Scenario: Sensor manifest with capabilities is rejected
    Given a sensor manifest with capability "rename-symbol"
    When the manifest is validated
    Then validation fails with "sensor"

  Scenario: Contract version compatibility with same major
    Given contract version 1.0
    And a peer version 1.3
    Then the versions are compatible

  Scenario: Contract version incompatibility with different major
    Given contract version 1.0
    And a peer version 2.0
    Then the versions are incompatible
