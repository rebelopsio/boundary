# Monorepo Support

Boundary supports analyzing monorepos with multiple services, shared modules, and per-service architecture rules.

## Per-Service Analysis

Use the `--per-service` flag to analyze each service independently:

```bash
boundary analyze . --per-service
```

This produces a separate report for each service discovered under the configured services pattern, plus an aggregate summary.

### Configuring the Services Pattern

Tell Boundary where your services live:

```toml
[project]
services_pattern = "services/*"
```

This matches directories like `services/auth/`, `services/billing/`, `services/notifications/`, etc. Each is analyzed as an independent unit with its own scores.

## Per-Service Layer Overrides

Each service may have its own internal structure. Use layer overrides to configure patterns per-service:

```toml
# Global defaults
[layers]
domain = ["**/domain/**"]
application = ["**/application/**"]
infrastructure = ["**/infrastructure/**"]

# Auth service has a different structure
[[layers.overrides]]
scope = "services/auth/**"
domain = ["services/auth/core/**"]
infrastructure = ["services/auth/server/**", "services/auth/adapters/**"]

# Shared modules
[[layers.overrides]]
scope = "common/modules/*/**"
domain = ["common/modules/*/domain/**"]
application = ["common/modules/*/app/**"]
```

## Cross-Service Dependencies

When analyzing the full monorepo (without `--per-service`), Boundary tracks dependencies between services. Cross-service dependencies that violate layer rules are flagged, helping enforce clean boundaries at service boundaries.

## Shared Modules

Shared modules (e.g., `common/`, `pkg/`) that are used across multiple services can be configured as cross-cutting concerns if they don't belong to any specific layer:

```toml
[layers]
cross_cutting = ["common/utils/**", "pkg/logger/**"]
```

Or given their own layer overrides if they contain domain logic:

```toml
[[layers.overrides]]
scope = "common/modules/users/**"
domain = ["common/modules/users/domain/**"]
application = ["common/modules/users/app/**"]
```
