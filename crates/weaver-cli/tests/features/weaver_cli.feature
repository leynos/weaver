Feature: Weaver CLI behaviour

  Scenario: Streaming a request to the daemon
    Given a running fake daemon
    When the operator runs "observe get-definition --symbol main"
    Then the daemon receives "request_observe_get_definition.jsonl"
    And stdout is "daemon says hello"
    And stderr is "daemon complains"
    And the CLI exits with code 17

  Scenario: Streaming an apply-patch request with stdin
    Given a running fake daemon
    And patch input is available
    When the operator runs "act apply-patch"
    Then the daemon receives "request_act_apply_patch.jsonl"
    And the CLI exits with code 0

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

  Scenario: Routing lifecycle commands through helper
    Given lifecycle responses succeed
    When the operator runs "daemon status"
    Then the lifecycle stub recorded "status"
    And no daemon command was sent
    And the CLI exits with code 0

  Scenario: Reporting lifecycle failures
    Given lifecycle responses fail with socket busy
    When the operator runs "daemon start"
    Then the lifecycle stub recorded "start"
    And stderr contains "already in use"
    And the CLI fails

  Scenario: Stopping the daemon through the lifecycle helper
    Given lifecycle responses succeed
    When the operator runs "daemon stop"
    Then the lifecycle stub recorded "stop"
    And no daemon command was sent
    And the CLI exits with code 0

  # Auto-start scenarios: When a domain command is issued and the daemon is not
  # running, the CLI attempts to start it automatically.

  Scenario: Bare invocation shows short help
    When the operator runs ""
    Then the CLI fails
    And stderr contains "Usage: weaver"
    And stderr contains "observe"
    And stderr contains "act"
    And stderr contains "verify"
    And stderr contains "weaver --help"

  Scenario: Auto-start shows waiting message before spawn failure
    Given auto-start will be triggered
    When the operator runs "observe get-definition --symbol main"
    Then stderr contains "Waiting for daemon start..."
    And stderr contains "failed to spawn"
    And the CLI fails
