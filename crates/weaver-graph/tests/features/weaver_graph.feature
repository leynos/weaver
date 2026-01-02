Feature: Call graph via LSP call hierarchy

  Scenario: Build a call graph for a symbol with callers and callees
    Given a call hierarchy client with a simple call chain
    When I build a call graph from "main" with depth 1
    Then the graph has 3 nodes and 2 edges
    And the graph includes node "main"
    And the graph includes node "caller"
    And the graph includes node "helper"
    And the graph includes an edge from "caller" to "main"
    And the graph includes an edge from "main" to "helper"

  Scenario: No symbol matches the requested position
    Given a call hierarchy client with no matching symbol
    When I build a call graph from "main" with depth 1
    Then the graph build fails with "symbol_not_found"

  Scenario: Call hierarchy request fails
    Given a call hierarchy client that returns an error
    When I build a call graph from "main" with depth 1
    Then the graph build fails with "validation_error"
