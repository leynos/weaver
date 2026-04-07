Feature: Graph-slice schema contracts

  Scenario: Default-budget success response includes all required fields
    Given a graph-slice success response with default budget
    When the response is serialized to JSON
    Then the JSON field "status" has value "success"
    And the JSON contains a "slice_version" field
    And the JSON contains a "entry" field
    And the JSON contains a "constraints" field
    And the JSON contains a "cards" field
    And the JSON contains a "edges" field
    And the JSON contains a "spillover" field

  Scenario: Truncated response includes spillover metadata
    Given a graph-slice truncated response with spillover
    When the response is serialized to JSON
    Then the JSON field "spillover.truncated" has value "true"
    And the JSON contains a "spillover.frontier" field

  Scenario: Non-truncated response has empty spillover frontier
    Given a graph-slice success response with default budget
    When the response is serialized to JSON
    Then the JSON field "spillover.truncated" has value "false"

  Scenario: Refusal response includes reason code
    Given a graph-slice refusal with reason "not_yet_implemented"
    When the response is serialized to JSON
    Then the JSON field "status" has value "refusal"
    And the JSON contains a "refusal" field

  Scenario: Default request resolves depth to 2
    Given a graph-slice request with no optional flags
    Then the depth is "2"

  Scenario: Default request resolves direction to both
    Given a graph-slice request with no optional flags
    Then the direction is "both"

  Scenario: Default request includes all edge types
    Given a graph-slice request with no optional flags
    Then the edge types include "call"
    And the edge types include "import"
    And the edge types include "config"

  Scenario: Duplicate edge types are normalized
    Given a graph-slice request with edge types "import,call,import"
    Then the edge types are "call,import"

  Scenario: Invalid depth is rejected
    Given a graph-slice request with depth "abc"
    Then the request is rejected

  Scenario: Unknown edge type is rejected
    Given a graph-slice request with edge types "call,unknown"
    Then the request is rejected

  Scenario: Edges carry resolution_scope enum strings
    Given a graph-slice response with all resolution scopes
    When the response is serialized to JSON
    Then the response contains edge with resolution_scope "full_symbol_table"
    And the response contains edge with resolution_scope "partial_symbol_table"
    And the response contains edge with resolution_scope "lsp"

  Scenario: Constraints echo normalized budget values
    Given a graph-slice success response with default budget
    When the response is serialized to JSON
    Then the JSON field "constraints.budget.max_cards" has value "30"
    And the JSON field "constraints.budget.max_edges" has value "200"
    And the JSON field "constraints.budget.max_estimated_tokens" has value "4000"
