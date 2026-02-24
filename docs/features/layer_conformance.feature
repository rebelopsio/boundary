Feature: True Layer Conformance Scoring (FR-28)
  As an architect using boundary,
  I want layer conformance based on (A,I) distance to expected layer region centroids,
  So that the score reflects how well each package fits its assigned architectural layer.

  Background:
    Given the sample-go-project fixture exists

  Scenario: JSON score output uses layer_conformance field
    When I run boundary analyze --score-only --format json
    Then the JSON output contains the key "layer_conformance"
    And the JSON output does not contain the key "layer_isolation"

  Scenario: JSON score output uses dependency_compliance field
    When I run boundary analyze --score-only --format json
    Then the JSON output contains the key "dependency_compliance"
    And the JSON output does not contain the key "dependency_direction"

  Scenario: Layer conformance is a valid percentage
    When I run boundary analyze --score-only --format json
    Then "layer_conformance" is a number between 0 and 100 inclusive

  Scenario: Interface coverage uses min-over-max formula
    Given a project with 2 port interfaces and 1 infrastructure adapter
    When I run boundary analyze --score-only --format json
    Then "interface_coverage" is approximately 50 (±5)
    And the old min(ports/adapters,1.0)*100 formula would give 100

  Scenario: Dependency compliance replaces dependency direction
    When I run boundary analyze in text format
    Then the text output contains "Dependency Compliance"
    And the text output does not contain "Dependency Direction"

  Scenario: Layer conformance replaces layer isolation in text output
    When I run boundary analyze in text format
    Then the text output contains "Layer Conformance"
    And the text output does not contain "Layer Isolation"
