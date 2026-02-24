# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this
repository.

## Project Overview

Boundary is a static analysis tool that helps engineers understand, document, and refactor
codebases toward DDD + Hexagonal Architecture. It uses tree-sitter for multi-language AST
parsing, builds dependency graphs with petgraph, and scores architectural patterns using
objective, deterministic metrics. Currently in active development with Go as the first
supported language.

## Git Workflow

**Always work on a feature branch. Never commit or push directly to `main`.**

1. Before making any changes, create a branch: `git checkout -b <type>/<short-description>`
2. Use conventional commit prefixes: `feat/`, `fix/`, `chore/`, `docs/`, `test/`, `ci/`
3. Commit on the branch, then ask the user before pushing
4. Never run `git push origin main` or any force-push
5. If already on `main` when work begins, create a branch immediately before touching any file

This applies to all changes — code, docs, config, and workflow files without exception.

## Development Workflow

**BDD → ATDD → TDD → Red → Green → Refactor. Atomic changes only.**

1. Write or update a Gherkin feature file in `docs/features/` → run `bdd-expert` agent
2. Write a failing acceptance test that maps to the scenario
3. Write failing unit tests → implement → run `code-reviewer` agent
4. `cargo test` must pass before committing
5. One logical change per commit, one behavior per PR

**Specs live in `docs/specs/` and are written before any implementation.**
Feature files reference specs rather than hardcoding values (e.g., "score as defined in
`docs/specs/scoring.md`").

## Documentation

The `docs/` directory uses [mdBook](https://rust-lang.github.io/mdBook/). Only `docs/src/` is
compiled into the documentation site — `docs/specs/` and `docs/features/` are intentionally
outside `docs/src/` and are never included in the compiled output.

```bash
mdbook serve docs    # Live preview at http://localhost:3000
mdbook build docs    # Build static site to docs/book/
```

**Keep documentation current.** When adding or changing a feature, update the relevant page in
`docs/src/` in the same PR. When changing scoring behavior, update `docs/specs/scoring.md`
first (spec before implementation).

```
docs/
├── book.toml          # mdBook config (src = "src")
├── src/               # Compiled into the docs site
│   ├── SUMMARY.md
│   ├── features/      # mdBook feature pages (not Gherkin — these are user-facing docs)
│   └── ...
├── specs/             # Technical specifications (NOT compiled, not user-facing)
│   └── scoring.md
└── features/          # Gherkin feature files (NOT compiled, not user-facing)
    └── 01-discovery.feature
```

## Specifications

- `docs/specs/scoring.md` — Scoring algorithm: metrics, formulas, pattern detection, weights
- `docs/features/` — Gherkin feature files by user journey

## User Journeys

1. **Discovery** — understand what architectural structure currently exists in a codebase
2. **Validation** — identify where a codebase violates DDD + Hexagonal Architecture rules
3. **Progress Tracking** — measure improvement over time as refactoring proceeds

## Build & Development Commands

```bash
cargo build                        # Debug build
cargo build --release              # Optimized build (LTO, stripped)
cargo run --bin boundary            # Run CLI
cargo test                         # Run all tests
cargo test -p boundary-core        # Test a single crate
cargo test test_name               # Run a single test by name
cargo clippy --all                 # Lint all crates
cargo fmt                          # Format code
cargo fmt -- --check               # Check formatting without changes
```

## Workspace Architecture

Cargo workspace with five crates under `crates/`:

- **boundary** — CLI binary. Parses args (clap), loads config (toml), orchestrates analysis,
  formats output (colored).
- **boundary-core** — Core domain. Defines the `LanguageAnalyzer` trait, `DependencyGraph`,
  component/layer types, scoring algorithm, and violation detection. All language analyzers
  depend on this.
- **boundary-go** — Go language analyzer. Implements `LanguageAnalyzer` using tree-sitter-go.
  Extracts interfaces, structs, imports via tree-sitter queries.
- **boundary-rust** — Rust language analyzer (placeholder, not yet implemented).
- **boundary-report** — Report generation (placeholder, not yet implemented).

**Dependency flow:** `boundary` (CLI) → `boundary-core` + language crates + `boundary-report`.
Language crates depend on `boundary-core` for shared traits/types.

## Key Abstractions

The central trait is `LanguageAnalyzer` in `boundary-core/src/analyzer.rs` — each language
crate implements it. The analysis pipeline is: parse files → extract components & dependencies
→ classify into architectural layers → build dependency graph → detect pattern → calculate
scores.

Architectural layers: `Domain`, `Application`, `Infrastructure`, `Presentation`.

Scoring is defined in `docs/specs/scoring.md`. Do not change scoring behavior without first
updating the spec.

## Configuration

Project config lives in `.boundary.toml`. Defines layer path patterns, overrides, scoring
weights, and cross-cutting concern patterns.

## GitHub Workflows

`.github/workflows/` contains: `ci.yml`, `release.yml`, `publish-crates.yml`, `docs.yml`,
`boundary.yml`, `release-please.yml`, `pr-labeler.yml`.

**When adding a new crate, binary, test suite, or language analyzer**, review the workflows and
check whether any jobs or steps need updating — e.g., matrix entries, artifact paths, crate
publish lists, or test commands. Do not assume existing workflows automatically cover new
additions.

## Key Constraints

- **Spec first**: no scoring or classification changes without a corresponding spec update
- **Synthetic nodes are never real components**: `<file>` and `<package>` graph nodes must
  never be counted in presence, conformance, compliance, or coverage calculations
- **Undefined ≠ 100**: when a dimension has no data (no adapters, no cross-layer edges), it is
  undefined and not reported — never defaulted to a perfect score
