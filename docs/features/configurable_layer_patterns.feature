Feature: Configurable Layer Classification Patterns (FR-18)
  Users can define custom path-to-layer mappings in .boundary.toml to support
  non-standard directory structures and monorepos with per-service layouts.

  Background:
    # fixture: fr18-custom-layers — non-standard structure requiring custom .boundary.toml
    #   services/auth/core/user.go       → domain (via override)
    #   services/auth/server/http.go     → infrastructure (via override)
    #   services/auth/app/handler.go     → application (via override)
    #   (no default "domain/", "infrastructure/" paths)

  @contract
  Scenario: Custom domain pattern classifies a file as domain
    Given a fixture with scope "services/auth/**" and domain = ["services/auth/core/**"]
    When I run "boundary analyze --format json"
    Then the "core" package appears in "components_by_layer.domain"

  @contract
  Scenario: Custom infrastructure pattern classifies a file as infrastructure
    Given a fixture with scope "services/auth/**" and infrastructure = ["services/auth/server/**"]
    When I run "boundary analyze --format json"
    Then the "server" package appears in "components_by_layer.infrastructure"

  Scenario: Paths outside the override scope fall back to global patterns
    Given a fixture with a scoped override for "services/auth/**"
    When a file lives outside that scope
    Then it is classified using global [layers] patterns

  Scenario: Classification coverage is 100% when all paths are covered by patterns
    Given a fixture where every file matches a configured layer pattern
    When I run "boundary analyze --format json"
    Then "metrics.classification_coverage.coverage_percentage" equals 100.0

  Scenario: Violation detected when infrastructure imports domain via custom patterns
    Given a fixture where server/ imports core/ (infrastructure → domain is valid)
    When there is a violation (domain imports infrastructure)
    Then that violation appears in the violations list
