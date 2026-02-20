Feature: Architecture Discovery

  As an engineer exploring an unfamiliar codebase
  I want boundary to report how my codebase maps to DDD and Hexagonal Architecture layers
  So that I understand the current structure before deciding how to refactor

  Scenario: Codebase with no recognizable architectural structure
    Given a Go module with no directories matching known DDD layer patterns
    When I run "boundary analyze ."
    Then the report states that no architectural layers were detected
    And the exit code is 0

  Scenario: Codebase with complete DDD layer structure reports all layer components
    Given a Go module with "domain", "application", and "infrastructure" directories
    And each directory contains at least one Go type
    When I run "boundary analyze ."
    Then the report lists components found in the "domain" layer
    And the report lists components found in the "application" layer
    And the report lists components found in the "infrastructure" layer

  Scenario: Codebase where all components map to DDD layers receives full structural presence
    Given a Go module containing only "domain", "application", and "infrastructure" directories
    And each directory contains at least one Go type
    When I run "boundary analyze ."
    Then the output contains "Structural Presence: 100%"

  Scenario: Codebase with partial DDD structure reports classified and unclassified directories
    Given a Go module with a "domain" directory and a "services" directory
    And "domain" matches a known DDD layer pattern
    And "services" does not match any known DDD layer pattern
    When I run "boundary analyze ."
    Then the report lists components found in the "domain" layer
    And the report identifies "services" as an unclassified directory

  Scenario: Codebase with unclassified directories prompts the user to add configuration
    Given a Go module with a "domain" directory and a "services" directory
    And "domain" matches a known DDD layer pattern
    And "services" does not match any known DDD layer pattern
    When I run "boundary analyze ."
    Then the report suggests adding a .boundary.toml to classify unrecognized directories

  Scenario: Configuration override assigns an unrecognized directory to a specified layer
    Given a Go module with an "adapters" directory containing Go types
    And "adapters" does not match any default layer pattern
    And a .boundary.toml that classifies "adapters/**" as the Infrastructure layer
    When I run "boundary analyze ."
    Then components in "adapters" are reported as belonging to the Infrastructure layer

  Scenario: Target path does not exist
    Given no directory exists at the specified path
    When I run "boundary analyze /tmp/nonexistent"
    Then the report states the target path could not be found
    And the exit code is non-zero

  Scenario: Target directory contains no Go files
    Given a directory containing only non-Go files
    When I run "boundary analyze ."
    Then the report states that no supported source files were found
    And the exit code is 0

  Scenario: Target directory contains Go files but no extractable components
    Given a directory containing Go files with no exported types
    When I run "boundary analyze ."
    Then the report states that no components were detected in the analyzed files
    And the exit code is 0
