Feature: Graph-slice schema contracts

  Scenario: Default-budget success response includes all required fields
    Given a graph-slice success response with default budget
    When the slice response is serialized to JSON
    Then the slice JSON field "status" has value "success"
    And the slice JSON contains a "slice_version" field
    And the slice JSON contains a "entry" field
    And the slice JSON contains a "constraints" field
    And the slice JSON contains a "cards" field
    And the slice JSON contains a "edges" field
    And the slice JSON contains a "spillover" field

  Scenario: Truncated response includes spillover metadata
    Given a graph-slice truncated response with spillover
    When the slice response is serialized to JSON
    Then the slice JSON field "spillover.truncated" has value "true"
    And the slice JSON contains a "spillover.frontier" field

  Scenario: Non-truncated response has empty spillover frontier
    Given a graph-slice success response with default budget
    When the slice response is serialized to JSON
    Then the slice JSON field "spillover.truncated" has value "false"
    And the slice JSON field "spillover.frontier" is empty

  Scenario: Refusal response includes reason code
    Given a graph-slice refusal with reason "not_yet_implemented"
    When the slice response is serialized to JSON
    Then the slice JSON field "status" has value "refusal"
    And the slice JSON contains a "refusal" field
    And the slice JSON field "refusal.reason" has value "not_yet_implemented"

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
    When the slice response is serialized to JSON
    Then the response contains edge with resolution_scope "full_symbol_table"
    And the response contains edge with resolution_scope "partial_symbol_table"
    And the response contains edge with resolution_scope "lsp"

  Scenario: Constraints echo normalized budget values
    Given a graph-slice success response with default budget
    When the slice response is serialized to JSON
    Then the slice JSON field "constraints.budget.max_cards" has value "30"
    And the slice JSON field "constraints.budget.max_edges" has value "200"
    And the slice JSON field "constraints.budget.max_estimated_tokens" has value "4000"
