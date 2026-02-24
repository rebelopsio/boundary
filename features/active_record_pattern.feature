Feature: Active Record Pattern Recognition (FR-20)
  The Active Record pattern allows domain entities to contain persistence methods
  directly. In strict mode (default) these cause violations; in active-record mode
  they are permitted.

  Background:
    # fixture: fr20-active-record
    #   domain/order.go — Order struct with Save()/Load() that imports infrastructure/db
    #   infrastructure/db/db.go — DB connection type
    # .boundary.toml (active-record): architecture_mode = "active-record"
    # (no .boundary.toml = strict mode, uses defaults)

  @contract
  Scenario: In strict mode a domain entity importing infrastructure is a violation
    Given a fixture where a domain struct imports the infrastructure layer
    And no .boundary.toml is present (strict mode)
    When I run "boundary analyze --format json"
    Then the violations list contains a LayerBoundary violation

  @contract
  Scenario: In active-record mode a domain entity importing infrastructure is not a violation
    Given a fixture where a domain struct imports the infrastructure layer
    And .boundary.toml declares architecture_mode = "active-record"
    When I run "boundary analyze --format json"
    Then no LayerBoundary violation is reported

  Scenario: Pattern detection reports active-record confidence
    Given a fixture with Active Record signals (no interfaces, domain imports infra)
    When I run "boundary analyze --format json"
    Then "pattern_detection.patterns" contains an entry for "active-record"
    And the active-record confidence is greater than 0.0
