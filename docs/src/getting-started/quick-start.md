# Quick Start

## 1. Initialize Configuration

In your project root, generate a `.boundary.toml` config file:

```bash
boundary init
```

This creates a `.boundary.toml` with sensible defaults for Go projects. Edit it to match your project structure.

## 2. Run Analysis

Analyze your codebase and see the full architecture report:

```bash
boundary analyze .
```

The output includes:

- Detected components grouped by architectural layer
- Violations with file paths and line numbers
- Architecture scores (0–100%) broken down by structural presence, layer isolation, dependency direction, and interface coverage

## 3. Check in CI

Use `boundary check` to get a pass/fail exit code suitable for CI pipelines:

```bash
boundary check . --fail-on error
```

Exit codes:
- `0` — No violations at or above the failure threshold
- `1` — Violations found

## 4. Track Progress Over Time

Record a snapshot of the current architecture score and prevent regressions from being merged:

```bash
# Record a snapshot
boundary check . --track

# Fail if the score drops below the last recorded snapshot
boundary check . --no-regression

# Do both in one step (typical CI setup)
boundary check . --track --no-regression
```

Snapshots are stored in `.boundary/history.ndjson` relative to the project root. If no snapshot has been recorded yet, `--no-regression` is a no-op.

## 5. Generate Diagrams

Produce architecture diagrams in Mermaid or GraphViz DOT format:

```bash
# Mermaid layer diagram
boundary diagram .

# GraphViz DOT dependency graph
boundary diagram . --diagram-type dot-dependencies
```

## 6. Deep-Dive Forensics

Inspect a specific module for DDD pattern adherence:

```bash
boundary forensics path/to/module
```

This shows per-aggregate analysis, domain event detection, port/adapter mapping, and improvement suggestions.

## Example Output

```
Boundary - Architecture Analysis
========================================

Overall Score: 85%
  Structural Presence: 100%
  Layer Isolation: 80%
  Dependency Direction: 90%
  Interface Coverage: 75%

Summary: 30 components, 12 dependencies

Metrics
----------------------------------------
  Components by layer:
    Application: 8
    Domain: 12
    Infrastructure: 6
    Presentation: 4
  Dependency depth: max=3, avg=1.2

Violations (2 found)
----------------------------------------

  ERROR [domain -> infrastructure] internal/domain/user/repository.go
    Domain layer must not depend on Infrastructure
    Suggestion: Define a port interface in the domain layer and inject the implementation

  WARN [missing port for PaymentAdapter] internal/infrastructure/payment/stripe.go
    Adapter has no corresponding port interface in the domain or application layer
    Suggestion: Add a port interface that this adapter implements

CHECK FAILED: 1 violation(s) at severity error or above
```
