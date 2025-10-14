Feature: Configuration precedence

  Scenario: CLI overrides environment and configuration file values
    Given a configuration file setting the daemon socket to "tcp://127.0.0.1:6100"
    And the environment overrides the daemon socket to "tcp://127.0.0.1:6200"
    When the CLI sets the daemon socket to "tcp://127.0.0.1:6300"
    Then loading the configuration resolves the daemon socket to "tcp://127.0.0.1:6300"

  Scenario: Defaults are returned when no configuration sources are provided
    When the configuration loads without overrides
    Then loading the configuration applies the built-in defaults
