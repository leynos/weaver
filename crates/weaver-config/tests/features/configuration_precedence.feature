Feature: Configuration precedence

  Scenario: CLI overrides environment and configuration file values
    Given a configuration file setting the daemon socket to "tcp://127.0.0.1:6100"
    And the environment overrides the daemon socket to "tcp://127.0.0.1:6200"
    When the CLI sets the daemon socket to "tcp://127.0.0.1:6300"
    Then loading the configuration resolves the daemon socket to "tcp://127.0.0.1:6300"

  Scenario: Defaults are returned when no configuration sources are provided
    When the configuration loads without overrides
    Then loading the configuration applies the built-in defaults

  Scenario: CLI locale overrides environment and configuration file values
    Given a configuration file setting the locale to "en-GB"
    And the environment overrides the locale to "fr-FR"
    When the CLI sets the locale to "de-DE"
    Then loading the configuration resolves the locale to "de-DE"
