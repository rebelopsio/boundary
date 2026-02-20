# Boundary Scoring Specification

**Version:** 0.1
**Status:** Draft

---

## Philosophy

Scoring in boundary is **objective, deterministic, and language-agnostic**. Given the same
dependency graph, the same score is always produced. There are no subjective thresholds tuned
by feel — every number derives from a defined formula. Scores describe *what is*, not *what
should be*; architectural intent is captured through pattern detection with confidence, not
through penalizing the absence of patterns the tool assumes you intended.

---

## Definitions

**Real component** — A named type extracted from source code (struct, interface, class, trait).
Synthetic graph nodes created for dependency tracking (`<file>`, `<package>`) are not real
components and are excluded from all scoring.

**Abstract type** — A type that defines a contract without providing an implementation. Per
language: `interface` in Go, `trait` in Rust, `interface`/`abstract class` in Java/TypeScript.

**Concrete type** — A type that provides an implementation. Per language: `struct` in Go,
`struct` (non-trait) in Rust, `class` in Java/TypeScript.

**Package** — The smallest unit of organization above a file. In Go: a Go package. In Rust: a
module. In Java: a Java package. In TypeScript: a directory.

**Internal dependency** — An import between two packages within the analyzed module.
Third-party library imports are external dependencies and are excluded from all coupling
calculations.

**Layer assignment** — The architectural layer (Domain, Application, Infrastructure,
Presentation) assigned to a component based on its file path and any configured overrides in
`.boundary.toml`.

---

## Core Metrics (per package)

These are Robert C. Martin's package design metrics from *Clean Architecture*, applied per
package within the analyzed module.

### Instability (I)

```
I = Ce / (Ca + Ce)

Ce = efferent coupling: count of distinct internal packages this package imports
Ca = afferent coupling: count of distinct internal packages that import this package

Range:        0.0 (maximally stable) → 1.0 (maximally unstable)
Special case: Ca + Ce = 0 → I = 0
```

### Abstractness (A)

```
A = Na / Nc

Na = count of abstract types in this package
Nc = count of total real components (abstract + concrete) in this package

Range:        0.0 (fully concrete) → 1.0 (fully abstract)
Special case: Nc = 0 → package excluded from all scoring
```

### Distance from Main Sequence (D)

```
D = |A + I - 1|

Range: 0.0 (on the main sequence) → 1.0 (maximally off)
```

The **main sequence** (`A + I = 1`) represents the ideal balance between abstractness and
stability. Packages should be either abstract-and-stable or concrete-and-unstable.

The two failure zones:

- **Zone of Pain** (D near 1, I near 0, A near 0): concrete and stable — rigid, hard to
  change, accumulates dependents
- **Zone of Uselessness** (D near 1, I near 1, A near 1): abstract and unstable — interfaces
  that nothing depends on

---

## Expected Layer Regions

Each DDD layer has an expected (A, I) region on the main sequence. A package's layer
conformance score measures how close its actual (A, I) values fall to its assigned layer's
expected region.

| Layer          | Expected A | Expected I | Rationale                                              |
|----------------|------------|------------|--------------------------------------------------------|
| Domain         | ≥ 0.5      | ≤ 0.3      | Defines ports (abstract), few outgoing deps (stable)   |
| Application    | 0.2 – 0.6  | 0.3 – 0.7  | Orchestrates via ports, depends on domain              |
| Infrastructure | ≤ 0.3      | ≥ 0.5      | Concrete implementations, depends on domain + externals|
| Presentation   | ≤ 0.3      | ≥ 0.5      | Concrete handlers, depends on application              |

**Layer conformance score (per package):**

```
conformance = 1.0 - distance((A, I), expected_region_centroid)
clamped to [0.0, 1.0]
```

---

## Pattern Detection

Before scoring, boundary identifies which architectural pattern the module most closely
matches. This is reported as a **confidence distribution** — a module may show meaningful
confidence in multiple patterns simultaneously, which indicates a codebase in transition.

### Pattern Fingerprints

**DDD + Hexagonal Architecture**
Signals: distinct domain, application, and infrastructure packages exist; domain has high A and
low I; infrastructure implements domain interfaces; no inward dependency violations.

**Active Record**
Signals: domain types directly contain persistence annotations (BSON/SQL tags or ORM field
markers); no repository interfaces; no distinct layers; A ≈ 0 throughout.

**Flat CRUD**
Signals: no layered directory structure; all types in one or two packages; A ≈ 0; no abstract
types anywhere.

**Anemic Domain**
Signals: domain package exists but A ≈ 0 (all structs, no interfaces); business logic
concentrated in service packages; no ports defined in domain.

**Service Layer**
Signals: some separation exists but no ports or adapters; services depend directly on data
access types; partial abstraction without clear hexagonal boundaries.

