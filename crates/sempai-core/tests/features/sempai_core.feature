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

  Scenario: Parser diagnostic JSON uses the stable schema fields
    Given a parser diagnostic with code "E_SEMPAI_YAML_PARSE" and message "invalid YAML"
    When the diagnostic report is serialized to JSON
    Then the first diagnostic JSON contains key "code"
    And the first diagnostic JSON contains key "message"
    And the first diagnostic JSON contains key "primary_span"
    And the first diagnostic JSON contains key "notes"
    And the first diagnostic JSON does not contain key "span"

  Scenario: Invalid diagnostic code payload fails deterministically
    Given diagnostic code payload "E_SEMPAI_NOT_A_REAL_CODE"
    When the diagnostic code payload is deserialized
    Then deserialization fails with message containing "E_SEMPAI_NOT_A_REAL_CODE"

  Scenario: Null primary span remains explicit in JSON
    Given a validator diagnostic with code "E_SEMPAI_SCHEMA_INVALID" and message "missing id"
    When the diagnostic report is serialized to JSON
    Then the first diagnostic JSON contains key "primary_span" with value "null"
