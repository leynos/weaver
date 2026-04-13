Feature: Formula normalization

  Scenario: Negation inside disjunction is rejected
    Given an engine with default configuration
    When YAML "rules:\n  - id: bad.not-in-or\n    message: detect\n    languages: [rust]\n    severity: ERROR\n    pattern-either:\n      - pattern: foo($X)\n      - pattern-not: bar($X)\n" is compiled
    Then compilation fails with code "E_SEMPAI_INVALID_NOT_IN_OR"

  Scenario: Conjunction without positive term is rejected
    Given an engine with default configuration
    When YAML "rules:\n  - id: bad.no-positive\n    message: detect\n    languages: [rust]\n    severity: ERROR\n    patterns:\n      - pattern-not: foo($X)\n" is compiled
    Then compilation fails with code "E_SEMPAI_MISSING_POSITIVE_TERM_IN_AND"
