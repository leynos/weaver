Feature: Sandbox process isolation

  Scenario: Allowed file is readable
    Given a sandbox world with fixture files
    And the command cats the allowed file
    And the sandbox allows the command and fixture file
    When the sandbox launches the command
    Then the sandboxed process succeeds
    And stdout contains "allowed file content"

  Scenario: Disallowed file access is blocked
    Given a sandbox world with fixture files
    And the command cats the forbidden file
    When the sandbox launches the command
    Then the sandboxed process fails

  Scenario: Environment inheritance is restricted by default
    Given a sandbox world with fixture files
    And environment variables KEEP_ME and DROP_ME are set
    And the sandbox allows only KEEP_ME to be inherited
    When the sandbox launches the command
    Then the sandboxed process succeeds
    And stdout contains "KEEP_ME=present"
    And stdout does not contain "DROP_ME"
    And environment markers are cleaned up

  Scenario: Environment variables are isolated by default
    Given a sandbox world with fixture files
    And environment variables KEEP_ME and DROP_ME are set
    And the sandbox uses the default environment isolation
    When the sandbox launches the command
    Then the sandboxed process succeeds
    And stdout does not contain "KEEP_ME"
    And stdout does not contain "DROP_ME"
    And environment markers are cleaned up

  Scenario: Environment variables are fully inherited when enabled
    Given a sandbox world with fixture files
    And environment variables KEEP_ME and DROP_ME are set
    And the sandbox inherits the full environment
    When the sandbox launches the command
    Then the sandboxed process succeeds
    And stdout contains "KEEP_ME=present"
    And stdout contains "DROP_ME=remove-me"
    And environment markers are cleaned up
