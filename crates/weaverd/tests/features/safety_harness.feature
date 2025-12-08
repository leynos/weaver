Feature: Double-Lock safety harness

  The Double-Lock safety harness ensures that all code modifications pass
  syntactic and semantic validation before being committed to the filesystem.
  This protects the codebase from corrupted or broken changes.

  Scenario: Successful edit passes both locks and commits changes
    Given a source file "test.txt" with content "hello world"
    And a syntactic lock that passes
    And a semantic lock that passes
    When an edit replaces "hello" with "greetings"
    Then the transaction commits successfully
    And the file contains "greetings world"

  Scenario: Syntactic lock failure prevents commit
    Given a source file "test.txt" with content "hello world"
    And a syntactic lock that fails with "parse error at line 1"
    And a semantic lock that passes
    When an edit replaces "hello" with "greetings"
    Then the transaction fails with a syntactic lock error
    And the file is unchanged

  Scenario: Semantic lock failure prevents commit
    Given a source file "test.txt" with content "hello world"
    And a syntactic lock that passes
    And a semantic lock that fails with "type error at line 1"
    When an edit replaces "hello" with "greetings"
    Then the transaction fails with a semantic lock error
    And the file is unchanged

  Scenario: Semantic backend unavailability surfaces error
    Given a source file "test.txt" with content "hello world"
    And a syntactic lock that passes
    And a semantic lock that is unavailable with "LSP server crashed"
    When an edit replaces "hello" with "greetings"
    Then the transaction fails with a backend error
    And the file is unchanged

  Scenario: Empty transaction returns no changes
    Given a syntactic lock that passes
    And a semantic lock that passes
    When no edits are submitted
    Then the transaction reports no changes

  Scenario: Multiple file edits are committed atomically
    Given a source file "file1.txt" with content "aaa"
    And a source file "file2.txt" with content "bbb"
    And a syntactic lock that passes
    And a semantic lock that passes
    When an edit replaces "aaa" with "AAA" in "file1.txt"
    And an edit replaces "bbb" with "BBB" in "file2.txt"
    Then the transaction commits successfully
    And the file "file1.txt" contains "AAA"
    And the file "file2.txt" contains "BBB"

  Scenario: Multi-file transaction fails if any file has syntactic errors
    Given a source file "file1.txt" with content "aaa"
    And a source file "file2.txt" with content "bbb"
    And a syntactic lock that fails with "syntax error in file2.txt"
    And a semantic lock that passes
    When an edit replaces "aaa" with "AAA" in "file1.txt"
    And an edit replaces "bbb" with "BBB" in "file2.txt"
    Then the transaction fails with a syntactic lock error
    And the file "file1.txt" is unchanged
    And the file "file2.txt" is unchanged

  Scenario: New file creation passes validation
    Given no existing file "new_file.txt"
    And a syntactic lock that passes
    And a semantic lock that passes
    When an edit creates "new_file.txt" with content "fresh content"
    Then the transaction commits successfully
    And the file "new_file.txt" contains "fresh content"
