Feature: Sempai engine stub behaviour

  Scenario: Engine is constructible with default configuration
    Given an engine with default configuration
    Then the engine has max matches per rule of 10000

  Scenario: Engine compile_yaml returns not-implemented error
    Given an engine with default configuration
    When YAML "rules: []" is compiled
    Then compilation fails with code "NOT_IMPLEMENTED"

  Scenario: Engine compile_dsl returns not-implemented error
    Given an engine with default configuration
    When DSL "pattern(\"fn $F\")" is compiled for language "rust"
    Then compilation fails with code "NOT_IMPLEMENTED"
