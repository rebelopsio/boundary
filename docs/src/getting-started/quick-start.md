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
- Architecture scores (0--100) broken down by layer isolation, dependency direction, and interface coverage

## 3. Check in CI

Use `boundary check` to get a pass/fail exit code suitable for CI pipelines:

```bash
boundary check . --format json --fail-on error
```

Exit codes:
- `0` -- No violations at or above the failure threshold
- `1` -- Violations found

## 4. Generate Diagrams

Produce architecture diagrams in Mermaid or GraphViz DOT format:

```bash
# Mermaid layer diagram
boundary diagram .

# GraphViz DOT dependency graph
boundary diagram . --diagram-type dot-dependencies
```

## 5. Deep-Dive Forensics

Inspect a specific module for DDD pattern adherence:

```bash
boundary forensics path/to/module
```

This shows per-aggregate analysis, domain event detection, port/adapter mapping, and improvement suggestions.

## Example Output

```
Architecture Analysis Report
=============================

Layers:
  Domain:          12 components
  Application:      8 components
  Infrastructure:   6 components
  Presentation:     4 components

Violations (2):
  ERROR: Domain depends on Infrastructure
    File: internal/domain/user/repository.go:15
    Import: github.com/app/internal/infrastructure/postgres

Scores:
  Layer Isolation:       85/100
  Dependency Direction:  90/100
  Interface Coverage:    75/100
  Overall:               85/100
```
