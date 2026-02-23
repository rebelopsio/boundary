Feature: Unclassified Component Handling (FR-23)
  When components cannot be assigned to a layer, boundary reports the
  unclassified percentage prominently and suggests adding patterns to
  .boundary.toml. Unclassified components are excluded from DDD scores
  to avoid inflating results.

  Background:
    # fixture: fr23-unclassified — project with unrecognized directory structure
    #   worker/processor.go — no layer match (default patterns don't cover "worker/")
    #   worker/scheduler.go — same
    #   domain/user.go      — domain layer (matches default "**/domain/**")

  @contract
  Scenario: Unclassified percentage appears in JSON output
    Given a fixture where some components do not match any layer pattern
    When I run "boundary analyze --format json"
    Then "metrics.classification_coverage.unclassified" is greater than 0
    And "metrics.classification_coverage.coverage_percentage" is less than 100

  @contract
  Scenario: Unclassified paths are listed in JSON output
    Given a fixture where "worker/" components are unclassified
    When I run "boundary analyze --format json"
    Then "metrics.classification_coverage.unclassified_paths" is non-empty

  Scenario: Text output shows unclassified count in Classification Coverage
    Given a fixture with unclassified components
    When I run "boundary analyze" (text output)
    Then the output contains "Unclassified:" followed by a non-zero count

  Scenario: Text output suggests adding patterns to .boundary.toml
    Given a fixture with unclassified components
    When I run "boundary analyze" (text output)
    Then the output contains ".boundary.toml"

  Scenario: DDD scores are not inflated by ignoring unclassified components
    Given a fixture with 50% classified and 50% unclassified components
    When I run "boundary analyze --format json"
    Then "score.structural_presence" is at most 60.0
