Feature: Observe get-card
  Scenario: Supported Rust symbols return success cards
    Given a daemon connection is established for get-card
    And a supported Rust source fixture
    When an observe get-card request is sent for the Rust fixture
    Then the stdout response contains "\"status\":\"success\""
    And the stdout response contains "\"name\":\"greet\""
    And the get-card response exits with status 0

  Scenario: Semantic detail degrades to Tree-sitter provenance
    Given a daemon connection is established for get-card
    And a supported Rust source fixture
    When an observe get-card semantic request is sent for the Rust fixture
    Then the stdout response contains "tree_sitter_degraded_semantic"
    And the get-card response exits with status 0

  Scenario: Unsupported files return structured refusals
    Given a daemon connection is established for get-card
    And an unsupported text fixture
    When an observe get-card request is sent for the unsupported fixture
    Then the stdout response contains "\"reason\":\"unsupported_language\""
    And the get-card response exits with status 1

  Scenario: Empty supported files return no-symbol refusals
    Given a daemon connection is established for get-card
    And an empty Python fixture
    When an observe get-card request is sent for the empty Python fixture
    Then the stdout response contains "\"reason\":\"no_symbol_at_position\""
    And the get-card response exits with status 1

  Scenario: Repeated requests reuse cached extraction output
    Given a daemon connection is established for get-card
    And a supported Rust source fixture
    When the same observe get-card request is sent twice for the Rust fixture
    Then both responses are identical
    And the get-card response exits with status 0

  Scenario: File edits invalidate cached extraction output
    Given a daemon connection is established for get-card
    And a supported Rust source fixture
    When an observe get-card request is sent for the Rust fixture
    And the Rust fixture is rewritten to return "welcome"
    And an observe get-card request is sent for the Rust fixture
    Then the latest stdout response contains "\"name\":\"welcome\""
    And the latest stdout response differs from the first response
    And the get-card response exits with status 0
