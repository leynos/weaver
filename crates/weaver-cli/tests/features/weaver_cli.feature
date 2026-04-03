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
    And stderr contains "error: operation required for domain 'observe'"
    And stderr contains "Available operations:"
    And stderr contains "get-card"
    And stderr contains "weaver observe get-definition --help"
    And no daemon command was sent

  Scenario: Rejecting an unknown domain
    When the operator runs "unknown-domain"
    Then the CLI fails
    And stderr contains "error: unknown domain 'unknown-domain'"
    And stderr contains "Valid domains: observe, act, verify"
    And stderr does not contain "Did you mean"
    And no daemon command was sent

  Scenario: Rejecting an unknown domain before daemon startup when an operation is present
    When the operator runs "unknown-domain get-definition"
    Then the CLI fails
    And stderr contains "error: unknown domain 'unknown-domain'"
    And stderr contains "Valid domains: observe, act, verify"
    And stderr does not contain "Waiting for daemon start..."
    And no daemon command was sent

  Scenario: Suggesting the closest valid domain for a typo
    When the operator runs "obsrve get-definition"
    Then the CLI fails
    And stderr contains "error: unknown domain 'obsrve'"
    And stderr contains "Valid domains: observe, act, verify"
    And stderr contains "Did you mean 'observe'?"
    And stderr does not contain "Waiting for daemon start..."
    And no daemon command was sent

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

  Scenario: Rendering unknown-operation alternatives for humans
    Given a running fake daemon emitting an unknown-operation payload
    When the operator runs "--output human observe nonexistent"
    Then the CLI fails
    And stderr contains "error: unknown operation 'nonexistent' for domain 'observe'"
    And stderr contains "Available operations:\n  get-definition\n  find-references\n  grep\n  diagnostics\n  call-hierarchy\n  get-card"

  Scenario: Forwarding unknown-operation payloads in JSON mode
    Given a running fake daemon emitting an unknown-operation payload
    When the operator runs "--output json observe nonexistent"
    Then the CLI fails
    And stderr contains "\"type\":\"UnknownOperation\""
    And stderr contains "\"known_operations\":[\"get-definition\",\"find-references\",\"grep\",\"diagnostics\",\"call-hierarchy\",\"get-card\"]"
    And stderr contains "\"operation\":\"nonexistent\""

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
