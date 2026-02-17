# Architecture Modes

Not every codebase follows strict DDD patterns. Boundary supports multiple architecture modes to reduce false positives and match your project's actual design.

## Available Modes

### `ddd` (default)

Strict Domain-Driven Design. Enforces full layer separation:

- Domain entities must not import infrastructure packages
- Adapters must have corresponding port interfaces
- All layer boundary violations are flagged

Best for: projects following hexagonal or clean architecture patterns.

### `active-record`

Relaxed rules for Active Record patterns where domain entities contain persistence logic (e.g., `.Save()`, `.Load()` methods that call the database directly):

- Domain entities importing database drivers are not flagged
- Port/adapter coverage requirements are relaxed
- Layer isolation scoring adjusts expectations

Best for: CRUD-heavy services, Rails-style codebases, or modules where full DDD adds unnecessary complexity.

### `service-oriented`

Designed for service-oriented architectures where the traditional layer model doesn't apply:

- Looser coupling requirements between components
- Focus on service boundary enforcement rather than layer isolation

Best for: microservices with flat internal structure, legacy codebases being gradually improved.

## Global Configuration

Set the architecture mode for the entire project:

```toml
[layers]
architecture_mode = "active-record"
```

## Per-Module Overrides

Real codebases often use different patterns in different modules. Configure per-module modes with layer overrides:

```toml
# Complex domain logic gets strict DDD
[[layers.overrides]]
scope = "services/billing/**"
architecture_mode = "ddd"
domain = ["services/billing/core/**"]
infrastructure = ["services/billing/adapters/**"]

# Simple CRUD module uses Active Record
[[layers.overrides]]
scope = "services/notifications/**"
architecture_mode = "active-record"
```

Cross-module dependencies still enforce layer rules at module boundaries, regardless of each module's internal mode.
