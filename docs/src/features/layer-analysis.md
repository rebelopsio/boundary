# Layer Analysis

Boundary classifies source code components into four architectural layers and enforces dependency rules between them.

## Architectural Layers

From innermost (most protected) to outermost:

| Layer | Purpose | Default Patterns |
|-------|---------|-----------------|
| **Domain** | Core business logic, entities, value objects | `**/domain/**`, `**/entity/**`, `**/model/**` |
| **Application** | Use cases, application services, orchestration | `**/application/**`, `**/usecase/**`, `**/service/**` |
| **Infrastructure** | Database adapters, external APIs, persistence | `**/infrastructure/**`, `**/adapter/**`, `**/repository/**`, `**/persistence/**` |
| **Presentation** | HTTP handlers, CLI, API controllers | `**/presentation/**`, `**/handler/**`, `**/api/**`, `**/cmd/**` |

## Dependency Rules

The core rule is that **inner layers must not depend on outer layers**:

```
Domain ← Application ← Infrastructure
                      ← Presentation
```

Valid dependencies:
- Application can import from Domain
- Infrastructure can import from Domain and Application
- Presentation can import from Domain and Application

Violations:
- Domain importing from Infrastructure or Presentation
- Application importing from Infrastructure or Presentation
- Any circular dependency between layers

## Scoring

Boundary calculates three sub-scores that combine into an overall architecture score (0--100):

| Score | Default Weight | What It Measures |
|-------|---------------|-----------------|
| **Layer Isolation** | 40% | Percentage of dependencies that respect layer boundaries |
| **Dependency Direction** | 40% | Whether dependencies flow in the correct direction (inward) |
| **Interface Coverage** | 20% | Ratio of adapters that have corresponding port interfaces |

## Component Extraction

Boundary identifies these component types from source code:

- **Interfaces / Traits** -- Port definitions
- **Structs / Classes** -- Entities, value objects, adapters
- **Imports** -- Dependency relationships between components
- **Functions** -- Service methods, handlers

## Cross-Cutting Concerns

Some packages (logging, error handling, utilities) don't belong to any layer. Configure these as cross-cutting concerns to exclude them from violation checks:

```toml
[layers]
cross_cutting = ["common/utils/**", "pkg/logger/**", "pkg/errors/**"]
```

Cross-cutting components are still tracked in the dependency graph for visualization, but dependencies to/from them don't count as violations.

## Custom Layer Patterns

Override the default patterns in `.boundary.toml` to match your project structure:

```toml
[layers]
domain = ["**/core/**", "**/models/**"]
application = ["**/app/**", "**/usecases/**"]
infrastructure = ["**/infra/**", "**/db/**", "**/clients/**"]
presentation = ["**/web/**", "**/grpc/**"]
```

For monorepos with per-service structures, use [layer overrides](../configuration/boundary-toml.md#layersoverrides).
