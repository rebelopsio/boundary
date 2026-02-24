Feature: Hybrid Architecture Tolerance (FR-22)
  Different modules in a codebase can use different architectural patterns.
  Per-module architecture_mode in [[layers.overrides]] adjusts what constitutes
  a violation for that module without affecting the rest of the project.

  Background:
    # fixture: fr22-hybrid
    #   services/legacy/domain/order.go — imports services/legacy/infra (active-record)
    #   services/legacy/infra/db.go     — DB type
    #   services/modern/domain/user.go  — pure domain entity (no infra imports)
    #   services/modern/infra/repo.go   — imports domain (correct direction)
    # .boundary.toml:
    #   [[layers.overrides]]
    #   scope = "services/legacy/**"
    #   architecture_mode = "active-record"

  @contract
  Scenario: Legacy module in active-record mode has no violations
    Given services/legacy/ is configured with architecture_mode = "active-record"
    When I run "boundary analyze --format json"
    Then no LayerBoundary violation is reported for the legacy module

  @contract
  Scenario: Modern module in DDD mode still enforces layer rules
    Given services/modern/ uses default DDD mode
    And its architecture is clean (infra imports domain, not vice versa)
    When I run "boundary analyze --format json"
    Then no LayerBoundary violation is reported for the modern module
