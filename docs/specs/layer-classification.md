# Layer Classification Specification

**Version:** 0.1
**Status:** Active

---

## Overview

Boundary assigns each source file to exactly one architectural layer — Domain, Application,
Infrastructure, or Presentation — or leaves it unclassified. Classification is driven by glob
patterns configured in `.boundary.toml`.

---

## Pattern Resolution Order

When a file path matches patterns from more than one layer, boundary resolves the conflict
using **first-match-wins** with a fixed priority order:

```
1. Domain
2. Application
3. Infrastructure
4. Presentation
```

The first layer whose pattern set matches the normalized file path wins. Later patterns are not
evaluated.

**Example of a conflict:**

```toml
[layers]
domain         = ["**/modules/**"]   # matches common/modules/billing/infrastructure/repo.go
infrastructure = ["**/infrastructure/**"]   # also matches
```

`common/modules/billing/infrastructure/repo.go` is classified as **Domain**, not
Infrastructure, because domain is evaluated first.

---

## Implications

### Broad patterns shadow more specific ones

A broad glob in a high-priority layer (domain or application) will absorb paths that a more
specific pattern in a lower-priority layer would have caught. Common pitfalls:

| Overly broad pattern | Paths unintentionally absorbed |
|----------------------|-------------------------------|
| `**/modules/**`      | `…/modules/*/infrastructure/` |
| `**/service/**`      | `…/service/adapter/`          |
| `**/core/**`         | `…/core/persistence/`         |

### No specificity-based resolution

Boundary does not resolve conflicts by preferring the most specific (longest) matching
pattern. Only the declared priority order matters. Two patterns that both match the same path
will always resolve to the higher-priority layer, regardless of which pattern is longer or more
precise.

---

## Layer Overrides

For scopes where the global patterns produce wrong results (e.g., a module that uses
"infrastructure" for IaaS resources rather than DDD adapters), use a `[[layers.overrides]]`
block:

```toml
[[layers.overrides]]
scope = "common/modules/projects/**"
domain = [
    "common/modules/projects/infrastructure/**",  # IaaS resources are domain here
    "common/modules/projects/api-keys/**",
]
application = [
    "common/modules/projects/tasks/**",
]
```

Override semantics:

- The `scope` glob determines which files this override applies to (matched against the
  normalized file path).
- Within scope, any layer key that is present in the override **replaces** the global patterns
  for that layer entirely. Layers not listed in the override continue to use global patterns.
- Override matching uses the same first-match-wins order: Domain → Application →
  Infrastructure → Presentation.
- Multiple override blocks are evaluated in declaration order; the first matching scope wins.

---

## Avoiding Pattern Conflicts

### Prefer leaf-level patterns

Patterns that target specific leaf directories are less likely to produce surprises than
patterns that target intermediate directory names:

```toml
# Risky — "modules" appears in many paths
domain = ["**/modules/**"]

# Safe — targets the actual domain subdirectory
domain = ["**/domain/**", "**/entity/**", "**/model/**"]
```

### Use overrides for structural exceptions

When a directory name has project-specific meaning that conflicts with the global conventions
(e.g., `infrastructure/` meaning IaaS rather than adapters), encode the exception as an
override rather than widening a global pattern.

### Audit with `boundary analyze --format json`

Inspect `metrics.components_by_layer` and `metrics.components_by_kind` to verify that
infrastructure adapters are classified as Infrastructure, not Domain. A suspiciously low
`infrastructure` count or zero `interface_coverage` is a signal of pattern misclassification.

---

## Cross-Cutting Concerns

Cross-cutting patterns (`[layers.cross_cutting]`) are applied after layer classification.
A file that matches a cross-cutting pattern is reclassified from its layer assignment to
`CrossCutting`. Cross-cutting classification does not participate in the priority order above
— it acts as a post-filter.

---

## Unclassified Files

A file that matches no layer pattern and no cross-cutting pattern is left **unclassified**.
Unclassified components:

- Are counted in structural presence (as unclassified)
- Do not contribute to layer conformance or dependency compliance scores
- Appear in output under `Unclassified` when non-zero

A high unclassified count indicates that layer patterns do not cover the project's directory
structure and should be expanded.
