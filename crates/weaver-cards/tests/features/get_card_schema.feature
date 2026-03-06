Feature: Symbol card schema contracts

  Scenario: Minimal detail card omits optional sections
    Given a symbol card at "minimal" detail level
    When the card is serialized to JSON
    Then the JSON contains a "card_version" field
    And the JSON contains a "symbol" field
    And the JSON contains a "provenance" field
    And the JSON does not contain a "signature" field
    And the JSON does not contain a "lsp" field

  Scenario: Structure detail card includes signature and doc
    Given a symbol card at "structure" detail level
    When the card is serialized to JSON
    Then the JSON contains a "signature" field
    And the JSON contains a "doc" field
    And the JSON contains a "structure" field
    And the JSON contains a "metrics" field

  Scenario: Refusal response includes reason code
    Given a refusal response with reason "not_yet_implemented"
    When the response is serialized to JSON
    Then the JSON field "status" has value "refusal"
    And the JSON contains a "refusal" field

  Scenario: Success response wraps a card
    Given a success response with a "structure" detail card
    When the response is serialized to JSON
    Then the JSON field "status" has value "success"
    And the JSON contains a "card" field

  Scenario: Default detail level is structure
    Given a get-card request with no detail flag
    Then the detail level is "structure"
