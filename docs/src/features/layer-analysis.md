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
| **Interface Coverage** | 20% | Ratio of infrastructure adapters that have corresponding port interfaces |

### Interface Coverage

Interface coverage measures how well your infrastructure layer uses ports (interfaces) to decouple from the domain. Boundary counts:

- **Ports**: Components with `ComponentKind::Port` (interfaces/traits in any layer)
- **Adapters**: Components in the **infrastructure** layer with kind `Adapter`, `Repository`, or `Service`

The score is `min(ports / adapters, 1.0) * 100`. If there are no infrastructure adapters, the score is 100.

## Component Extraction

Boundary identifies these component types from source code:

- **Interfaces / Traits** -- Port definitions
- **Structs / Classes** -- Entities, value objects, adapters
- **Imports** -- Dependency relationships between components
- **Functions** -- Service methods, handlers

## Automatic Filtering

### Standard Library Imports

Standard library imports are automatically excluded from the dependency graph. For Go, any import path without a dot (e.g., `fmt`, `encoding/json`) is recognized as stdlib. This prevents stdlib packages from inflating the unclassified component count.

### External Dependencies

Import targets that don't correspond to any source file in the project (e.g., third-party libraries like `github.com/stripe/stripe-go`) are automatically treated as cross-cutting. They appear in the dependency graph but don't trigger layer violations.

## Cross-Cutting Concerns

Some packages (logging, error handling, utilities) don't belong to any layer. Configure these as cross-cutting concerns to exclude them from violation checks:

```toml
[layers]
cross_cutting = ["common/utils/**", "pkg/logger/**", "pkg/errors/**"]
```

Cross-cutting patterns apply to both source files and import targets. Use `**` glob patterns for best results:

```toml
cross_cutting = ["**/methods/**", "**/observability/**", "**/uptime/**"]
```

Cross-cutting components are still tracked in the dependency graph for visualization, but dependencies to/from them don't count as violations.

## Anemic Domain Model Detection

Boundary flags domain entities that have no business methods as potential anemic domain models. This check only applies to components in the **domain** layer — infrastructure DTOs and data transfer objects in other layers are not flagged.

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
