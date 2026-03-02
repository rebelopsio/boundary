# Rules & Rule IDs

Every violation Boundary reports carries a **rule ID** — a short, stable identifier like
`L001` or `PA001`. Rule IDs let you selectively suppress false positives, filter output, and
(in the future) configure severity per rule.

## Rule Catalog

### Layer Violations (`L`)

| ID | Name | Description | Severity |
|----|------|-------------|----------|
| <a id="l001"></a>L001 | domain-depends-on-infrastructure | Domain layer imports directly from infrastructure | Error |
| <a id="l002"></a>L002 | domain-depends-on-application | Domain layer depends on application orchestration | Error |
| <a id="l003"></a>L003 | application-bypasses-ports | Application layer calls infrastructure without a port | Error |
| <a id="l004"></a>L004 | init-function-coupling | Init/main wiring function couples layers directly | Warning |
| <a id="l005"></a>L005 | domain-uses-infrastructure-type | Domain code references an infrastructure type | Error |
| <a id="l099"></a>L099 | layer-boundary-violation | Catch-all for other forbidden layer crossings | Error |

### Dependency Violations (`D`)

| ID | Name | Description | Severity |
|----|------|-------------|----------|
| <a id="d001"></a>D001 | circular-dependency | Circular dependency detected between components | Error |

### Port/Adapter Violations (`PA`)

| ID | Name | Description | Severity |
|----|------|-------------|----------|
| <a id="pa001"></a>PA001 | missing-port-interface | Infrastructure adapter has no matching domain port | Warning |
| <a id="pa002"></a>PA002 | port-without-implementation | Domain port has no infrastructure adapter implementing it | Info |
| <a id="pa003"></a>PA003 | constructor-returns-concrete-type | Constructor returns concrete type instead of port interface | Warning |

#### PA003: constructor-returns-concrete-type

Detects constructors in the infrastructure layer that return a concrete struct pointer instead
of a port interface. This is a Dependency Inversion Principle violation — callers become coupled
to the concrete implementation rather than depending on an abstraction.

**Violation:**
```go
// infrastructure/mailgun/service.go
func NewMailGunService(apiKey string) *MailGunService {
    return &MailGunService{apiKey: apiKey}
}
```

**Fix:** Return the port interface instead:
```go
// infrastructure/mailgun/service.go
func NewMailGunService(apiKey string) ports.NotificationService {
    return &MailGunService{apiKey: apiKey}
}
```

When PA003 fires, PA001 (missing-port-interface) is suppressed for the same adapter since PA003
provides more specific guidance.

#### PA002: port-without-implementation

Detects domain-layer port interfaces that have no matching infrastructure adapter. This helps
identify ports that may have been defined but never implemented, or whose adapter was removed.

Default severity is **Info** because unimplemented ports may be planned, implemented in a
separate module, or defined as part of an interface-first design approach.

**Violation:**
```go
// domain/ports/audit.go
type AuditLogger interface {
    Log(event string) error
}
// No adapter implementing AuditLogger exists in the infrastructure layer
```

**Fix:** Create an infrastructure adapter:
```go
// infrastructure/logging/audit.go
type fileAuditLogger struct { path string }

func NewFileAuditLogger(path string) ports.AuditLogger {
    return &fileAuditLogger{path: path}
}
```

PA002 checks both explicit `implements` relationships (from constructor analysis) and
name-heuristic matching (same logic as PA001, inverted).

### Custom Rules (`C-`)

Custom rules defined in `.boundary.toml` receive IDs prefixed with `C-` followed by the rule
name. For example, a rule named `no-logging-in-domain` gets the ID `C-no-logging-in-domain`.

See [Custom Rules](./custom-rules.md) for how to define them.

## Configuration

### Severity Overrides

Override the default severity for any rule using its **rule ID** or **category name** in
`[rules.severities]`:

```toml
[rules.severities]
# Category names (backward compatible)
layer_boundary = "error"
missing_port = "warning"
domain_infra_leak = "error"

# Rule IDs take precedence over category names
PA001 = "info"
L001 = "warning"
```

When both a rule ID and category name are configured, the rule ID wins. This lets you set a
baseline per category and override individual rules.

### Path-specific Ignores

Suppress specific rules for files matching glob patterns:

```toml
[[rules.ignore]]
rule = "PA001"
paths = ["infrastructure/**/*document.go"]

[[rules.ignore]]
rule = "L005"
paths = ["legacy/**"]
```

Unlike `--ignore` (which suppresses a rule globally), path-specific ignores only suppress
violations in files matching the glob patterns. This is useful when certain areas of the
codebase intentionally diverge from the architecture (e.g., legacy modules undergoing
migration).

## Ignoring Rules

Use `--ignore` to suppress specific rules by ID. This is useful for false positives or rules
that don't apply to your codebase.

```bash
# Ignore a single rule
boundary analyze . --ignore PA001

# Ignore multiple rules (comma-separated)
boundary analyze . --ignore PA001,L005

# Works with check too — ignored violations don't affect the exit code
boundary check . --ignore PA001
```

Ignored violations are removed before output formatting and before the `check` pass/fail
decision.

## Output Format

Rule IDs appear in all output formats.

### Text

```
  L001 ERROR [domain-depends-on-infrastructure] domain/user.go:10
    Domain component imports infrastructure package
    Suggestion: Define a port interface in the domain layer
```

### JSON

Each violation includes `rule` and `rule_name` fields:

```json
{
  "rule": "L001",
  "rule_name": "domain-depends-on-infrastructure",
  "kind": { "LayerBoundary": { "from_layer": "Domain", "to_layer": "Infrastructure" } },
  "severity": "error",
  "location": { "file": "domain/user.go", "line": 10, "column": 1 },
  "message": "Domain component imports infrastructure package"
}
```

Filter by rule ID with `jq`:

```bash
# Show only L001 violations
boundary analyze . --format json | jq '.violations[] | select(.rule == "L001")'

# Count PA001 occurrences
boundary analyze . --format json | jq '[.violations[] | select(.rule == "PA001")] | length'
```

### Markdown

```markdown
| Rule | Severity | Name | Location | Message |
|------|----------|------|----------|---------|
| L001 | ERROR | domain-depends-on-infrastructure | domain/user.go:10 | ... |
```
