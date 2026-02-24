# Custom Violation Rules

Boundary's built-in rules catch layer boundary violations, circular dependencies, and missing
ports. Custom rules let you enforce additional architectural constraints specific to your
project.

## Defining a Custom Rule

Add one or more `[[rules.custom_rules]]` entries to `.boundary.toml`:

```toml
[[rules.custom_rules]]
name        = "no-domain-external"
from_pattern = ".*domain.*"
to_pattern  = ".*external.*"
action      = "deny"
severity    = "warning"
message     = "Domain must not import external packages"
```

| Field          | Required | Description |
|----------------|----------|-------------|
| `name`         | Yes | Unique identifier shown in violation output |
| `from_pattern` | Yes | Regex matched against the **source** component's path |
| `to_pattern`   | Yes | Regex matched against the **import path** of the dependency |
| `action`       | No | Only `"deny"` is supported (default: `"deny"`) |
| `severity`     | No | `"error"`, `"warning"`, or `"info"` (default: `"error"`) |
| `message`      | No | Custom violation message; a default is generated if omitted |

## How Matching Works

`from_pattern` is matched against the source component's **component ID** — the package path
plus the component name, e.g. `internal/domain/user::<file>`.

`to_pattern` is matched against the **import path** recorded in the dependency edge, e.g.
`github.com/acme/app/external/payments`.

Both patterns are full regular expressions (via the Rust `regex` crate). Use `.*` to match
any path segment.

## Examples

### Prevent domain from importing specific packages

```toml
[[rules.custom_rules]]
name        = "no-http-in-domain"
from_pattern = ".*domain.*"
to_pattern  = ".*/net/http$"
action      = "deny"
severity    = "error"
message     = "Domain layer must not import net/http directly"
```

### Warn when a deprecated package is imported anywhere

```toml
[[rules.custom_rules]]
name        = "no-legacy-client"
from_pattern = ".*"
to_pattern  = ".*/legacy/client.*"
action      = "deny"
severity    = "warning"
message     = "legacy/client is deprecated — use clients/v2 instead"
```

### Multiple rules fire independently

```toml
[[rules.custom_rules]]
name        = "no-domain-db"
from_pattern = ".*domain.*"
to_pattern  = ".*/database.*"
severity    = "error"
message     = "Domain must not import database packages directly"

[[rules.custom_rules]]
name        = "no-domain-redis"
from_pattern = ".*domain.*"
to_pattern  = ".*/redis.*"
severity    = "warning"
message     = "Domain must not import redis packages directly"
```

Each rule is evaluated independently. A single dependency edge can trigger multiple rules if
it matches more than one pattern pair.

## Violation Output

Custom rule violations appear in all output formats alongside built-in violations:

```
WARN [custom: no-domain-external] internal/domain/user/entity.go:4
  Domain must not import external packages
  Suggestion: This dependency is forbidden by custom rule 'no-domain-external'.
```

In JSON output, custom rule violations have `kind.CustomRule.rule_name` set to the rule's
`name` field.

## Severity and `check` Behaviour

Custom rule severity interacts with `boundary check --fail-on` the same way built-in
violations do:

```bash
# Warning-severity custom rules pass a check at the error threshold
boundary check . --fail-on error   # exits 0 if only warnings present

# Lower the threshold to catch warnings too
boundary check . --fail-on warning  # exits 1
```
