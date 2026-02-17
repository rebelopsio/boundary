# CLI Reference

## Global Options

```
boundary [COMMAND]

Options:
  -h, --help     Print help
  -V, --version  Print version
```

## Commands

### `boundary analyze`

Analyze a codebase and print a full architecture report.

```
boundary analyze [OPTIONS] <PATH>

Arguments:
  <PATH>  Path to the project root

Options:
  -c, --config <CONFIG>        Config file path (defaults to .boundary.toml in project root)
      --format <FORMAT>        Output format [default: text] [possible values: text, json, markdown]
      --compact                Compact output (single-line JSON, no colors for text)
      --languages <LANGUAGES>  Languages to analyze (auto-detect if not specified)
      --incremental            Use incremental analysis (cache unchanged files)
      --per-service            Analyze each service independently (monorepo support)
```

**Examples:**

```bash
# Analyze current directory
boundary analyze .

# JSON output for a specific project
boundary analyze /path/to/project --format json

# Analyze only Go files with incremental caching
boundary analyze . --languages go --incremental

# Per-service monorepo analysis
boundary analyze . --per-service
```

---

### `boundary check`

Analyze and exit with code 0 (pass) or 1 (fail). Designed for CI pipelines.

```
boundary check [OPTIONS] <PATH>

Arguments:
  <PATH>  Path to the project root

Options:
      --fail-on <FAIL_ON>      Minimum severity to cause failure [default: error]
  -c, --config <CONFIG>        Config file path
      --format <FORMAT>        Output format [default: text] [possible values: text, json, markdown]
      --compact                Compact output (single-line JSON, no colors for text)
      --languages <LANGUAGES>  Languages to analyze (auto-detect if not specified)
      --track                  Save analysis snapshot for evolution tracking
      --no-regression          Fail if architecture score regresses from last snapshot
      --incremental            Use incremental analysis (cache unchanged files)
      --per-service            Analyze each service independently (monorepo support)
```

**Examples:**

```bash
# CI check with JSON output
boundary check . --format json --fail-on error

# Track architecture evolution
boundary check . --track --no-regression
```

---

### `boundary init`

Create a default `.boundary.toml` configuration file in the current directory.

```
boundary init [OPTIONS]

Options:
      --force  Overwrite existing config
```

**Examples:**

```bash
# Create config (fails if .boundary.toml already exists)
boundary init

# Overwrite existing config
boundary init --force
```

---

### `boundary diagram`

Generate an architecture diagram in Mermaid or GraphViz DOT format.

```
boundary diagram [OPTIONS] <PATH>

Arguments:
  <PATH>  Path to the project root

Options:
  -c, --config <CONFIG>              Config file path
      --diagram-type <DIAGRAM_TYPE>  Diagram type [default: layers]
                                     [possible values: layers, dependencies, dot, dot-dependencies]
      --languages <LANGUAGES>        Languages to analyze (auto-detect if not specified)
```

**Diagram types:**

| Type | Format | Description |
|------|--------|-------------|
| `layers` | Mermaid | Layer-grouped component diagram |
| `dependencies` | Mermaid | Component dependency graph |
| `dot` | GraphViz DOT | Layer diagram in DOT format |
| `dot-dependencies` | GraphViz DOT | Dependency graph in DOT format |

**Examples:**

```bash
# Mermaid layer diagram
boundary diagram .

# GraphViz DOT dependency graph, save to file
boundary diagram . --diagram-type dot-dependencies > architecture.dot
```

---

### `boundary forensics`

Generate a detailed forensics report for a specific module with DDD pattern analysis.

```
boundary forensics [OPTIONS] <PATH>

Arguments:
  <PATH>  Path to the module directory

Options:
      --project-root <PROJECT_ROOT>  Project root (auto-detected if not specified)
  -c, --config <CONFIG>              Config file path
      --languages <LANGUAGES>        Languages to analyze (auto-detect if not specified)
  -o, --output <OUTPUT>              Write output to file instead of stdout
```

The forensics report includes:

- Per-aggregate analysis with fields and method signatures
- Domain event detection (structs ending with `Event`)
- Value object heuristics (structs without identity fields)
- Import classification (stdlib, internal, external)
- Dependency audit with infrastructure leak detection
- Port/adapter mapping with interface coverage
- Improvement suggestions (anemic models, missing events, unmatched ports)

**Examples:**

```bash
# Analyze a specific module
boundary forensics internal/domain/billing

# Save report to markdown file
boundary forensics internal/domain/billing -o report.md

# Specify project root explicitly
boundary forensics services/auth/core --project-root /path/to/monorepo
```
