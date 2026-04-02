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
    And the first diagnostic message contains "normalization"

  Scenario: Engine compile_yaml keeps the placeholder for dependency search rules
    Given an engine with default configuration
    When YAML "rules:\n  - id: demo.depends\n    message: detect vulnerable dependency\n    languages: [python]\n    severity: WARNING\n    r2c-internal-project-depends-on:\n      namespace: pypi\n      package: requests\n" is compiled
    Then compilation fails with code "NOT_IMPLEMENTED"
    And the first diagnostic message contains "normalization"

  Scenario: Engine compile_yaml rejects extract mode during execution gating
    Given an engine with default configuration
    When YAML "rules:\n  - id: demo.extract\n    mode: extract\n    message: extract foo\n    languages: [python]\n    severity: WARNING\n    dest-language: python\n    extract: foo($X)\n    pattern: source($X)\n" is compiled
    Then compilation fails with code "E_SEMPAI_UNSUPPORTED_MODE"
    And the first diagnostic message contains "extract"

  Scenario: Engine compile_yaml rejects unknown modes during execution gating
    Given an engine with default configuration
    When YAML "rules:\n  - id: demo.custom\n    mode: custom-mode\n    message: custom mode\n    languages: [python]\n    severity: WARNING\n    pattern: foo($X)\n" is compiled
    Then compilation fails with code "E_SEMPAI_UNSUPPORTED_MODE"
    And the first diagnostic message contains "custom-mode"

  Scenario: Engine compile_dsl returns not-implemented error
    Given an engine with default configuration
    When DSL "pattern(\"fn $F\")" is compiled for language "rust"
    Then compilation fails with code "NOT_IMPLEMENTED"

  Scenario: Engine execute returns not-implemented error
    Given an engine with default configuration
    When a query plan is executed
    Then execution fails with code "NOT_IMPLEMENTED"