### Confidence Calculation

Confidence for each pattern is a weighted match against its fingerprint signals, producing a
value in [0.0, 1.0] per pattern. The values across all patterns are independent and do not sum
to 1.0 — a module in active refactoring may show 0.7 DDD and 0.6 Anemic Domain simultaneously.

**When top pattern confidence < 0.5:** Report discovery output only. Do not compute or display
DDD scores. Inform the user that no pattern was detected with sufficient confidence and suggest
next steps.

**When top pattern confidence ≥ 0.5:** Compute all applicable score dimensions and report
alongside the confidence distribution.

---

## Score Dimensions

### 1. Structural Presence

*What fraction of real components have been assigned to an architectural layer?*

```
presence = (classified + cross_cutting) / total_real_components

classified    = real components with a layer assignment
cross_cutting = real components marked as cross-cutting concerns
total         = all real components (excludes synthetic nodes, excludes external)

Range:        0.0 – 1.0, reported as a percentage
Special case: total = 0 → presence = 0.0 (not 1.0)
```

### 2. Layer Conformance

*How well do the actual (A, I) values of each package match its assigned layer's expected
region?*

```
conformance = mean(conformance_score) over all classified packages with Nc ≥ 1

Range:        0.0 – 1.0, reported as a percentage
Special case: no classified packages → conformance = undefined (not reported)
```

### 3. Dependency Rule Compliance

*What fraction of cross-layer imports flow in the correct direction?*

```
compliance = correct_edges / total_cross_layer_edges

correct_edges           = edges where both endpoints are internal, non-external,
                          non-cross-cutting, classified, and the direction does not
                          violate the layer ordering rule
total_cross_layer_edges = all edges between differently-classified, internal,
                          non-cross-cutting components

Layer ordering (no violations):
  Domain ← Application ← Infrastructure ← Presentation
  Domain ← Infrastructure  (ports → adapters)

Range:        0.0 – 1.0, reported as a percentage
Special case: total_cross_layer_edges = 0 → compliance = undefined (not reported)
```

### 4. Interface Coverage

*Are infrastructure adapters backed by domain port interfaces?*

```
coverage = min(ports, adapters) / max(ports, adapters)

ports    = real components of kind Port assigned to the Domain layer
adapters = real components of kind Adapter or Repository assigned to the
           Infrastructure layer

Range:        0.0 – 1.0, reported as a percentage
Special case: adapters = 0 → coverage = undefined (not reported)
Special case: ports = 0 and adapters > 0 → coverage = 0.0
```

---

## Overall Score

The overall score is only computed when:
- Pattern confidence ≥ 0.5 for at least one pattern, **and**
- Structural presence > 0.0

```
overall = presence × weighted_correctness / 100

weighted_correctness = (w1 × conformance + w2 × compliance + w3 × coverage)
                       computed only over defined dimensions, weights redistributed
                       proportionally when a dimension is undefined

Default weights (configurable per module in .boundary.toml):
  w1  layer conformance       0.40
  w2  dependency compliance   0.40
  w3  interface coverage      0.20

Constraint: weights must sum to 1.0
```

When the overall score cannot be computed (confidence < 0.5 or presence = 0), boundary reports
the pattern confidence distribution and structural presence only.

---

## Output Format

Score dimensions are displayed in the terminal using the following format:

```
Structural Presence:      100%
Layer Conformance:         85%
Dependency Compliance:     72%
Interface Coverage:        50%

Overall Score:             78%
```

Rules:
- Labels use title case followed by a colon
- Values are always whole percentages (rounded to nearest integer)
- Undefined dimensions are omitted entirely — they are not shown as `N/A` or `0%`
- When overall score cannot be computed, it is omitted and a reason is stated instead

---

## What Is Not Scored

- **External dependencies** — third-party library imports are excluded from all coupling
  calculations
- **Synthetic graph nodes** — `<file>` and `<package>` placeholder nodes are excluded from all
  component counts
- **Cross-cutting concerns** — excluded from layer conformance and dependency compliance;
  included in structural presence
- **Undetected patterns** — when top pattern confidence < 0.5, DDD dimensions are not computed

---

## Language Adaptations

| Concept       | Go              | Rust                    | Java                              | TypeScript                  |
|---------------|-----------------|-------------------------|-----------------------------------|-----------------------------|
| Abstract type | `interface`     | `trait`                 | `interface`, `abstract class`     | `interface`, `abstract class` |
| Concrete type | `struct`        | `struct` (non-trait)    | `class` (non-abstract)            | `class` (non-abstract)      |
| Package unit  | Go package      | Rust module             | Java package                      | Directory                   |
| Port signal   | interface in domain layer | trait in domain module | interface in domain package | interface in domain dir |
