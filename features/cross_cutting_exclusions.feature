Feature: Cross-Cutting Concern Exclusions (FR-19)
  Users can mark paths as cross-cutting concerns in .boundary.toml.
  Components in those paths are excluded from layer violation checks and
  layer conformance scoring, but still appear in structural presence.

  Background:
    # fixture: fr19-cross-cutting
    #   domain/user.go           — domain entity
    #   pkg/logger/log.go        — cross-cutting utility (no layer)
    #   infrastructure/repo.go   — imports domain AND pkg/logger
    #   domain/bad.go            — imports infrastructure (violation without cross-cutting)
    # .boundary.toml: cross_cutting = ["pkg/logger/**"]

  @contract
  Scenario: Dependency on cross-cutting package does not count as a violation
    Given a fixture where infrastructure imports a cross-cutting logger package
    When I run "boundary analyze --format json"
    Then no violation is reported for the logger import

  @contract
  Scenario: Cross-cutting components appear in structural presence count
    Given a fixture with one cross-cutting component
    When I run "boundary analyze --format json"
    Then "metrics.classification_coverage.cross_cutting" is at least 1

  Scenario: Cross-cutting components are excluded from layer isolation scoring
    Given a fixture where a cross-cutting package would otherwise lower conformance
    When I run "boundary analyze --format json"
    Then "metrics.classification_coverage.cross_cutting" equals the cross-cutting count

  Scenario: Text output shows cross-cutting count in Classification Coverage
    Given a fixture with cross-cutting components
    When I run "boundary analyze" (text output)
    Then the output shows "Cross-cutting:" with a non-zero count
