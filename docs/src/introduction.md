# Introduction

Boundary is a static analysis tool that validates architectural boundaries in codebases following Domain-Driven Design (DDD) and Hexagonal Architecture patterns. It automatically detects architectural violations, scores adherence to architectural principles, and generates visual documentation of system boundaries and dependencies.

## Why Boundary?

Architectural rules often live in wikis or team knowledge but aren't enforced in code. Over time, boundaries erode: domain logic leaks into infrastructure, adapters skip port interfaces, and layers become tightly coupled. Manual code review catches some of these issues, but not at scale.

Boundary solves this by:

- **Detecting violations automatically** -- Catch domain-to-infrastructure dependencies before they reach production
- **Quantifying architectural health** -- Objective scores for layer isolation and dependency flow
- **Generating documentation** -- Up-to-date architecture diagrams generated from code
- **Integrating with CI/CD** -- Fail builds on critical violations

## Supported Languages

Boundary uses [tree-sitter](https://tree-sitter.github.io/tree-sitter/) for multi-language AST parsing:

- Go
- Rust
- TypeScript / TSX
- Java

## How It Works

The analysis pipeline follows these steps:

1. **Parse** -- Build ASTs for each source file using tree-sitter
2. **Extract** -- Identify components (interfaces, structs, imports, dependencies)
3. **Classify** -- Assign components to architectural layers (Domain, Application, Infrastructure, Presentation)
4. **Build Graph** -- Construct a dependency graph with layer metadata using [petgraph](https://docs.rs/petgraph)
5. **Analyze** -- Detect violations and calculate scores
6. **Report** -- Output results as text, JSON, Markdown, or diagrams

## Architecture

```
boundary (CLI)
├── boundary-core    -- Analyzer trait, graph types, scoring, violations
├── boundary-go      -- Go language analyzer
├── boundary-rust    -- Rust language analyzer
├── boundary-typescript -- TypeScript/TSX analyzer
├── boundary-java    -- Java language analyzer
├── boundary-report  -- Report generation (text, markdown, mermaid, DOT)
└── boundary-lsp     -- LSP server for editor integration
```
