# Boundary

A static analysis tool for evaluating Domain-Driven Design (DDD) and Hexagonal Architecture patterns in your codebase.

## Why?

Maintaining clean architecture boundaries is challenging as codebases grow. This tool helps teams:

- **Visualize** their architecture layers and dependencies
- **Score** adherence to hexagonal/DDD principles
- **Detect** architectural violations automatically
- **Document** architectural patterns as code

Born from the tedious work of manually documenting a large codebase's architecture, Boundary turns those learnings into automated analysis you can run on every PR.

## Features

- **Multi-language support** - Currently supports Go, with Rust and others planned
- **Architectural scoring** - Quantify layer isolation, dependency flow, and interface coverage
- **Violation detection** - Catch domain→infrastructure dependencies, improper layering, etc.
- **Dependency graphing** - Visualize component relationships with Mermaid/GraphViz
- **Configurable** - Define your own layer patterns and violation rules
- **CI/CD ready** - Fail builds on architectural violations

## Quick Start

```bash
# Install
cargo install boundary

# Analyze a repository
boundary analyze /path/to/repo

# Generate architecture diagram
boundary analyze /path/to/repo --diagram

# Output as JSON for CI integration
boundary check /path/to/repo --format json --fail-on error
```

## Example Output

```
Boundary Analysis Report
========================

Overall Score: 78/100

Layer Isolation:        85/100 ✓
Dependency Direction:   72/100 ⚠
Interface Coverage:     77/100 ⚠

Violations Found: 12

❌ ERROR: Domain layer depends on infrastructure
   File: internal/domain/user/repository.go:15
   Import: github.com/yourorg/yourapp/internal/infrastructure/postgres
   
⚠️  WARNING: Missing interface for adapter
   File: internal/infrastructure/email/smtp.go:23
   Component: SMTPEmailSender has no corresponding port interface

Components Analyzed:
  - 45 Domain Entities
  - 23 Ports (interfaces)
  - 31 Adapters (implementations)
  - 18 Use Cases
  - 12 Repositories
```

## Configuration

Create `.boundary.toml` in your repository root:

```toml
[project]
name = "my-service"
languages = ["go", "rust"]

[layers.domain]
patterns = [
    "**/domain/**",
    "**/model/**",
    "**/entity/**"
]
allowed_imports = [
    "context",
    "time",
    "errors"
]

[layers.application]
patterns = [
    "**/usecase/**",
    "**/application/**",
    "**/service/**"
]

[layers.infrastructure]
patterns = [
    "**/adapter/**",
    "**/infrastructure/**",
    "**/repository/impl/**"
]

[violations]
[violations.domain-infra-dependency]
severity = "error"
message = "Domain layer must not depend on infrastructure"

[violations.missing-port-interface]
severity = "warning"
message = "Adapter should implement a port interface"

[scoring]
# Weights for overall score calculation
layer_isolation_weight = 0.4
dependency_direction_weight = 0.4
interface_coverage_weight = 0.2
```

## What It Detects

### ✅ Good Patterns

- Domain entities free of infrastructure concerns
- Ports (interfaces) defining contracts
- Adapters implementing ports
- Dependencies flowing inward (infrastructure → application → domain)
- Repository pattern usage
- Value objects vs entities

### ❌ Violations

- Domain depending on infrastructure
- Infrastructure concerns leaking into domain (DB annotations, HTTP tags)
- Missing interfaces for external adapters
- Circular dependencies between layers
- Direct database/HTTP client usage in domain/application layers

## How It Works

Boundary uses [tree-sitter](https://tree-sitter.github.io/) to parse source code and build an abstract syntax tree (AST). It then:

1. **Extracts components** - Identifies interfaces, structs, functions, and their relationships
2. **Classifies layers** - Uses path patterns and import analysis to determine architectural layers
3. **Builds dependency graph** - Maps how components depend on each other
4. **Detects violations** - Compares actual dependencies against architectural rules
5. **Calculates scores** - Quantifies architectural health with configurable weights
6. **Generates reports** - Outputs findings in markdown, JSON, or HTML

## Architecture

```
boundary/
├── boundary          # CLI binary
├── boundary-core     # Core types, graph, metrics
├── boundary-go       # Go language analyzer
├── boundary-rust     # Rust language analyzer (planned)
└── boundary-report   # Report generators
```

The tool is designed to be extensible - each language analyzer implements a common trait, making it straightforward to add support for new languages.

## Roadmap

- [ ] Core architecture and scoring engine
- [ ] Go language support
- [ ] Rust language support
- [ ] Java/Kotlin support
- [ ] TypeScript support
- [ ] GitHub Actions integration
- [ ] VS Code extension for inline violations
- [ ] Architecture evolution tracking over time
- [ ] Bounded context analysis
- [ ] Event Storming diagram generation

## Contributing

This project emerged from real-world architectural documentation work. Contributions are welcome!

Areas where help would be appreciated:

- Additional language analyzers
- Better heuristics for layer detection
- Example configurations for common frameworks
- Documentation and examples
- Bug reports and feature requests

See [CONTRIBUTING.md](CONTRIBUTING.md) for details.

## Inspiration

- [cargo-modules](https://github.com/regexident/cargo-modules) - Module structure visualization
- [ArchUnit](https://www.archunit.org/) - Architecture testing for Java
- [NDepend](https://www.ndepend.com/) - .NET architecture analysis
- [Go's import cycle detection](https://golang.org/ref/spec#Import_declarations)

## License

MIT

## FAQ

**Q: How does this differ from dependency checkers like `go mod graph`?**

A: Boundary understands *architectural* layers, not just package dependencies. It knows that `domain → infrastructure` is a violation even if Go allows it.

**Q: Can I use this in CI/CD?**

A: Yes! Use `boundary check --format json --fail-on error` to fail builds on violations.

**Q: Does it work with monorepos?**

A: Yes, you can analyze subdirectories and configure different layer patterns per service.

**Q: I use a different architectural style (Clean Architecture, Onion, etc.). Will this work?**

A: The core concepts are similar. You'll need to configure layer patterns for your specific style, but the violation detection should still be valuable.

---

**Built with 🦀 Rust** | **Powered by 🌳 tree-sitter**
