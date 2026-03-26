Feature: Card extraction cache

  Scenario: Cache hit for unchanged file
    Given a fresh cache-backed extractor
    And a Rust cache fixture named "greet"
    When the fixture is extracted twice
    Then the cache records 1 hit
    And the cache records 1 miss
    And the extracted_at timestamp is preserved across requests

  Scenario: Cache miss on first request
    Given a fresh cache-backed extractor
    And a Rust cache fixture named "greet"
    When the fixture is extracted once
    Then the cache records 0 hits
    And the cache records 1 miss

  Scenario: Cache invalidation on content change
    Given a fresh cache-backed extractor
    And a Rust cache fixture named "greet"
    When the fixture is extracted once
    And the Rust cache fixture changes to "welcome"
    And the fixture is extracted once
    Then the cache records 0 hits
    And the cache records 2 misses
    And the cache stores 1 entry

  Scenario: LRU eviction under memory pressure
    Given a cache-backed extractor with capacity 1
    And a Rust cache fixture named "greet"
    When the fixture is extracted once
    And a second Rust cache fixture named "welcome" is extracted once
    Then the cache stores 1 entry
    And the cache records 0 hits
    And the cache records 2 misses

  Scenario: Unsupported language bypasses cache
    Given a fresh cache-backed extractor
    And an unsupported cache fixture
    When extraction fails
    Then the cache stores 0 entries

  Scenario: Position out of range bypasses cache
    Given a fresh cache-backed extractor
    And a Rust cache fixture named "greet"
    And the request position is 99:99
    When extraction fails
    Then the cache stores 0 entries
