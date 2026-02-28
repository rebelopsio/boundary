# Reports

Boundary produces reports in three formats: plain text (default), JSON, and Markdown.

```bash
boundary analyze . --format text      # default — coloured terminal output
boundary analyze . --format json      # machine-readable
boundary analyze . --format markdown  # suitable for wikis and PR comments
```

---

## Markdown Format

The Markdown report is designed to be pasted into GitHub PR descriptions, wikis, Confluence
pages, or any Markdown renderer. Every section that has data is rendered; sections with no
data are omitted.

```bash
boundary analyze . --format markdown
boundary analyze . --format markdown > architecture.md
```

### Sections

#### Scores

Overall architecture score and each sub-dimension, rendered as a table.

```markdown
## Scores

| Metric                  | Score        |
|-------------------------|--------------|
| **Overall**             | **78.0/100** |
| Structural Presence     | 100.0/100    |
| Layer Conformance       | 85.0/100     |
| Dependency Compliance   | 72.0/100     |
| Interface Coverage      | 60.0/100     |
```

#### Summary

Total component and dependency counts.

#### Metrics

Components by layer, components by kind, dependency depth, and classification coverage.

#### Package Metrics

Robert C. Martin's package-level coupling metrics — Instability (I), Abstractness (A), and
Distance from the main sequence (D) — for each package in the project.

```markdown
## Package Metrics

| Package        | A    | I    | D    | Zone        |
|----------------|------|------|------|-------------|
| domain         | 0.50 | 0.00 | 0.50 | —           |
| application    | 0.00 | 1.00 | 0.00 | —           |
| infrastructure | 0.00 | 1.00 | 0.00 | —           |
| common         | 0.00 | 0.00 | 1.00 | ⚠ Pain      |
```

The **Zone** column is populated when a package is far from the main sequence (D > 0.5):

| Zone            | Condition              | Meaning |
|-----------------|------------------------|---------|
| ⚠ Pain         | A < 0.5 and I < 0.5    | Concrete and stable — rigid, hard to change |
| ⚠ Uselessness  | A > 0.5 and I > 0.5    | Abstract and unstable — unused abstractions |
| —               | otherwise              | On or near the main sequence |

See [scoring concepts](../getting-started/quick-start.md) for the full metric definitions.

#### Pattern Detection

The detected architectural pattern and confidence scores for all five patterns.

```markdown
## Pattern Detection

Top Pattern: **ddd-hexagonal** (78% confidence)

| Pattern        | Confidence |
|----------------|------------|
| ddd-hexagonal  | 78%        |
| service-layer  | 35%        |
| anemic-domain  | 20%        |
| flat-crud      | 5%         |
| active-record  | 0%         |
```

Confidence values are independent — they do not sum to 100%. A codebase in transition may
show meaningful confidence for multiple patterns simultaneously.

#### Violations

All violations in a table, with rule ID, severity, rule name, location, and message.

```markdown
| Rule | Severity | Name | Location | Message |
|------|----------|------|----------|---------|
| L001 | ERROR | domain-depends-on-infrastructure | domain/user.go:10 | Domain depends on infra |
| PA001 | WARN | missing-port-interface | infrastructure/repo.go:5 | No matching port |
```

See [Rules & Rule IDs](./rules.md) for the full rule catalog.

---

## JSON Format

JSON output includes every field, suitable for programmatic processing, dashboards, or saving
snapshots.

```bash
boundary analyze . --format json | jq '.score.overall'
boundary analyze . --format json | jq '.violations[] | select(.severity == "error")'
boundary analyze . --format json | jq '.package_metrics[] | select(.zone != null)'
```

Top-level fields in the JSON output:

| Field               | Description |
|---------------------|-------------|
| `score`             | Architecture score dimensions (omitted if pattern confidence < 0.5) |
| `violations`        | Array of all violations |
| `component_count`   | Total number of real components |
| `dependency_count`  | Total number of dependency edges |
| `files_analyzed`    | Number of source files analyzed |
| `metrics`           | Detailed metrics breakdown |
| `package_metrics`   | Array of per-package A/I/D metrics |
| `pattern_detection` | Pattern confidence distribution |

Each violation object includes:

| Field       | Description |
|-------------|-------------|
| `rule`      | Stable rule ID (e.g. `L001`, `PA001`, `D001`) |
| `rule_name` | Human-readable rule name (e.g. `domain-depends-on-infrastructure`) |
| `kind`      | Violation kind with structured details |
| `severity`  | `error`, `warning`, or `info` |
| `location`  | File path, line, and column |
| `message`   | Human-readable description |
| `suggestion`| Fix suggestion (when available) |

Filter violations by rule ID with `jq`:

```bash
boundary analyze . --format json | jq '.violations[] | select(.rule == "L001")'
```

---

## Text Format

The default terminal output with colour highlighting. Designed for developer workflows and CI
log readability.

```bash
boundary analyze .         # coloured output
boundary analyze . --compact  # single-line JSON, no colour (useful for piping)
```
