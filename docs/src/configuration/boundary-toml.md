# Configuration: .boundary.toml

Boundary is configured via a `.boundary.toml` file in your project root. Run `boundary init` to generate a starter config.

## Full Reference

```toml
[project]
languages = ["go"]
exclude_patterns = ["vendor/**", "**/*_test.go", "**/testdata/**"]
# services_pattern = "services/*"   # For monorepo per-service analysis

[layers]
# Glob patterns to classify files into architectural layers.
domain = ["**/domain/**", "**/entity/**", "**/model/**"]
application = ["**/application/**", "**/usecase/**", "**/service/**"]
infrastructure = ["**/infrastructure/**", "**/adapter/**", "**/repository/**", "**/persistence/**"]
presentation = ["**/presentation/**", "**/handler/**", "**/api/**", "**/cmd/**"]

# Paths exempt from layer violation checks (cross-cutting concerns)
# cross_cutting = ["common/utils/**", "pkg/logger/**", "pkg/errors/**"]

# Global architecture mode: "ddd" (default), "active-record", or "service-oriented"
# architecture_mode = "ddd"

[scoring]
# Weights for score components (should sum to 1.0)
layer_isolation_weight = 0.4
dependency_direction_weight = 0.4
interface_coverage_weight = 0.2

[rules]
# Minimum severity to cause failure: "error", "warning", or "info"
fail_on = "error"
# min_score = 70.0   # Optional minimum architecture score
# detect_init_functions = true   # Detect Go init() side effects

[rules.severities]
layer_boundary = "error"
circular_dependency = "error"
missing_port = "warning"
init_coupling = "warning"
```

## Sections

### `[project]`

| Key | Type | Default | Description |
|-----|------|---------|-------------|
| `languages` | list | `[]` (auto-detect) | Languages to analyze. Options: `go`, `rust`, `typescript`, `java` |
| `exclude_patterns` | list | `["vendor/**", "**/*_test.go", "**/testdata/**"]` | Glob patterns for files to skip |
| `services_pattern` | string | _(none)_ | Glob for service directories in monorepos (e.g., `"services/*"`) |

### `[layers]`

Each layer accepts a list of glob patterns. Files matching a pattern are classified into that layer.

| Key | Default Patterns |
|-----|-----------------|
| `domain` | `**/domain/**`, `**/entity/**`, `**/model/**` |
| `application` | `**/application/**`, `**/usecase/**`, `**/service/**` |
| `infrastructure` | `**/infrastructure/**`, `**/adapter/**`, `**/repository/**`, `**/persistence/**` |
| `presentation` | `**/presentation/**`, `**/handler/**`, `**/api/**`, `**/cmd/**` |

Additional fields:

| Key | Type | Description |
|-----|------|-------------|
| `cross_cutting` | list | Paths exempt from layer violation checks |
| `architecture_mode` | string | Global mode: `"ddd"`, `"active-record"`, or `"service-oriented"` |

### `[[layers.overrides]]`

Per-module overrides for layer classification. The first matching `scope` wins.

```toml
[[layers.overrides]]
scope = "services/auth/**"
domain = ["services/auth/core/**"]
infrastructure = ["services/auth/server/**", "services/auth/adapters/**"]
# architecture_mode = "active-record"   # Optional per-module mode
```

Omitted layers fall back to the global patterns.

### `[scoring]`

| Key | Default | Description |
|-----|---------|-------------|
| `layer_isolation_weight` | `0.4` | Weight for layer isolation score |
| `dependency_direction_weight` | `0.4` | Weight for dependency direction score |
| `interface_coverage_weight` | `0.2` | Weight for interface coverage score |

Weights should sum to 1.0.

### `[rules]`

| Key | Type | Default | Description |
|-----|------|---------|-------------|
| `fail_on` | string | `"error"` | Minimum severity to cause non-zero exit |
| `min_score` | float | _(none)_ | Optional minimum overall score |
| `detect_init_functions` | bool | `true` | Detect Go `init()` side-effect coupling |

### `[rules.severities]`

Override the default severity for built-in violation types:

| Violation Type | Default Severity | Description |
|---------------|-----------------|-------------|
| `layer_boundary` | `error` | Inner layer depends on outer layer |
| `circular_dependency` | `error` | Circular dependency between components |
| `missing_port` | `warning` | Adapter without a corresponding port interface |
| `init_coupling` | `warning` | Go `init()` function creates hidden coupling |

### Custom Rules

Define custom dependency rules:

```toml
[[rules.custom_rules]]
name = "no-http-in-domain"
from_pattern = "**/domain/**"
to_pattern = "**/net/http**"
action = "deny"
severity = "error"
message = "Domain layer must not import HTTP packages"
```

| Key | Description |
|-----|-------------|
| `name` | Rule identifier |
| `from_pattern` | Glob for the source of the dependency |
| `to_pattern` | Glob for the target of the dependency |
| `action` | `"deny"` (only option currently) |
| `severity` | `"error"`, `"warning"`, or `"info"` |
| `message` | Custom violation message |
