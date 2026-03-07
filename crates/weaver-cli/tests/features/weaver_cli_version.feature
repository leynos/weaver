Feature: Weaver CLI version output

  Scenario: Version flag outputs version and exits successfully
    When the operator runs "--version"
    Then stdout contains "weaver"
    And the CLI exits with code 0

  Scenario: Short version flag outputs version and exits successfully
    When the operator runs "-V"
    Then stdout contains "weaver"
    And the CLI exits with code 0

  Scenario: Version flag produces no stderr output
    When the operator runs "--version"
    Then stderr is ""
    And the CLI exits with code 0

  Scenario: Help flag includes quick-start example
    When the operator runs "--help"
    Then stdout contains "Quick start:"
    And stdout contains "weaver observe get-definition"
    And the CLI exits with code 0
