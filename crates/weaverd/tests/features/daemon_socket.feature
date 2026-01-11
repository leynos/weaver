Feature: Daemon socket listener

  Scenario: Accepting a TCP connection
    Given a TCP socket listener is running
    When a client connects
    Then the listener records 1 connection

  Scenario: Accepting concurrent TCP connections
    Given a TCP socket listener is running
    When two clients connect
    Then the listener records 2 connections

  Scenario: Listener binding fails when the socket is in use
    Given a TCP socket is already bound
    When the listener starts on the same socket
    Then starting the listener fails
