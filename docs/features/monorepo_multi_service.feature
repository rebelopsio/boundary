Feature: Monorepo / Multi-Service Support (FR-24)
  boundary analyze --per-service discovers each service directory, analyzes
  them independently, and produces per-service scores with an aggregate rollup.
  Shared modules used by multiple services are identified.

  Background:
    # fixture: fr24-monorepo
    #   services/auth/domain/user.go   — auth domain
    #   services/auth/infra/repo.go    — auth infra, imports domain + shared
    #   services/order/domain/order.go — order domain
    #   services/order/infra/store.go  — order infra, imports domain + shared
    #   shared/logger/log.go           — shared utility (imported by both services)
    # .boundary.toml: services_pattern = "services/*"

  @contract
  Scenario: Per-service JSON output contains a "services" array
    Given a monorepo with two services under services/
    When I run "boundary analyze --per-service --format json"
    Then the output has a top-level "services" array with 2 entries

  @contract
  Scenario: Each service entry has its own score
    Given a monorepo with two services
    When I run "boundary analyze --per-service --format json"
    Then each entry in "services" has a "service_name" and a "result" object

  Scenario: Aggregate result is included in per-service output
    Given a monorepo with two services
    When I run "boundary analyze --per-service --format json"
    Then the output contains an "aggregate" object

  Scenario: Shared modules are detected and listed
    Given both services import the same shared/logger package
    When I run "boundary analyze --per-service --format json"
    Then the output contains a "shared_modules" array
    And at least one shared module is identified

  Scenario: Services with sufficient structure show numeric scores in the text table
    Given a monorepo where the auth service has clear domain and infrastructure layers
    When I run "boundary analyze --per-service --format text"
    Then the auth service row shows numeric values in the Overall, Conformance, Compliance, and Iface Cov columns

  Scenario: Services with insufficient structure show suppressed scores in the text table
    Given a monorepo where the order service has only a single file with no recognisable architectural layers
    When I run "boundary analyze --per-service --format text"
    Then the order service row shows "—" in all score columns rather than "0.0"
