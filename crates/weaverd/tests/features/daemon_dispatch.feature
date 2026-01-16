Feature: Daemon JSONL request dispatch

  The daemon reads JSONL requests from connected clients, routes them to the
  appropriate domain handler, and streams responses back. Malformed requests
  and unknown domains or operations result in structured error responses.

  Scenario: Dispatching a valid observe command
    Given a daemon connection is established
    When a valid observe get-definition request is sent
    Then the response includes an exit message with status 1
    And the response includes a not implemented message

  Scenario: Rejecting malformed JSONL
    Given a daemon connection is established
    When a malformed JSONL request is sent
    Then the response includes an error message
    And the response includes an exit message with status 1

  Scenario: Rejecting unknown domain
    Given a daemon connection is established
    When a request with unknown domain "bogus" is sent
    Then the response includes an unknown domain error
    And the response includes an exit message with status 1

  Scenario: Rejecting unknown operation
    Given a daemon connection is established
    When a request with unknown operation "nonexistent" in domain "observe" is sent
    Then the response includes an unknown operation error
    And the response includes an exit message with status 1

  Scenario: Dispatching a valid act command
    Given a daemon connection is established
    When a valid act apply-patch request is sent
    Then the response includes an exit message with status 1
    And the response includes a not implemented message

  Scenario: Dispatching a valid verify command
    Given a daemon connection is established
    When a valid verify diagnostics request is sent
    Then the response includes an exit message with status 1
    And the response includes a not implemented message
