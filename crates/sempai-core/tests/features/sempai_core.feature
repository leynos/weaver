Feature: Sempai core type construction and serialization

  Scenario: Span serializes to JSON with byte and line/column fields
    Given a span from bytes "10..42" at lines "2:0..4:0"
    When the span is serialized to JSON
    Then the JSON contains key "start_byte" with value "10"
    And the JSON contains key "end_byte" with value "42"

  Scenario: Language enum round-trips through serde
    Given language "rust"
    When the language is serialized and deserialized
    Then the round-tripped language equals the original

  Scenario: DiagnosticReport formats with code and message
    Given a diagnostic with code "E_SEMPAI_YAML_PARSE" and message "invalid YAML"
    When the diagnostic report is formatted
    Then the formatted output contains "E_SEMPAI_YAML_PARSE"
    And the formatted output contains "invalid YAML"

  Scenario: DiagnosticReport not_implemented includes feature name
    Given a not-implemented report for feature "compile_yaml"
    When the diagnostic report is formatted
    Then the formatted output contains "NOT_IMPLEMENTED"
    And the formatted output contains "compile_yaml"
