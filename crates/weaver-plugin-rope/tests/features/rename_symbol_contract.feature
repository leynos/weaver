Feature: Rope shared rename-symbol contract fixtures

  Scenario: The shared valid request fixture passes contract validation
    Given the shared valid rename-symbol request fixture
    When the rope crate validates the shared request fixture
    Then the shared fixture passes contract validation

  Scenario: The shared missing-uri request fixture is rejected
    Given the shared rename-symbol request fixture missing uri
    When the rope crate validates the shared request fixture
    Then the shared fixture fails with a message containing "uri"

  Scenario: The shared diff response fixture passes contract validation
    Given the shared successful diff response fixture
    When the rope crate validates the shared response fixture
    Then the shared fixture passes contract validation

  Scenario: The shared non-diff response fixture is rejected
    Given the shared successful non-diff response fixture
    When the rope crate validates the shared response fixture
    Then the shared fixture fails with a message containing "diff output"
