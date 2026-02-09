Feature: Plugin execution via the runner

  Scenario: Successful actuator plugin execution
    Given a registry with an actuator plugin "rope" for "python"
    And a mock executor that returns a diff
    When plugin "rope" is executed with operation "rename"
    Then the response is successful
    And the response output is a diff

  Scenario: Plugin not found in registry
    Given a registry with an actuator plugin "rope" for "python"
    And a mock executor that returns a diff
    When plugin "nonexistent" is executed with operation "rename"
    Then the execution fails with "not_found"

  Scenario: Executor returns non-zero exit error
    Given a registry with an actuator plugin "rope" for "python"
    And a mock executor that returns a non-zero exit error
    When plugin "rope" is executed with operation "rename"
    Then the execution fails with "non_zero_exit"

  Scenario: Plugin produces empty output
    Given a registry with an actuator plugin "rope" for "python"
    And a mock executor that returns empty output
    When plugin "rope" is executed with operation "rename"
    Then the response is successful
    And the response output is empty

  Scenario: Registry lookup by language
    Given a registry with an actuator plugin "rope" for "python"
    And a registry with a sensor plugin "jedi" for "python"
    When actuator plugins for "python" are queried
    Then 1 plugin is returned
    And the returned plugin is named "rope"
