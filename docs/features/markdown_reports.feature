Feature: Markdown Reports (FR-12)
  As a developer or tech lead
  I want to generate an architecture analysis report in Markdown format
  So that I can include it in documentation, wikis, or pull request comments

  Scenario: Markdown report contains all core sections
    Given a Go project with multiple packages
    When I run "boundary analyze . --format markdown"
    Then the output starts with "# Boundary - Architecture Analysis"
    And the output contains a "## Scores" section
    And the output contains a "## Summary" section
    And the output contains a "## Metrics" section

  Scenario: Markdown output contains a Package Metrics section
    Given a project with multiple packages (e.g. domain, application, infrastructure)
    When I run "boundary analyze . --format markdown"
    Then the output contains a "## Package Metrics" section

  Scenario: Package metrics table includes A, I, D columns
    Given a project with multiple packages
    When I run "boundary analyze . --format markdown"
    Then the Package Metrics table has an "A" column for abstractness
    And the Package Metrics table has an "I" column for instability
    And the Package Metrics table has a "D" column for distance from the main sequence

  Scenario: Zone of Pain package appears with zone annotation
    Given a project where the "common" package has A=0.0 and I=0.0 (Zone of Pain)
    When I run "boundary analyze . --format markdown"
    Then the Package Metrics table shows "⚠ Pain" in the Zone column for "common"

  Scenario: Markdown output contains a Pattern Detection section
    Given a project with detectable architectural patterns
    When I run "boundary analyze . --format markdown"
    Then the output contains a "## Pattern Detection" section

  Scenario: Pattern detection shows the top pattern name and confidence
    Given a DDD project
    When I run "boundary analyze . --format markdown"
    Then the Pattern Detection section begins with "Top Pattern: **<name>** (<confidence>% confidence)"
    And the section includes a table of all patterns with their confidence percentages
