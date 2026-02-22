Feature: R.C. Martin Package Metrics
  As an engineer evaluating architectural health
  I want boundary to compute Instability, Abstractness, and Distance per package
  So that I can see where each package falls on the main sequence and identify structural problems

  # Formulas (all values in range 0.0–1.0, rounded to 2 decimal places):
  #   A (Abstractness)  = Na / Nc          — Na: abstract types, Nc: total real components
  #   I (Instability)   = Ce / (Ca + Ce)   — Ce: efferent coupling, Ca: afferent coupling
  #   D (Distance)      = |A + I - 1|
  #
  # Special cases:
  #   Ca + Ce = 0 → I = 0.0 (package present in output, not excluded)
  #   Nc = 0      → package excluded from package_metrics entirely

  # ============================================================================
  # Scenarios using the shared three-package DDD project
  # ============================================================================

  Background:
    Given a Go project with the following packages:
      | package        | interfaces | structs |
      | domain         | 1          | 1       |
      | application    | 0          | 1       |
      | infrastructure | 0          | 1       |
    And "application" imports "domain"
    And "infrastructure" imports "domain"

  # domain:         Na=1, Nc=2 → A=0.5 | Ca=2, Ce=0 → I=0.0 | D=|0.5+0.0-1|=0.5
  # application:    Na=0, Nc=1 → A=0.0 | Ca=0, Ce=1 → I=1.0 | D=|0.0+1.0-1|=0.0
  # infrastructure: Na=0, Nc=1 → A=0.0 | Ca=0, Ce=1 → I=1.0 | D=|0.0+1.0-1|=0.0

  Scenario: A mixed package has abstractness proportional to its abstract type count
    When I run "boundary analyze . --format json"
    Then the package metrics for "domain" include abstractness = 0.5

  Scenario: A fully concrete package has abstractness 0.0
    When I run "boundary analyze . --format json"
    Then the package metrics for "infrastructure" include abstractness = 0.0

  Scenario: A package imported by others but importing nothing has instability 0.0
    When I run "boundary analyze . --format json"
    Then the package metrics for "domain" include instability = 0.0

  Scenario: A leaf package that imports others but has no dependents has instability 1.0
    When I run "boundary analyze . --format json"
    Then the package metrics for "application" include instability = 1.0

  Scenario: A concrete unstable package is on the main sequence
    # infrastructure: A=0.0, I=1.0 → D = |0.0 + 1.0 - 1| = 0.0
    When I run "boundary analyze . --format json"
    Then the package metrics for "infrastructure" include distance = 0.0

  @contract
  Scenario: Package metrics appear in JSON output
    When I run "boundary analyze . --format json"
    Then the JSON output includes a "package_metrics" array
    And each entry contains "package", "abstractness", "instability", and "distance" fields

  Scenario: Text output shows overall score but not per-package metric fields
    When I run "boundary analyze ."
    Then the text output includes an overall architectural score
    And the text output does not contain the word "abstractness"
    And the text output does not contain the word "instability"

  # ============================================================================
  # Standalone scenarios (independent fixtures, no Background applies)
  # ============================================================================

  Scenario: An isolated package with no internal coupling has instability 0.0
    # Special case: Ca + Ce = 0 → I = 0.0 (defined, not undefined)
    Given a Go project with a single package "util" containing 1 struct and no internal imports
    And no other package imports "util"
    When I run "boundary analyze . --format json"
    Then the package metrics for "util" include instability = 0.0

  Scenario: An isolated package with real components still appears in the metrics output
    # Distinguishes the Ca+Ce=0 special case from the Nc=0 exclusion rule
    Given a Go project with a single package "util" containing 1 struct and no internal imports
    And no other package imports "util"
    When I run "boundary analyze . --format json"
    Then the "package_metrics" array includes an entry for "util"

  Scenario: A package in the Zone of Pain has distance 1.0
    # Concrete (A=0.0) and stable (I=0.0): D = |0.0 + 0.0 - 1| = 1.0
    Given a Go project where a "common" package has 0 interfaces and 1 struct
    And "common" imports no internal packages
    And "serviceA" imports "common"
    And "serviceB" imports "common"
    When I run "boundary analyze . --format json"
    Then the package metrics for "common" include distance = 1.0

  Scenario: A package in the Zone of Uselessness has distance 1.0
    # Abstract (A=1.0) and unstable (I=1.0): D = |1.0 + 1.0 - 1| = 1.0
    Given a Go project where an "abstractions" package has 1 interface and 0 structs
    And "abstractions" imports "foundation"
    And no other package imports "abstractions"
    When I run "boundary analyze . --format json"
    Then the package metrics for "abstractions" include distance = 1.0

  Scenario: A package with no real components is excluded from the metrics output
    # Special case: Nc = 0 → excluded entirely (not present with zero values)
    Given a Go project containing a package "empty" with no exported types
    When I run "boundary analyze . --format json"
    Then the "package_metrics" array does not include an entry for "empty"
