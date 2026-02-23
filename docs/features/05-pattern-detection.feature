Feature: Pattern Detection with Confidence Distribution
  As an engineer using boundary to evaluate a codebase
  I want boundary to identify which architectural pattern my module most closely matches
  So that I know whether the DDD scores are meaningful and can see my codebase's pattern fingerprint

  # Patterns detected (independent confidence values in [0.0, 1.0]):
  #   ddd-hexagonal    — distinct layers, domain has high A + low I, ports/adapters present
  #   active-record    — domain types have persistence methods, no distinct layers, A ≈ 0
  #   flat-crud        — no layers, 1–2 packages, all concrete, no abstract types
  #   anemic-domain    — domain package exists but A ≈ 0, no ports, business logic in services
  #   service-layer    — some separation but no ports/adapters, services call data types directly
  #
  # Gate rule:
  #   top confidence ≥ 0.5 → compute all score dimensions (DDD scores shown)
  #   top confidence < 0.5 → report pattern distribution and structural presence only
  #
  # Confidence values are independent — a codebase in transition may score high on
  # multiple patterns simultaneously. Values do NOT sum to 1.0.

  Background:
    Given a Go project with the following packages:
      | package        | interfaces | structs |
      | domain         | 1          | 1       |
      | application    | 0          | 1       |
      | infrastructure | 0          | 1       |
    And "application" imports "domain"
    And "infrastructure" imports "domain"

  # ============================================================================
  # JSON output shape
  # ============================================================================

  @contract
  Scenario: Pattern detection appears in JSON output
    When I run "boundary analyze . --format json"
    Then the JSON output includes a "pattern_detection" object
    And the "pattern_detection" object contains a "patterns" array
    And the "patterns" array contains entries for "ddd-hexagonal", "active-record", "flat-crud", "anemic-domain", and "service-layer"
    And each pattern entry has a "name" field and a "confidence" field
    And every confidence value in the "patterns" array is between 0.0 and 1.0 inclusive
    And the "pattern_detection" object contains a "top_pattern" string field
    And the "pattern_detection" object contains a "top_confidence" number field

  # ============================================================================
  # DDD + Hexagonal detection
  # ============================================================================

  Scenario: A well-structured DDD project is detected as DDD+Hexagonal
    When I run "boundary analyze . --format json"
    Then the "top_pattern" is "ddd-hexagonal"

  Scenario: A well-structured DDD project scores at least 0.5 confidence for DDD+Hexagonal
    When I run "boundary analyze . --format json"
    Then the pattern confidence for "ddd-hexagonal" is at least 0.5

  # ============================================================================
  # Gate: DDD scores shown when top confidence ≥ 0.5
  # ============================================================================

  Scenario: Score dimensions are included when top pattern confidence is at least 0.5
    When I run "boundary analyze . --format json"
    Then the JSON output includes a "score" object with "overall", "layer_isolation", and "dependency_direction" fields

  # ============================================================================
  # Text output
  # ============================================================================

  Scenario: Text output shows the detected pattern name and its confidence
    When I run "boundary analyze ."
    Then the text output includes the detected pattern name
    And the text output includes the top confidence value as a decimal or percentage

  # ============================================================================
  # Standalone scenarios (independent fixtures, no Background applies)
  # ============================================================================

  # ----
  # Flat CRUD detection
  # ----

  Scenario: A flat project with one package and no abstract types is detected as Flat CRUD
    Given a Go project with a single package "flat" containing 3 structs and no interfaces
    When I run "boundary analyze . --format json"
    Then the pattern confidence for "flat-crud" is at least 0.5

  # ----
  # Anemic Domain detection
  # The "services" package imports "domain" to establish coupling that confirms
  # "services" is the logic layer and "domain" is a data container — the key
  # anemic-domain signal. Without this import both packages have I=0.0 and
  # provide near-zero structural signal for pattern discrimination.
  # ----

  Scenario: A project with a domain package containing only structs is detected as Anemic Domain
    Given a Go project with the following packages:
      | package  | interfaces | structs |
      | domain   | 0          | 2       |
      | services | 0          | 1       |
    And "services" imports "domain"
    When I run "boundary analyze . --format json"
    Then the pattern confidence for "anemic-domain" is at least 0.5

  # ----
  # Gate: DDD scores omitted when top confidence < 0.5
  #
  # Fixture reasoning: two packages with identical all-concrete structure, no
  # layer-convention names ("alpha"/"beta"), and no imports between them produce
  # near-zero signal for every pattern. Neither is named "domain" or follows any
  # layer convention, so DDD signals are absent; all types are concrete so
  # flat-crud and anemic-domain signals are weak without distinct packaging cues.
  # The fixture is structurally neutral by design, not "ambiguous by intent".
  # ----

  Scenario: Score dimensions are omitted when no pattern reaches the confidence threshold
    Given a Go project with the following packages:
      | package | interfaces | structs |
      | alpha   | 0          | 2       |
      | beta    | 0          | 2       |
    When I run "boundary analyze . --format json"
    Then the "pattern_detection" object is present
    And the "top_confidence" is below 0.5
    And the JSON output does not include a "score" object

  Scenario: Text output describes the low-confidence state when no pattern is dominant
    Given a Go project with the following packages:
      | package | interfaces | structs |
      | alpha   | 0          | 2       |
      | beta    | 0          | 2       |
    When I run "boundary analyze ."
    Then the text output does not include an overall architectural score

  # ----
  # Next steps suggestion (low-confidence path)
  # FR-27 requires that when top confidence < 0.5, boundary suggests next steps.
  # The specific wording of the suggestion is TBD; deferring until output format
  # is finalized. Marked @pending so the test suite still compiles.
  # ----

  @pending
  Scenario: Text output suggests next steps when no pattern is dominant
    Given a Go project with the following packages:
      | package | interfaces | structs |
      | alpha   | 0          | 2       |
      | beta    | 0          | 2       |
    When I run "boundary analyze ."
    Then the text output includes a suggestion for improving pattern clarity

  # ----
  # Confidence value independence
  # A project with domain structs and infrastructure but no ports provides weak
  # but non-zero signal for both anemic-domain (domain has no interfaces) and
  # flat-crud (no abstract types anywhere). We assert the concrete outcome —
  # both score above zero — rather than a probabilistic "may" statement.
  # ----

  Scenario: A project in transition scores above zero for more than one pattern
    Given a Go project with the following packages:
      | package        | interfaces | structs |
      | domain         | 0          | 3       |
      | infrastructure | 0          | 2       |
    And "infrastructure" imports "domain"
    When I run "boundary analyze . --format json"
    Then more than one pattern entry has confidence above 0.0
