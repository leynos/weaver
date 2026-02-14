Feature: Act refactor

  Scenario: Refactor applies a valid plugin diff
    Given a workspace file for refactoring
    And a valid act refactor request
    When the act refactor command executes
    Then the refactor command succeeds
    And the target file is updated

  Scenario: Refactor reports plugin runtime failures
    Given a workspace file for refactoring
    And a valid act refactor request
    And a runtime error from the refactor plugin
    When the act refactor command executes
    Then the refactor command fails with status 1
    And the target file is unchanged
    And the stderr stream contains "act refactor failed"

  Scenario: Refactor rejects malformed plugin diffs
    Given a workspace file for refactoring
    And a valid act refactor request
    And a malformed diff response from the refactor plugin
    When the act refactor command executes
    Then the refactor command fails with status 1
    And the target file is unchanged

  Scenario: Refactor validates required arguments
    Given a workspace file for refactoring
    And a refactor request missing provider
    When the act refactor command executes
    Then the refactor command is rejected as invalid arguments
    And the target file is unchanged
