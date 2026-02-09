Feature: Apply patch

  Scenario: Apply patch modifies a file
    Given a workspace with the default source file
    And a patch that replaces the main message
    And an apply-patch syntactic lock that passes
    And an apply-patch semantic lock that passes
    When the patch is applied
    Then the apply-patch file "src/main.rs" contains "New Message"
    And the apply-patch response succeeds

  Scenario: Apply patch creates a file
    Given an empty workspace
    And a patch that creates a new module
    And an apply-patch syntactic lock that passes
    And an apply-patch semantic lock that passes
    When the patch is applied
    Then the apply-patch file "src/new.rs" contains "fn hello() {}"
    And the apply-patch response succeeds

  Scenario: Apply patch deletes a file
    Given a workspace with a deletable file
    And a patch that deletes a file
    And an apply-patch syntactic lock that passes
    And an apply-patch semantic lock that passes
    When the patch is applied
    Then the file "src/remove.rs" is missing
    And the apply-patch response succeeds

  Scenario: Reject path traversal
    Given a patch that targets a parent directory
    And an apply-patch syntactic lock that passes
    And an apply-patch semantic lock that passes
    When the patch is applied
    Then the apply-patch fails with "InvalidPath"

  Scenario: Reject invalid diff headers
    Given a patch with an invalid diff header
    And an apply-patch syntactic lock that passes
    And an apply-patch semantic lock that passes
    When the patch is applied
    Then the apply-patch fails with "InvalidDiffHeader"

  Scenario: Reject missing create hunks
    Given a patch that omits the create hunk
    And an apply-patch syntactic lock that passes
    And an apply-patch semantic lock that passes
    When the patch is applied
    Then the apply-patch fails with "MissingHunk"

  Scenario: Reject syntactic lock failure
    Given a workspace with the default source file
    And a patch that replaces the main message
    And an apply-patch syntactic lock on "src/main.rs" that fails with "syntax error"
    And an apply-patch semantic lock that passes
    When the patch is applied
    Then the apply-patch file "src/main.rs" contains "Old Message"
    And the apply-patch fails with "SyntacticLock"

  Scenario: Reject semantic lock failure
    Given a workspace with the default source file
    And a patch that replaces the main message
    And an apply-patch syntactic lock that passes
    And an apply-patch semantic lock on "src/main.rs" that fails with "type error"
    When the patch is applied
    Then the apply-patch file "src/main.rs" contains "Old Message"
    And the apply-patch fails with "SemanticLock"
