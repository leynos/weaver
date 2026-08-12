Feature: Configuration precedence under OrthoConfig v0.9.0

  Scenario: CLI overrides environment and configuration file values
    Given a configuration file setting the locale to "en-GB"
    And an environment layer setting the locale to "fr-FR"
    When a CLI layer sets the locale to "de-DE"
    Then the resolved locale is "de-DE"

  Scenario: Defaults apply when no external layers are present
    When the configuration layers are merged without overrides
    Then the built-in Weaver defaults are returned

  Scenario: Invalid higher-precedence input fails closed
    Given a configuration file setting the locale to "en-GB"
    When an environment layer sets the locale to "not_a_locale"
    Then configuration loading reports an invalid locale

  Scenario: The last duplicate capability directive wins
    Given lower layers allow the Rust rename capability
    When a CLI layer denies the Rust rename capability
    Then the resolved capability matrix denies the Rust rename capability
