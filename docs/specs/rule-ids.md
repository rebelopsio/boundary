# Rule ID Specification

## Overview

Every violation emitted by Boundary carries a **rule ID** — a short, stable identifier that
uniquely names the rule that was violated. Rule IDs enable:

- Selective suppression of false positives via `--ignore`
- CI/CD configuration per rule (Phase 2)
- Trend tracking of specific violation types over time (Phase 2)

## ID Format

Rule IDs follow the pattern `{prefix}{number}`:

| Prefix | Category | Examples |
|--------|----------|----------|
| `L` | Layer boundary violations | L001, L002 |
| `D` | Dependency graph violations | D001 |
| `PA` | Port/adapter violations | PA001 |
| `C-` | Custom user-defined rules | C-no-logging-in-domain |

Numbers are zero-padded to 3 digits. Custom rules use a hyphenated name instead of a number.

## Rule Mapping (Phase 1)

These rules map to existing `ViolationKind` variants — no new detection logic.

| Rule ID | Name | ViolationKind | Default Severity |
|---------|------|---------------|------------------|
| L001 | domain-depends-on-infrastructure | `LayerBoundary { Domain, Infrastructure }` | Error |
| L002 | domain-depends-on-application | `LayerBoundary { Domain, Application }` | Error |
| L003 | application-bypasses-ports | `LayerBoundary { Application, Infrastructure }` | Error |
| L004 | init-function-coupling | `InitFunctionCoupling` | Warning |
| L005 | domain-uses-infrastructure-type | `DomainInfrastructureLeak` | Error |
| L099 | layer-boundary-violation | `LayerBoundary { other combos }` | Error |
| D001 | circular-dependency | `CircularDependency` | Error |
| PA001 | missing-port-interface | `MissingPort` | Warning |
| PA003 | constructor-returns-concrete-type | `ConstructorReturnsConcrete` | Warning |
| C-{name} | {name} | `CustomRule { name }` | (user-defined) |

### Layer Boundary Specialization

`LayerBoundary` violations are assigned specific IDs based on the `from_layer` → `to_layer`
pair:

- **Domain → Infrastructure** (L001): The most critical violation — domain logic depends
  directly on infrastructure.
- **Domain → Application** (L002): Domain should not depend on application orchestration.
- **Application → Infrastructure** (L003): Application layer should use port interfaces, not
  call infrastructure directly.
- **All other combinations** (L099): Catch-all for less common layer violations (e.g.,
  Domain → Presentation).

## CLI Usage (Phase 1)

```bash
# Suppress specific rules
boundary analyze . --ignore PA001
boundary analyze . --ignore PA001,L005

# Works with both analyze and check
boundary check . --ignore PA001
```

The `--ignore` flag accepts a comma-separated list of rule IDs. Ignored violations are removed
before output formatting and before the `check` pass/fail decision.

## Output Format

### Text

```
  L001 ERROR [domain-depends-on-infrastructure] domain/user.go:10
    Domain component imports infrastructure package
    Suggestion: Define a port interface in the domain layer
```

### JSON

Each violation object includes `rule` and `rule_name` fields alongside existing fields:

```json
{
  "rule": "L001",
  "rule_name": "domain-depends-on-infrastructure",
  "kind": { "LayerBoundary": { "from_layer": "domain", "to_layer": "infrastructure" } },
  "severity": "error",
  "location": { "file": "domain/user.go", "line": 10, "column": 1 },
  "message": "Domain component imports infrastructure package",
  "suggestion": "Define a port interface in the domain layer"
}
```

### Markdown

```markdown
| Rule | Severity | Name | Location | Message |
|------|----------|------|----------|---------|
| L001 | ERROR | domain-depends-on-infrastructure | domain/user.go:10 | ... |
```

## Phase 2 — Config-based Rule Configuration

### Severity Overrides in `[rules.severities]`

Both **category names** (legacy) and **rule IDs** are accepted as keys:

```toml
[rules.severities]
# Category names (backward compatible)
layer_boundary = "error"
missing_port = "warning"
domain_infra_leak = "error"      # NEW — was hardcoded to Error before Phase 2
init_coupling = "warning"

# Rule IDs (more precise, takes precedence over category names)
L001 = "error"
PA001 = "info"
```

**Precedence:** rule ID (e.g. `PA001`) > category name (e.g. `missing_port`) > built-in default.

#### Category Name Mapping

| Category Name | Violation Kinds |
|---------------|----------------|
| `layer_boundary` | `LayerBoundary` (all `from_layer`/`to_layer` combos) |
| `circular_dependency` | `CircularDependency` |
| `missing_port` | `MissingPort` |
| `constructor_concrete` | `ConstructorReturnsConcrete` |
| `init_coupling` | `InitFunctionCoupling` |
| `domain_infra_leak` | `DomainInfrastructureLeak` |

### Path-specific Ignores `[[rules.ignore]]`

Suppress specific rules for files matching glob patterns:

```toml
[[rules.ignore]]
rule = "PA001"
paths = ["infrastructure/**/*document.go"]

[[rules.ignore]]
rule = "L005"
paths = ["legacy/**"]
```

| Field | Type | Description |
|-------|------|-------------|
| `rule` | string | Rule ID to suppress (e.g. `PA001`, `L001`, `C-my-rule`) |
| `paths` | list of strings | Glob patterns; violation is suppressed if the file matches any |

Path-specific ignores are applied uniformly — they filter violations in CLI output, library
API, and (future) LSP integration.

## Phase 3 (Future)

- Documentation URLs on violations
- New detection rules: PA002 (port-without-implementation)
- PA003 (constructor-returns-concrete-type) — implemented in Phase 3
- Historical tracking / violation trend comparison
