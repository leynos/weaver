Feature: Act refactor

  Scenario: Refactor applies a valid plugin diff with an explicit rope provider
    Given a workspace file for refactoring
    And a valid act refactor request for rope
    When the act refactor command executes
    Then the refactor command succeeds
    And the target file is updated
    And the stderr stream contains "\"type\":\"CapabilityResolution\""
    And the stderr stream contains "\"selected_provider\":\"rope\""

  Scenario: Refactor applies a valid plugin diff with an explicit rust-analyzer provider
    Given a workspace file for refactoring
    And a valid act refactor request for rust-analyzer
    When the act refactor command executes
    Then the refactor command succeeds
    And the target file is updated
    And the stderr stream contains "\"selected_provider\":\"rust-analyzer\""

  Scenario: Refactor reports plugin runtime failures
    Given a workspace file for refactoring
    And a valid act refactor request for rope
    And a runtime error from the refactor plugin
    When the act refactor command executes
    Then the refactor command fails with status 1
    And the target file is unchanged
    And the stderr stream contains "act refactor failed"

  Scenario: Refactor rejects malformed plugin diffs
    Given a workspace file for refactoring
    And a valid act refactor request for rope
    And a malformed diff response from the refactor plugin
    When the act refactor command executes
    Then the refactor command fails with status 1
    And the target file is unchanged

  Scenario: Refactor rejects successful plugin responses without diffs
    Given a workspace file for refactoring
    And a valid auto-routed act refactor request resolved to rope
    And a non-diff success response from the refactor plugin
    When the act refactor command executes
    Then the refactor command fails with status 1
    And the target file is unchanged
    And the stderr stream contains "did not return diff output"

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

  Scenario: Refactor reports the complete required argument contract
    Given an act refactor request without the required flags
    When the act refactor command executes
    Then the refactor command fails with status 1
    And the dispatch error contains "--provider <plugin>"
    And the dispatch error contains "--refactoring <operation>"
    And the dispatch error contains "--file <path>"
    And the dispatch error contains "Providers: rope, rust-analyzer"
    And the dispatch error contains "Refactorings: rename"
