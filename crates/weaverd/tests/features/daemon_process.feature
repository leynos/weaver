Feature: Daemon process management

  Scenario: Background launch publishes runtime artefacts
    Given a fresh daemon process world
    When the daemon starts in background mode
    Then daemonisation was requested
    And the daemon wrote the lock file
    And the daemon wrote the pid file
    And the daemon wrote the ready health snapshot
    When shutdown is triggered
    And the daemon run completes
    Then the runtime artefacts are removed

  Scenario: Duplicate start fails while daemon is running
    Given a fresh daemon process world
    When the daemon starts in background mode
    Then daemonisation was requested
    And the daemon wrote the lock file
    And the daemon wrote the pid file
    Then starting the daemon again fails with already running
    When shutdown is triggered
    And the daemon run completes
    Then the runtime artefacts are removed

  Scenario: Stale runtime artefacts are reclaimed
    Given a fresh daemon process world
    And stale runtime artefacts exist
    When the daemon starts in foreground mode
    Then the daemon run succeeds
    And the runtime artefacts are removed
