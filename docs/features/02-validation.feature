Feature: Architecture Validation
  As an engineer maintaining a codebase
  I want boundary to detect and enforce architectural boundaries
  So that violations are caught automatically before they compound

  Scenario: Codebase with correct layering reports no violations
    Given a Go module with "domain", "application", and "infrastructure" directories
    And each directory contains at least one Go type
    And no component imports across a forbidden layer boundary
    When I run "boundary analyze ."
    Then the report states that no violations were found

  Scenario: Domain component importing infrastructure is reported as a layer boundary violation
    Given a Go module where a type in the "domain" directory imports from the "infrastructure" directory
    When I run "boundary analyze ."
    Then the report identifies a "layer boundary violation" between "domain" and "infrastructure"
    And the violation includes a suggestion for how to resolve it

  Scenario: analyze command always exits zero regardless of violations
    Given a Go module where a type in the "domain" directory imports from the "infrastructure" directory
    When I run "boundary analyze ."
    Then the exit code is 0

  Scenario: check command exits non-zero when violations meet the default fail-on threshold
    Given a Go module where a type in the "domain" directory imports from the "infrastructure" directory
    When I run "boundary check ."
    Then the exit code is non-zero

  Scenario: check command exits zero when only warning-level violations are present and fail-on is error
    Given a Go module with an infrastructure adapter and no matching domain port interface
    And boundary reports this condition as a "missing port" warning
    When I run "boundary check . --fail-on error"
    Then the exit code is 0

  Scenario: check command exits non-zero when fail-on threshold is lowered to warning
    Given a Go module with an infrastructure adapter and no matching domain port interface
    And boundary reports this condition as a "missing port" warning
    When I run "boundary check . --fail-on warning"
    Then the exit code is non-zero

  Scenario: check command exits zero but does not claim a clean architecture when no layers are detected
    Given a Go module where no directories match any known DDD layer pattern
    When I run "boundary check ."
    Then the exit code is 0
    And the report states that no architectural layers were detected
    And the report does not state that no violations were found
