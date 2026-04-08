Feature: Sempai YAML rule parsing

  Scenario: Parse a valid legacy search rule
    Given YAML "rules:\n  - id: demo.rule\n    message: detect foo\n    languages: [rust]\n    severity: WARNING\n    pattern: foo($X)\n"
    When the rule file is parsed
    Then parsing succeeds with 1 rule

  Scenario: Parse a valid dependency search rule
    Given YAML "rules:\n  - id: demo.depends\n    message: detect vulnerable dependency\n    languages: [python]\n    severity: WARNING\n    r2c-internal-project-depends-on:\n      namespace: pypi\n      package: requests\n"
    When the rule file is parsed
    Then parsing succeeds with 1 rule

  Scenario: Reject malformed YAML
    Given YAML "rules:\n  - id: demo.rule\n    message: detect foo\n    languages: [rust]\n    severity: WARNING\n    pattern: ["
    When the rule file is parsed
    Then parsing fails with diagnostic code "E_SEMPAI_YAML_PARSE"

  Scenario: Reject a rule missing its id
    Given YAML "rules:\n  - message: detect foo\n    languages: [rust]\n    severity: WARNING\n    pattern: foo($X)\n"
    When the rule file is parsed
    Then parsing fails with diagnostic code "E_SEMPAI_SCHEMA_INVALID"
