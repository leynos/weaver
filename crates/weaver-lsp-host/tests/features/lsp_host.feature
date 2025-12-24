Feature: LSP host core routing

  Scenario: Initialises Rust server and routes core requests
    Given stub servers for all primary languages
    When rust is initialised
    And rust handles a definition request
    And rust handles a references request
    And rust handles a diagnostics request
    Then rust capabilities are available from the server
    And rust recorded a definition call
    And rust recorded a references call
    And rust recorded a diagnostics call

  Scenario: Document sync notifications are routed
    Given stub servers for all primary languages
    When rust opens a document
    And rust changes a document
    And rust closes a document
    Then rust recorded a did open call
    And rust recorded a did change call
    And rust recorded a did close call

  Scenario: Deny override blocks unsupported capability
    Given a python server missing references
    And a deny override for python references
    When python is initialised
    And python handles a references request
    Then the request fails with an unavailable capability error
    And python recorded only initialisation

  Scenario: Force override enables diagnostics
    Given a typescript server missing diagnostics
    And a force override for typescript diagnostics
    When typescript handles a diagnostics request
    Then diagnostics succeed via override
    And typescript recorded a diagnostics call

  Scenario: Server initialisation failures surface errors
    Given a rust server that fails during initialisation
    When rust is initialised
    Then the request fails with a server error

  Scenario: Document sync failures surface errors
    Given a rust server that fails during document sync
    When rust changes a document
    Then the document sync request fails with a server error
