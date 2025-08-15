Feature: get definition command
  Scenario: daemon auto-start
    Given a temporary runtime dir
    When I invoke the get-definition command for file "foo.py" line 1 char 0
    Then the output includes a symbol line
