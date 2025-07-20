Feature: project status command
  Scenario: daemon auto-start
    Given a temporary runtime dir
    When I invoke the project-status command
    Then the output includes a project status line
