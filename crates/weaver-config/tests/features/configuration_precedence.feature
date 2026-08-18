Feature: Configuration precedence under OrthoConfig v0.9.0

  Scenario: CLI locale overrides environment and configuration file values
    Given a configuration file setting the locale to "en-GB"
    And the environment overrides the locale to "fr-FR"
    When the CLI sets the locale to "de-DE"
    Then loading the configuration resolves the locale to "de-DE"

  Scenario: Environment locale overrides a configuration file value
    Given a configuration file setting the locale to "en-GB"
    And the environment overrides the locale to "fr-FR"
    When the configuration loads
    Then loading the configuration resolves the locale to "fr-FR"

  Scenario: Configuration file locale overrides the built-in default
    Given a configuration file setting the locale to "en-GB"
    When the configuration loads
    Then loading the configuration resolves the locale to "en-GB"

  Scenario: Defaults are returned when no configuration sources are provided
    When the configuration loads without overrides
    Then loading the configuration applies the built-in defaults

  Scenario: Invalid environment locale fails closed
    Given a configuration file setting the locale to "en-GB"
    When the environment sets the locale to "not_a_locale"
    Then configuration loading reports an invalid locale

  Scenario: CLI daemon socket overrides environment and configuration file values
    Given a configuration file setting the daemon socket to "tcp://127.0.0.1:6100"
    And the environment overrides the daemon socket to "tcp://127.0.0.1:6200"
    When the CLI sets the daemon socket to "tcp://127.0.0.1:6300"
    Then loading the configuration resolves the daemon socket to "tcp://127.0.0.1:6300"

  Scenario: The last duplicate capability directive wins during configuration loading
    Given a configuration file allowing the Rust rename capability
    And the environment forces the Rust rename capability
    When the CLI denies the Rust rename capability
    Then loading the configuration resolves the capability matrix to deny the Rust rename capability
