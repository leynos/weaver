Feature: Weaver CLI behaviour

  Scenario: Streaming a request to the daemon
    Given a running fake daemon
    When the operator runs "observe get-definition --symbol main"
    Then the daemon receives "request_observe_get_definition.jsonl"
    And stdout is "daemon says hello"
    And stderr is "daemon complains"
    And the CLI exits with code 17

  Scenario: Probing capability output
    Given capability overrides force python rename
    When the operator runs "--capabilities"
    Then capabilities output is "capabilities_force_python.json"
    And stderr is ""
    And the CLI exits with code 0

  Scenario: Rejecting a missing operation
    When the operator runs "observe"
    Then the CLI fails
    And stderr contains "command operation must be provided"

  Scenario: Reporting malformed daemon responses
    Given a running fake daemon sending malformed json
    When the operator runs "observe get-definition --symbol main"
    Then the CLI fails
    And stderr contains "failed to parse daemon message"

  Scenario: Detecting a missing exit status
    Given a running fake daemon that closes without exit
    When the operator runs "observe get-definition --symbol main"
    Then the CLI fails
    And stderr contains "daemon closed the stream without sending an exit status"

  Scenario: Aborting after repeated empty responses
    Given a running fake daemon that emits empty lines
    When the operator runs "observe get-definition --symbol main"
    Then the CLI fails
    And stderr contains "Warning: received"
