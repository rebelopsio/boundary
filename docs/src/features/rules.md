# Rules & Rule IDs

Every violation Boundary reports carries a **rule ID** — a short, stable identifier like
`L001` or `PA001`. Rule IDs let you selectively suppress false positives, filter output, and
(in the future) configure severity per rule.

## Rule Catalog

### Layer Violations (`L`)

| ID | Name | Description | Severity |
|----|------|-------------|----------|
| L001 | domain-depends-on-infrastructure | Domain layer imports directly from infrastructure | Error |
| L002 | domain-depends-on-application | Domain layer depends on application orchestration | Error |
| L003 | application-bypasses-ports | Application layer calls infrastructure without a port | Error |
| L004 | init-function-coupling | Init/main wiring function couples layers directly | Warning |
| L005 | domain-uses-infrastructure-type | Domain code references an infrastructure type | Error |
| L099 | layer-boundary-violation | Catch-all for other forbidden layer crossings | Error |

### Dependency Violations (`D`)

| ID | Name | Description | Severity |
|----|------|-------------|----------|
| D001 | circular-dependency | Circular dependency detected between components | Error |

### Port/Adapter Violations (`PA`)

| ID | Name | Description | Severity |
|----|------|-------------|----------|
| PA001 | missing-port-interface | Infrastructure adapter has no matching domain port | Warning |

### Custom Rules (`C-`)

Custom rules defined in `.boundary.toml` receive IDs prefixed with `C-` followed by the rule
name. For example, a rule named `no-logging-in-domain` gets the ID `C-no-logging-in-domain`.

See [Custom Rules](./custom-rules.md) for how to define them.

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
