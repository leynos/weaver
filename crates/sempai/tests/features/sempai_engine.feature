Feature: Sempai engine facade behaviour

  Scenario: Engine is constructible with default configuration
    Given an engine with default configuration
    Then the engine has max matches per rule of 10000

  Scenario: Engine compile_yaml reports malformed YAML
    Given an engine with default configuration
    When YAML "rules:\n  - id: demo.rule\n    message: detect foo\n    languages: [rust]\n    severity: ERROR\n    pattern: [" is compiled
    Then compilation fails with code "E_SEMPAI_YAML_PARSE"

  Scenario: Engine compile_yaml reports schema errors
    Given an engine with default configuration
    When YAML "rules:\n  - message: detect foo\n    languages: [rust]\n    severity: ERROR\n    pattern: foo($X)\n" is compiled
    Then compilation fails with code "E_SEMPAI_SCHEMA_INVALID"

  Scenario: Engine compile_yaml keeps a post-parse placeholder for valid YAML
    Given an engine with default configuration
    When YAML "rules:\n  - id: demo.rule\n    message: detect foo\n    languages: [rust]\n    severity: ERROR\n    pattern: foo($X)\n" is compiled
    Then compilation fails with code "NOT_IMPLEMENTED"
    And the first diagnostic message contains "normalisation"

  Scenario: Engine compile_dsl returns not-implemented error
    Given an engine with default configuration
    When DSL "pattern(\"fn $F\")" is compiled for language "rust"
    Then compilation fails with code "NOT_IMPLEMENTED"

  Scenario: Engine execute returns not-implemented error
    Given an engine with default configuration
    When a query plan is executed
    Then execution fails with code "NOT_IMPLEMENTED"
