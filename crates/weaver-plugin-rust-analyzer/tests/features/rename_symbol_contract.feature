Feature: rust-analyzer shared rename-symbol contract fixtures

  Scenario: The shared valid request fixture passes contract validation
    Given the shared valid rename-symbol request fixture
    When the rust-analyzer crate validates the shared request fixture
    Then the shared fixture passes contract validation

  Scenario: The shared wrong-operation request fixture is rejected
    Given the shared rename-symbol request fixture with the wrong operation
    When the rust-analyzer crate validates the shared request fixture
    Then the shared fixture fails with a message containing "expects operation"

  Scenario: The shared failed response fixture remains contract-valid
    Given the shared failed response fixture with a reason code
    When the rust-analyzer crate validates the shared response fixture
    Then the shared fixture passes contract validation

  Scenario: The shared non-diff response fixture is rejected
    Given the shared successful non-diff response fixture
    When the rust-analyzer crate validates the shared response fixture
    Then the shared fixture fails with a message containing "diff output"
