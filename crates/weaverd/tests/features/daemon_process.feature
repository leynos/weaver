Feature: Daemon process management

  Scenario: Background launch publishes lifecycle artefacts
    Given a fresh daemon process world
    When the daemon starts in background mode
    Then daemonisation was requested
    And the daemon wrote the lock file
    And the daemon wrote the pid file
    And the daemon wrote the ready health snapshot
    And the daemon recorded the starting health snapshot
    When shutdown is triggered
    Then the daemon wrote the stopping health snapshot
    And the daemon run completes
    Then the runtime artefacts are removed

  Scenario: Duplicate start fails while daemon is running
    Given a fresh daemon process world
    When the daemon starts in background mode
    And the daemon wrote the ready health snapshot
    Then daemonisation was requested
    And the daemon wrote the lock file
    And the daemon wrote the pid file
    And the daemon recorded the starting health snapshot
    And starting the daemon again fails with already running
    When shutdown is triggered
    Then the daemon wrote the stopping health snapshot
    And the daemon run completes
    Then the runtime artefacts are removed

  Scenario: Stale runtime artefacts are reclaimed
    Given a fresh daemon process world
    And stale runtime artefacts exist
    When the daemon starts in foreground mode
    Then the daemon run succeeds
    And the stale runtime pid is replaced with the current process id
    And the runtime artefacts are removed

  Scenario: Stale runtime artefacts with invalid pid are reclaimed
    Given a fresh daemon process world
    And stale runtime artefacts with invalid pid exist
    When the daemon starts in foreground mode
    Then the daemon run succeeds
    And the stale runtime pid is replaced with the current process id
    And the runtime artefacts are removed

  Scenario: Missing pid file indicates startup in progress
    Given a fresh daemon process world
    And a lock without a pid file exists
    When the daemon starts in foreground mode
    Then the daemon run fails with launch already in progress
    And the lock file remains in place

  Scenario: Invalid configuration fails the daemon run
    Given a fresh daemon process world
    When the daemon starts in foreground mode with invalid configuration
    Then the daemon run fails with invalid configuration

  Scenario: Waiting for readiness without a running daemon fails
    Given a fresh daemon process world
    When we wait for the daemon to become ready
    Then waiting for readiness fails
