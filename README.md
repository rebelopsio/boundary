# Boundary

A static analysis tool for evaluating Domain-Driven Design (DDD) and Hexagonal Architecture patterns in your codebase.

## Why?

Maintaining clean architecture boundaries is challenging as codebases grow. This tool helps teams:

- **Visualize** their architecture layers and dependencies
- **Score** adherence to hexagonal/DDD principles
- **Detect** architectural violations automatically
- **Enforce** boundaries in CI/CD pipelines

Born from the tedious work of manually documenting a large codebase's architecture, Boundary turns those learnings into automated analysis you can run on every PR.

## Installation

```bash
# From source
cargo install --path crates/boundary

# Or build locally
cargo build --release
```

## Quick Start

```bash
# Initialize configuration (optional)
boundary init

# Analyze a repository
boundary analyze /path/to/repo

# Output as JSON
boundary analyze /path/to/repo --format json

# CI/CD check - exits 1 on violations
boundary check /path/to/repo --fail-on error

# JSON output for CI parsing
boundary check /path/to/repo --format json --fail-on error
```

## Configuration

Create `.boundary.toml` in your repository root (or run `boundary init`):

```toml
[project]
languages = ["go"]
exclude_patterns = ["vendor/**", "**/*_test.go", "**/testdata/**"]

[layers]
# Glob patterns to classify files into architectural layers
domain = ["**/domain/**", "**/entity/**", "**/model/**"]
application = ["**/application/**", "**/usecase/**", "**/service/**"]
infrastructure = ["**/infrastructure/**", "**/adapter/**", "**/repository/**", "**/persistence/**"]
presentation = ["**/presentation/**", "**/handler/**", "**/api/**", "**/cmd/**"]

[scoring]
# Weights for score components (should sum to 1.0)
layer_isolation_weight = 0.4
dependency_direction_weight = 0.4
interface_coverage_weight = 0.2

[rules]
# Severity levels: "error", "warning", "info"
fail_on = "error"
# min_score = 70.0

[rules.severities]
layer_boundary = "error"
circular_dependency = "error"
missing_port = "warning"
```

## Output Formats

### Text (default)

```bash
boundary analyze /path/to/repo
```

```
Boundary - Architecture Analysis
========================================

Overall Score: 60.0/100
  Layer Isolation:       50.0/100
  Dependency Direction:  50.0/100
  Interface Coverage:    100.0/100

Summary: 5 components, 3 dependencies

Violations (1 found)
----------------------------------------

  ERROR [domain -> infrastructure] internal/domain/user/bad_dependency.go:3
    domain layer depends on infrastructure layer
    Suggestion: Introduce a port interface in the domain layer.
```

### JSON

```bash
boundary analyze /path/to/repo --format json
```

```json
{
  "score": {
    "overall": 60.0,
    "layer_isolation": 50.0,
    "dependency_direction": 50.0,
    "interface_coverage": 100.0
  },
  "violations": [
    {
      "kind": { "LayerBoundary": { "from_layer": "Domain", "to_layer": "Infrastructure" } },
      "severity": "error",
      "location": { "file": "internal/domain/user/bad_dependency.go", "line": 3, "column": 0 },
      "message": "domain layer depends on infrastructure layer",
      "suggestion": "Introduce a port interface in the domain layer."
    }
  ],
  "component_count": 5,
  "dependency_count": 3
}
```

### JSON Check Output

```bash
boundary check /path/to/repo --format json --fail-on error
```

```json
{
  "score": { "overall": 60.0, "..." : "..." },
  "violations": [ "..." ],
  "component_count": 5,
  "dependency_count": 3,
  "check": {
    "passed": false,
    "fail_on": "error",
    "failing_violation_count": 1
  }
}
```

Use `--compact` for single-line JSON output suitable for log parsing.

## Exit Codes

| Code | Meaning |
|------|---------|
| 0    | Success (analyze) or check passed (check) |
| 1    | Check failed - violations found at or above `--fail-on` severity |
| 2    | Runtime error (invalid path, bad config, etc.) |

## CI/CD Integration

### GitHub Actions

```yaml
name: Architecture Check
on: [pull_request]

jobs:
  boundary:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - name: Install Rust
        uses: dtolnay/rust-toolchain@stable
      - name: Install Boundary
        run: cargo install --path crates/boundary
      - name: Check Architecture
        run: boundary check . --format json --fail-on error
```

See `.github/workflows/boundary.yml` for a working example.

## What It Detects

### Good Patterns

- Domain entities free of infrastructure concerns
- Ports (interfaces) defining contracts
- Adapters implementing ports
- Dependencies flowing inward (infrastructure -> application -> domain)

### Violations

- Domain depending on infrastructure
- Application layer bypassing domain
- Circular dependencies between layers
- Missing interfaces for external adapters

## How It Works

Boundary uses [tree-sitter](https://tree-sitter.github.io/) to parse source code into ASTs. It then:

1. **Extracts components** - Identifies interfaces, structs, and their relationships
2. **Classifies layers** - Uses path patterns to determine architectural layers
3. **Builds dependency graph** - Maps how components depend on each other
4. **Detects violations** - Compares actual dependencies against architectural rules
5. **Calculates scores** - Quantifies architectural health with configurable weights
6. **Generates reports** - Outputs findings in text or JSON format

## Architecture

```
boundary/
├── boundary          # CLI binary
├── boundary-core     # Core types, graph, metrics
├── boundary-go       # Go language analyzer
├── boundary-rust     # Rust language analyzer (planned)
└── boundary-report   # Report generators (text, JSON)
```

Each language analyzer implements the `LanguageAnalyzer` trait, making it straightforward to add support for new languages.

## Features

- **Go language support** - Extracts interfaces, structs, imports via tree-sitter
- **Architectural scoring** - Layer isolation, dependency direction, interface coverage
- **Violation detection** - Layer boundary crossings, circular dependencies
- **JSON output** - Machine-readable output for CI/CD integration
- **Parallel processing** - Uses rayon for fast multi-file analysis
- **Configurable** - Define layer patterns, scoring weights, and violation rules

## Roadmap

- [x] Core architecture and scoring engine
- [x] Go language support
- [x] JSON output format
- [x] CI/CD integration (GitHub Actions)
- [ ] Rust language support
- [ ] Java/Kotlin support
- [ ] TypeScript support
- [ ] VS Code extension
- [ ] Architecture evolution tracking

## Contributing

Contributions are welcome! Areas where help is appreciated:

- Additional language analyzers
- Better heuristics for layer detection
- Example configurations for common frameworks
- Documentation and examples

See [CONTRIBUTING.md](CONTRIBUTING.md) for details.

## Inspiration

- [cargo-modules](https://github.com/regexident/cargo-modules) - Module structure visualization
- [ArchUnit](https://www.archunit.org/) - Architecture testing for Java
- [NDepend](https://www.ndepend.com/) - .NET architecture analysis

## License

MIT
