Feature: Act refactor

  Scenario: Refactor applies a valid plugin diff after automatic routing to rope
    Given a workspace file for refactoring
    And a valid auto-routed act refactor request resolved to rope
    When the act refactor command executes
    Then the refactor command succeeds
    And the target file is updated
    And the stderr stream contains "\"type\":\"CapabilityResolution\""
    And the stderr stream contains "\"selected_provider\":\"rope\""

  Scenario: Refactor applies a valid plugin diff after automatic routing to rust-analyzer
    Given a workspace file for refactoring
    And a valid auto-routed act refactor request resolved to rust-analyzer
    When the act refactor command executes
    Then the refactor command succeeds
    And the target file is updated
    And the stderr stream contains "\"selected_provider\":\"rust-analyzer\""

  Scenario: Refactor reports plugin runtime failures
    Given a workspace file for refactoring
    And a valid auto-routed act refactor request resolved to rope
    And a runtime error from the refactor plugin
    When the act refactor command executes
    Then the refactor command fails with status 1
    And the target file is unchanged
    And the stderr stream contains "act refactor failed"

  Scenario: Refactor rejects malformed plugin diffs
    Given a workspace file for refactoring
    And a valid auto-routed act refactor request resolved to rope
    And a malformed diff response from the refactor plugin
    When the act refactor command executes
    Then the refactor command fails with status 1
    And the target file is unchanged

  Scenario: Refactor refuses unsupported languages deterministically
    Given a workspace file for refactoring
    And an unsupported-language act refactor request
    When the act refactor command executes
    Then the refactor command fails with status 1
    And the target file is unchanged
    And the stderr stream contains "\"refusal_reason\":\"unsupported_language\""

  Scenario: Refactor refuses explicit provider mismatches deterministically
    Given a workspace file for refactoring
    And a Python act refactor request with an incompatible provider override
    When the act refactor command executes
    Then the refactor command fails with status 1
    And the target file is unchanged
    And the stderr stream contains "\"refusal_reason\":\"explicit_provider_mismatch\""
