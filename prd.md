# Product Requirements Document: Boundary

**Version:** 2.3
**Last Updated:** February 24, 2026
**Author:** Stephen Morgan
**Status:** Active Development

---

## Executive Summary

Boundary is a static analysis tool that evaluates Domain-Driven Design (DDD) and Hexagonal Architecture patterns in codebases. It automatically detects architectural violations, scores adherence to architectural principles, and generates visual documentation of system boundaries and dependencies.

The tool emerged from the manual work of documenting a large codebase's architecture—turning those learnings into automation that teams can run continuously in their development workflow.

---

## Problem Statement

### Current Pain Points

1. **Manual Architecture Reviews** - Identifying layer violations requires deep code review and institutional knowledge
2. **Architecture Drift** - Clean boundaries erode over time as teams grow and developers change
3. **Documentation Lag** - Architecture documentation becomes stale and disconnected from reality
4. **Inconsistent Enforcement** - Architectural principles exist in wikis but aren't enforced in code
5. **Onboarding Friction** - New developers struggle to understand implicit architectural boundaries

### Why Existing Tools Fall Short

- **General linters** (eslint, golangci-lint) don't understand architectural layers
- **Dependency checkers** (`go mod graph`) show packages but not architectural violations
- **Language-specific tools** (ArchUnit for Java) require rewriting for each language
- **Manual reviews** don't scale and miss subtle violations

---

## Goals and Success Metrics

### Primary Goals

1. **Detect violations automatically** - Catch domain→infrastructure dependencies before they reach production
2. **Quantify architectural health** - Provide objective scores for layer isolation and dependency flow
3. **Generate documentation** - Produce up-to-date architecture diagrams from code
4. **Enable CI/CD integration** - Fail builds on critical violations

### Success Metrics

**Adoption Metrics:**

- 100+ GitHub stars in first 3 months
- 10+ organizations using in production within 6 months
- 500+ crates.io downloads per month

**Quality Metrics:**

- False positive rate <10% for violation detection
- Analysis completes in <30 seconds for 100k LOC codebases
- Zero critical bugs in violation detection logic

**Impact Metrics:**

- Users report 50%+ reduction in architecture review time
- 80%+ of violations caught in CI before PR review
- 90%+ of users find architectural scores useful

---

## User Personas

### Primary: Senior/Staff Engineers

**Needs:**

- Enforce architectural patterns across large codebases
- Prevent technical debt from accumulating
- Generate documentation for new team members
- Quantify architecture quality in metrics

**Pain Points:**

- Spending too much time in code review catching violations
- Architecture drift happening silently
- Difficulty visualizing system boundaries

### Secondary: Team Leads/Engineering Managers

**Needs:**

- Track architectural health over time
- Compare architecture quality across teams/services
- Make data-driven decisions about refactoring priorities

**Pain Points:**

- No objective measure of architecture quality
- Difficult to prioritize technical debt
- Can't track improvement over time

### Tertiary: New Engineers

**Needs:**

- Understand codebase architecture quickly
- Know what patterns to follow
- Get fast feedback on architectural mistakes

**Pain Points:**

- Implicit architectural knowledge not documented
- Getting surprised by violated patterns in review
- Uncertain about where to put new code

---

## User Stories

### MVP (Version 0.1)

```gherkin
As a senior engineer,
I want to analyze a Go repository for DDD violations,
So that I can identify where domain logic depends on infrastructure.

As a team lead,
I want to score our service's architectural health,
So that I can track improvement over time.

As a developer,
I want to run boundary in CI,
So that architectural violations are caught before code review.

As an architect,
I want to configure custom layer patterns,
So that boundary understands our specific project structure.

As a developer,
I want to see violation details with file/line numbers,
So that I can fix issues quickly.
```

### Post-MVP

```gherkin
As a senior engineer,
I want to generate architecture diagrams automatically,
So that documentation stays in sync with code.

As a team lead,
I want to analyze Rust and TypeScript repositories,
So that we can use boundary across our entire stack.

As a developer,
I want NeoVim inline warnings for violations,
So that I get feedback while coding.

As an architect,
I want to track architecture metrics over time,
So that I can see trends and regressions.
```

---

## Functional Requirements

### Core Analysis Engine

**FR-1: Multi-language AST Parsing**

- **Priority:** P0
- **Status:** ✅ Complete
- **Description:** Parse source files using tree-sitter to build ASTs
- **Acceptance Criteria:**
  - ✅ Supports Go (Phase 1)
  - ✅ Supports Rust (Phase 3)
  - ✅ Supports TypeScript/TSX (Phase 4)
  - ✅ Supports Java (Phase 4)
  - ✅ Gracefully handles parse errors
  - Processes files in parallel for performance

**FR-2: Component Extraction**

- **Priority:** P0
- **Description:** Identify architectural components from AST
- **Acceptance Criteria:**
  - Detects interfaces/traits (ports)
  - Identifies implementations (adapters)
  - Finds domain entities
  - Discovers repositories
  - Tracks use cases/services

**FR-3: Layer Classification**

- **Priority:** P0
- **Description:** Classify components into architectural layers
- **Acceptance Criteria:**
  - Uses configurable path patterns
  - Analyzes import statements
  - Supports custom layer definitions
  - Handles ambiguous cases gracefully

**FR-4: Dependency Graph Building**

- **Priority:** P0
- **Description:** Build directed graph of component dependencies
- **Acceptance Criteria:**
  - Tracks import relationships
  - Identifies method calls across layers
  - Detects type references
  - Handles transitive dependencies

### Violation Detection

**FR-5: Layer Boundary Violations**

- **Priority:** P0
- **Description:** Detect dependencies that violate layer boundaries
- **Acceptance Criteria:**
  - Flags domain→infrastructure dependencies
  - Detects circular dependencies
  - Identifies infrastructure leakage into domain
  - Configurable violation severity

**FR-6: Pattern Violations**

- **Priority:** P1
- **Description:** Detect missing or incorrect DDD patterns
- **Acceptance Criteria:**
  - Adapters without port interfaces
  - Direct database access in domain layer
  - Missing repository pattern
  - Infrastructure annotations in domain entities

**FR-7: Custom Violation Rules**

- **Priority:** P1
- **Description:** Allow users to define custom violation rules
- **Acceptance Criteria:**
  - TOML-based rule configuration
  - Regular expression support for patterns
  - Configurable severity levels
  - Custom violation messages

### Scoring System

**FR-8: Architecture Scoring**

- **Priority:** P0
- **Status:** ✅ Complete
- **Description:** Calculate numerical scores for architecture quality. Full specification in `docs/specs/scoring.md`.
- **Score Dimensions:**
  - **Structural Presence** ✅ — `(classified + cross_cutting) / total_real_components`. Synthetic nodes, external dependencies excluded.
  - **Layer Conformance** ✅ — Mean distance of each package's (A, I) values from its assigned layer's expected region on the main sequence. Uses R.C. Martin metrics (FR-26, FR-28). JSON field: `layer_conformance`.
  - **Dependency Rule Compliance** ✅ — `correct_edges / total_cross_layer_edges`. JSON field: `dependency_compliance`.
  - **Interface Coverage** ✅ — `min(ports, adapters) / max(ports, adapters) * 100`. JSON field: `interface_coverage`.
- **Overall Score Formula** ✅ — `presence × weighted_correctness / 100` (multiplicative gate)
- **Pattern Detection Gate** ✅ — When top pattern confidence < 0.5, DDD scores are suppressed (FR-27).
- **Configurable Weights** ✅ — `layer_conformance_weight`, `dependency_compliance_weight`, `interface_coverage_weight` in `.boundary.toml`

**FR-9: Metrics Collection**

- **Priority:** P1
- **Description:** Collect detailed metrics about codebase architecture
- **Acceptance Criteria:**
  - Component counts by type
  - Violation counts by severity
  - Dependency depth metrics
  - Layer coupling metrics

**FR-26: R.C. Martin Package Metrics**

- **Priority:** P1
- **Status:** ❌ Not Started
- **Description:** Compute Instability (I), Abstractness (A), and Distance from Main Sequence (D) per package, as defined in `docs/specs/scoring.md`. These are the foundation for true Layer Conformance scoring.
- **Acceptance Criteria:**
  - `I = Ce / (Ca + Ce)` — efferent vs total coupling; range 0.0–1.0
  - `A = Na / Nc` — abstract types vs total real components; range 0.0–1.0
  - `D = |A + I - 1|` — distance from main sequence; range 0.0–1.0
  - Packages with `Nc = 0` excluded from all scoring
  - Exposed in JSON output and metrics report
  - Zone of Pain / Zone of Uselessness flagging (informational, not a violation)

**FR-27: Pattern Detection with Confidence Distribution**

- **Priority:** P1
- **Status:** ❌ Not Started
- **Description:** Detect which architectural pattern a module most closely matches and produce a confidence distribution. Gates whether DDD scores are computed.
- **Acceptance Criteria:**
  - Detect: DDD+Hexagonal, Active Record, Flat CRUD, Anemic Domain, Service Layer
  - Each pattern produces a confidence value in [0.0, 1.0]; values are independent (do not sum to 1.0)
  - When top confidence ≥ 0.5: compute all applicable score dimensions
  - When top confidence < 0.5: report confidence distribution and structural presence only; omit DDD scores; suggest next steps
  - Confidence distribution included in JSON and text output
  - A module in transition may show high confidence in multiple patterns simultaneously

**FR-28: True Layer Conformance**

- **Priority:** P1
- **Status:** ✅ Complete
- **Description:** Replace the current `layer_isolation` approximation with true Layer Conformance based on R.C. Martin (A, I) metrics per package.
- **Acceptance Criteria:**
  - ✅ Expected (A, I) regions per layer: Domain (A ≥ 0.5, I ≤ 0.3), Application (A 0.2–0.6, I 0.3–0.7), Infrastructure (A ≤ 0.3, I ≥ 0.5), Presentation (A ≤ 0.3, I ≥ 0.5)
  - ✅ Conformance per package: `1.0 - distance((A,I), expected_region_centroid)`, clamped to [0.0, 1.0]
  - ✅ Overall layer conformance: mean over all classified packages with at least one real component
  - ✅ Exposed as `layer_conformance` in JSON output

### Reporting

**FR-10: CLI Output**

- **Priority:** P0
- **Description:** Human-readable terminal output
- **Acceptance Criteria:**
  - Colored, formatted text
  - Clear violation descriptions
  - File paths and line numbers
  - Summary statistics

**FR-11: JSON Output**

- **Priority:** P0
- **Description:** Machine-readable JSON for CI/CD
- **Acceptance Criteria:**
  - Structured violation data
  - Score breakdown
  - Exit codes (0=pass, 1=violations)
  - Schema documentation

**FR-12: Markdown Reports**

- **Priority:** P1
- **Description:** Generate markdown documentation
- **Acceptance Criteria:**
  - Architecture overview
  - Violation listing
  - Score breakdown
  - Component inventory

**FR-13: Diagram Generation**

- **Priority:** P2
- **Description:** Visual architecture diagrams
- **Acceptance Criteria:**
  - Mermaid format
  - GraphViz DOT format
  - Layer visualization
  - Dependency flow visualization

### Configuration

**FR-14: Configuration File**

- **Priority:** P0
- **Description:** `.boundary.toml` configuration support
- **Acceptance Criteria:**
  - Layer pattern definitions
  - Allowed imports per layer
  - Violation rules
  - Scoring weights
  - Schema validation

**FR-15: CLI Arguments**

- **Priority:** P0
- **Description:** Command-line configuration options
- **Acceptance Criteria:**
  - Override config file settings
  - Specify languages to analyze
  - Set output format
  - Control failure conditions

### CI/CD Integration

**FR-16: Exit Codes**

- **Priority:** P0
- **Description:** Proper exit codes for CI integration
- **Acceptance Criteria:**
  - 0 = success (no violations or only warnings)
  - 1 = failure (violations above threshold)
  - Configurable failure severity

**FR-17: Incremental Analysis**

- **Priority:** P2
- **Description:** Analyze only changed files
- **Acceptance Criteria:**
  - Git diff integration
  - Cached dependency graph
  - Fast re-analysis (<5s for small changes)

**FR-25: Module Forensics Reports**

- **Priority:** P1
- **Status:** ✅ Complete
- **Description:** Generate deep-dive forensics reports for individual modules with DDD pattern analysis
- **Acceptance Criteria:**
  - ✅ `boundary forensics <path>` CLI command
  - ✅ Module-scoped analysis (walks only the target directory)
  - ✅ Per-aggregate analysis: fields with types, method signatures, value objects, DDD patterns
  - ✅ Domain event detection (structs ending with `Event`)
  - ✅ Value object heuristic (structs without identity fields)
  - ✅ Import classification (stdlib, internal domain/application/infrastructure, external)
  - ✅ Dependency audit per entity (infrastructure leak detection)
  - ✅ Port/adapter mapping with interface coverage
  - ✅ Heuristic improvement suggestions (anemic models, missing events, unmatched ports)
  - ✅ Markdown report output with `--output` flag
  - ✅ Auto-detection of project root via `.boundary.toml` / `.git`

---

## Technical Requirements

### Performance

**TR-1: Analysis Speed**

- Analyze 100k LOC in <30 seconds
- Parallel file processing
- Efficient AST caching
- Memory usage <500MB for typical projects

**TR-2: Scalability**

- Handle monorepos with 1M+ LOC
- Support 1000+ files
- Process multiple languages simultaneously

### Reliability

**TR-3: Error Handling**

- Graceful parse error recovery
- Clear error messages
- No crashes on malformed input
- Partial analysis on errors

**TR-4: Correctness**

- False positive rate <10%
- False negative rate <5%
- Consistent results across runs
- Deterministic output

### Maintainability

**TR-5: Code Quality**

- 80%+ test coverage
- Documented public APIs
- Clean separation of concerns
- No clippy warnings

**TR-6: Extensibility**

- Plugin architecture for new languages
- Custom violation rule system
- Configurable scoring algorithms

### Compatibility

**TR-7: Platform Support**

- Linux (primary target)
- macOS
- Windows (best effort)

**TR-8: Language Versions**

- Go: 1.18+
- Rust: 2021 edition+
- (Future languages TBD)

---

## Architecture

### System Components

```
┌─────────────────────────────────────────────────────┐
│                   boundary (CLI)                     │
│  - Argument parsing (clap)                           │
│  - Configuration loading (toml)                      │
│  - Orchestration via AnalysisPipeline                │
└────────────────────┬────────────────────────────────┘
                     │
        ┌────────────┼────────────┐
        ▼            │            ▼
┌──────────────────┐ │   ┌──────────────────┐
│  boundary-core   │ │   │ boundary-report  │
│  - Analyzer trait│ │   │  - Text (colored)│
│  - Graph types   │ │   │  - Markdown      │
│  - Metrics/Score │ │   │  - Mermaid       │
│  - Violations    │ │   │  - GraphViz DOT  │
│  - Pipeline      │ │   └──────────────────┘
│  - Cache         │ │
└────────┬─────────┘ │
         │ implements │
    ┌────┴────┬───────┴──┬─────────┐
    ▼         ▼          ▼         ▼
┌─────────┐ ┌────────┐ ┌──────┐ ┌──────────┐
│boundary-│ │boundary│ │bound-│ │boundary- │
│go       │ │-rust   │ │ary-  │ │typescript│
│         │ │        │ │java  │ │          │
└─────────┘ └────────┘ └──────┘ └──────────┘

┌─────────────────────────────────────────────────────┐
│                  boundary-lsp                        │
│  - tower-lsp based LSP server                        │
│  - Real-time diagnostics on save                     │
│  - Hover info for component layer/kind               │
│  - Uses AnalysisPipeline + incremental cache         │
└─────────────────────────────────────────────────────┘
```

### Data Flow

1. **Input** → Configuration file + Source code paths
2. **Parse** → Tree-sitter ASTs for each file
3. **Extract** → Components, imports, relationships
4. **Classify** → Assign components to architectural layers
5. **Build Graph** → Dependency graph with layer metadata
6. **Analyze** → Detect violations, calculate scores
7. **Report** → Format and output results

### Key Design Decisions

**DD-1: Tree-sitter for Parsing**

- **Rationale:** Robust, battle-tested, multi-language support
- **Alternative Considered:** Language-specific parsers (too much maintenance)

**DD-2: Cargo Workspace Structure**

- **Rationale:** Clear separation, easy to add languages, good for open source
- **Alternative Considered:** Monolithic crate (harder to maintain)

**DD-3: TOML Configuration**

- **Rationale:** Rust-native, human-readable, good schema support
- **Alternative Considered:** YAML (too permissive), JSON (not human-friendly)

**DD-4: Petgraph for Dependency Graphs**

- **Rationale:** Mature, well-documented, good algorithms
- **Alternative Considered:** Custom graph implementation (reinventing wheel)

---

## User Experience

### CLI Interface

```bash
# Primary commands
boundary analyze <path>           # Analyze and show report
boundary check <path>              # Analyze and exit with code
boundary diagram <path>            # Generate diagrams
boundary forensics <path>          # Deep-dive module forensics report
boundary init                      # Create .boundary.toml template

# Common options
--config <path>                    # Custom config file
--format <markdown|json|text>      # Output format
--fail-on <error|warning|info>     # Failure threshold
--languages <go,rust>              # Limit to languages
--output <path>                    # Write to file
--verbose                          # Detailed output
--quiet                            # Minimal output
```

### Example Workflows

**Workflow 1: Initial Repository Analysis**

```bash
# Generate config template
boundary init

# Edit .boundary.toml with project-specific patterns
vim .boundary.toml

# Run analysis
boundary analyze .

# Review violations and fix
# ...

# Re-analyze
boundary analyze .
```

**Workflow 2: CI/CD Integration**

```bash
# In GitHub Actions / GitLab CI
boundary check . --format json --fail-on error
```

**Workflow 3: Documentation Generation**

```bash
# Generate architecture documentation
boundary analyze . --format markdown --output docs/architecture.md

# Generate diagrams
boundary diagram . --output docs/architecture.mermaid
```

### Error Messages

**Good Error Message:**

```
❌ ERROR: Domain layer depends on infrastructure

File: internal/domain/user/repository.go:15
Layer: domain
Import: github.com/yourorg/app/internal/infrastructure/postgres

The domain layer should not import from the infrastructure layer.
This creates tight coupling and makes testing difficult.

Suggestion: Create a port interface in the domain layer and
implement it with an adapter in the infrastructure layer.

Example:
  // domain/user/repository.go
  type UserRepository interface {
    Save(ctx context.Context, user *User) error
  }

  // infrastructure/postgres/user_repository.go
  type PostgresUserRepository struct { ... }
  func (r *PostgresUserRepository) Save(...) error { ... }
```

**Bad Error Message:**

```
Error: violation at internal/domain/user/repository.go:15
```

---

## Milestones and Phases

### Phase 0: Foundation ✅

**Goal:** Establish project structure and basic parsing

- [x] Create repository structure
- [x] Set up Cargo workspace
- [x] Define core traits and types
- [x] Implement basic Go file parsing
- [x] Write initial tests

**Deliverable:** Can parse Go files and extract basic components

### Phase 1: MVP ✅

**Goal:** Working Go analyzer with violation detection

- [x] Complete Go component extraction
- [x] Implement layer classification (pattern-based)
- [x] Build dependency graph
- [x] Detect basic violations (domain→infra)
- [x] Calculate simple scores
- [x] CLI with text output
- [x] Configuration file support

**Deliverable:** `boundary analyze` works on real Go projects

### Phase 2: CI/CD Ready ✅

**Goal:** Production-ready for Go projects

- [x] JSON output format
- [x] Exit code handling
- [x] Performance optimization
- [x] Comprehensive error handling
- [x] Documentation and examples
- [x] GitHub Actions example

**Deliverable:** Teams can use in CI pipelines

### Phase 3: Enhanced Analysis ✅

**Goal:** Advanced features and Rust support

- [x] Markdown report generation
- [x] Diagram generation (Mermaid)
- [x] Rust language analyzer
- [x] Custom violation rules
- [x] Metrics collection
- [x] Architecture evolution tracking

**Deliverable:** `boundary` v0.3.0 release

### Phase 4: Ecosystem (Complete)

**Goal:** Broad language support, LSP, NeoVim plugin, and incremental analysis

- [x] TypeScript analyzer (crate created, tree-sitter-typescript)
- [x] Java analyzer (crate created, tree-sitter-java)
- [x] FR-6: Pattern violation detection (adapter without port, domain-infra leak)
- [x] FR-13: GraphViz DOT output
- [x] FR-17: Incremental analysis with SHA-256 content hashing
- [x] LSP server (boundary-lsp) with tower-lsp
- [x] Cross-platform binary releases via cargo-dist
- [x] NeoVim plugin ([boundary.nvim](https://github.com/rebelopsio/boundary.nvim)) — separate repo, connects to boundary-lsp
- [x] `boundary forensics` command — deep-dive module reports with DDD pattern detection, dependency audits, and improvement suggestions
- [x] Enriched data model — field types, method signatures, domain event detection, value object heuristics
- [x] Module-scoped analysis (`analyze_module()`) for targeted forensics
- [x] Scoring bug fixes (unclassified component handling, display formatting)
- [x] Validate TypeScript analyzer against real TS projects
- [x] Validate Java analyzer against real Java projects
- [x] Validate LSP server end-to-end with NeoVim plugin
- [x] End-to-end integration tests for all 4 language analyzers

**Released:** `boundary` v0.4.0 (core features), v0.4.2 (validation & integration tests)

### Phase 5: Real-World Accuracy ✅

**Goal:** Reduce false positives and handle real-world architectural patterns discovered from analyzing production codebases (639-core, etc.)

**FR-18: Configurable Layer Classification Patterns**

- **Priority:** P0
- **Description:** Allow `.boundary.toml` to define custom path-to-layer mappings beyond the built-in heuristics
- **Acceptance Criteria:**
  - Support glob-based path patterns for layer assignment (e.g., `"common/modules/*/domain/**" = "domain"`)
  - Per-module/service layer overrides for monorepos
  - Support microservice directory structures where each service has its own internal layers
  - Pattern: `services/*/server/` → infrastructure, `common/modules/*/domain/` → domain

**FR-19: Cross-Cutting Concern Exclusions**

- **Priority:** P0
- **Description:** Allow marking packages/paths as cross-cutting concerns that are exempt from layer violation checks
- **Acceptance Criteria:**
  - Configurable list of cross-cutting paths (e.g., `common/utils/`, `pkg/logger/`)
  - Cross-cutting components excluded from layer isolation scoring
  - Dependencies to/from cross-cutting concerns don't count as violations
  - Still tracked in dependency graph for visualization purposes

**FR-20: Active Record Pattern Recognition**

- **Priority:** P1
- **Description:** Recognize Active Record entities (domain objects with inline DB methods) as a valid architectural pattern
- **Acceptance Criteria:**
  - Detect Active Record pattern: entities with methods like `.Load()`, `.Save()`, `.UpdateSet()` that call DB directly
  - Configurable mode: `strict` (flags as violation) vs `permissive` (allows Active Record in domain)
  - When permissive, don't flag domain entities importing DB drivers as violations
  - Default: `strict` (standard DDD)

**FR-21: Init Function Dependency Detection (Go)**

- **Priority:** P1
- **Description:** Detect implicit dependencies created by Go `init()` function registration patterns
- **Acceptance Criteria:**
  - Detect `init()` functions that register routes, services, or handlers
  - Track global variable assignments in `init()` as implicit dependencies
  - Flag init-based side effects that create hidden cross-layer coupling
  - Include init-registered dependencies in the dependency graph

**FR-22: Hybrid Architecture Tolerance**

- **Priority:** P1
- **Description:** Support codebases that intentionally use different patterns in different modules (e.g., full DDD for complex domains, Active Record for simple CRUD)
- **Acceptance Criteria:**
  - Per-module architecture mode configuration in `.boundary.toml`
  - Modules can declare their pattern: `ddd`, `active-record`, `service-oriented`
  - Scoring adjusts expectations based on declared pattern
  - Cross-module dependencies still enforce layer rules at module boundaries

**FR-23: Unclassified Component Handling**

- **Priority:** P0
- **Description:** Provide clear guidance when many components can't be classified into layers
- **Acceptance Criteria:**
  - Report percentage of unclassified components prominently
  - Suggest `.boundary.toml` patterns for unclassified paths
  - Separate "classification coverage" metric distinct from "layer isolation"
  - Don't inflate scores by ignoring unclassified components (fixed in v0.4.0)

**FR-24: Monorepo / Multi-Service Support**

- **Priority:** P1
- **Description:** First-class support for monorepos with multiple services sharing domain modules
- **Acceptance Criteria:**
  - Analyze individual services independently within a monorepo
  - Detect shared domain modules used across services
  - Score each service separately with aggregate rollup
  - Identify cross-service dependency violations
  - Support `services/` directory pattern with per-service layer structure

### Phase 6: Scoring Spec Completion ✅

**Goal:** Fully implement the scoring specification in `docs/specs/scoring.md`, replacing the current edge-counting approximations with R.C. Martin-based metrics and pattern-aware scoring.

- [x] FR-26: Compute Instability (I), Abstractness (A), Distance (D) per package
- [x] FR-27: Pattern detection with confidence distribution; gate DDD scores on confidence ≥ 0.5
- [x] FR-28: True Layer Conformance based on (A, I) distance to expected layer region
- [x] Fix Interface Coverage formula: `min(ports, adapters) / max(ports, adapters)`
- [x] Resolve field naming: `layer_conformance`, `dependency_compliance` (spec-aligned)
- [x] Expose per-package I, A, D values in JSON output and metrics report
- [x] Zone of Pain / Zone of Uselessness detection (informational)
- [x] Update `docs/specs/scoring.md` status from Draft → Active

**Deliverable:** Scoring matches the specification exactly; pattern confidence drives whether architecture scores are shown.

---

## Out of Scope

### Delivered (previously out of scope)

- ~~Multiple language support~~ → Go, Rust, TypeScript, Java all supported
- ~~Diagram generation~~ → Mermaid + GraphViz DOT supported
- ~~VS Code integration~~ → LSP server available for any editor
- ~~Architecture evolution tracking~~ → Implemented in Phase 3
- ~~Custom violation rules~~ → TOML-based rules supported

### Not Planned

- Runtime analysis (only static analysis)
- Code modification/refactoring
- Security vulnerability detection
- Performance analysis
- Test coverage analysis
- License compliance checking
- HTML reports (markdown + JSON cover reporting needs)
- GitHub App integration (CLI + CI is sufficient)
- Architecture dashboard web UI

---

## Open Questions

### Resolved

**Q1:** How do we handle Go modules with replace directives?

- **Status:** Resolved — tree-sitter parses at the file level; module resolution is out of scope for static analysis

**Q2:** Should we cache parsed ASTs between runs?

- **Status:** Resolved — Implemented in Phase 4 (FR-17) using SHA-256 content hashing with `.boundary/cache.json`

**Q3:** How do we handle generic types in Go 1.18+?

- **Status:** Resolved — tree-sitter-go handles generics natively

**Q4:** Should we support incremental analysis in MVP?

- **Status:** Resolved — Added in Phase 4 with `--incremental` flag

**Q5:** What's the right default failure threshold?

- **Status:** Resolved — Default is `error` severity; configurable with `--fail-on`

**Q6:** Should we support both hexagonal and clean architecture?

- **Status:** Resolved — Layer patterns are configurable in `.boundary.toml`

### Open

**Q7:** How should Active Record entities be scored?

- **Status:** Open
- **Impact:** High — affects false positive rate on real codebases
- **Context:** 639-core uses Active Record for simple modules (entities with inline `.Load()`, `.Save()` DB methods). Strict DDD flags these as violations, but they're intentional.
- **Options:** (a) Global `architecture_mode` toggle, (b) Per-module mode in config, (c) Annotation-based opt-out
- **Lean Toward:** Per-module mode (FR-22)

**Q8:** How should unclassified components affect scoring?

- **Status:** Open
- **Impact:** High — directly affects score accuracy
- **Context:** On 639-core (979 components), 607 were unclassified, inflating scores to 100/100 before the v0.4.0 fix. Now they penalize scores, but projects with no `.boundary.toml` patterns get very low scores.
- **Options:** (a) Separate "classification coverage" metric, (b) Only score classified components but show coverage %, (c) Auto-suggest patterns
- **Lean Toward:** Separate classification coverage metric (FR-23) + auto-suggest

**Q9:** Should boundary support analyzing individual services within a monorepo?

- **Status:** Open
- **Impact:** Medium — key for microservice architectures
- **Context:** 639-core has 15+ services under `services/` with shared `common/modules/`. Analyzing the whole repo as one unit produces noisy results.
- **Lean Toward:** Yes, with per-service config sections (FR-24)

**Q10:** How should Go `init()` function side effects be modeled?

- **Status:** Open
- **Impact:** Medium — creates hidden dependencies in Go codebases
- **Context:** 639-core uses `init()` to register routes and services globally. These dependencies are invisible to import-based analysis.
- **Lean Toward:** Detect and flag as a separate violation type (FR-21)

**Q11:** Should we rename `layer_isolation` / `dependency_direction` to match the spec?

- **Status:** ✅ Resolved — renamed in Phase 6 when FR-28 shipped. JSON output now uses `layer_conformance` and `dependency_compliance`. Config weight keys updated accordingly.

**Q12:** How do we communicate scoring limitations to users before Phase 6 is complete?

- **Status:** ✅ Resolved — Phase 6 is complete. Scores are now spec-compliant. Pattern detection gate suppresses scores when confidence < 0.5.

---

## Success Criteria

### MVP Success (Phase 1) ✅

- ✅ Analyze Go repositories successfully
- ✅ Detect domain→infrastructure violations
- ✅ Calculate basic architecture scores
- ✅ Output readable CLI reports
- ✅ Configuration file support

### CI/CD Success (Phase 2) ✅

- ✅ JSON output format
- ✅ Exit codes for CI pipelines
- ✅ Configurable failure severity

### Enhanced Analysis Success (Phase 3) ✅

- ✅ Markdown report generation
- ✅ Mermaid diagram generation
- ✅ Rust language analyzer
- ✅ Custom violation rules (TOML-based)
- ✅ Metrics collection and evolution tracking

### Ecosystem Success (Phase 4) ✅

- ✅ 4 language analyzers (Go, Rust, TypeScript, Java)
- ✅ LSP server for editor integration
- ✅ Incremental analysis with caching
- ✅ GraphViz DOT output
- ✅ Cross-platform binary releases (macOS, Linux, Windows)
- ✅ Module forensics reports with DDD pattern detection
- ✅ Enriched component extraction (field types, method signatures, domain events, value objects)

### Real-World Accuracy (Phase 5) ✅

- [x] Configurable layer patterns for non-standard project structures
- [x] Cross-cutting concern exclusions reduce noise
- [x] Monorepo per-service analysis
- [x] Classification coverage metric guides users toward accurate scoring
- [x] Active Record, hybrid architecture, and init function coupling handled

### Scoring Spec Success (Phase 6) ✅

- [x] R.C. Martin metrics (I, A, D) computed per package
- [x] Pattern detection confidence gates score output
- [x] Layer Conformance based on (A, I) distance to expected region
- [x] Interface Coverage formula matches spec exactly
- [x] Field names aligned with `docs/specs/scoring.md`
- [x] `docs/specs/scoring.md` status updated to Active

### Long-term Success (1 Year)

- 100+ GitHub stars
- 4+ language analyzers (achieved)
- Active community contributions
- Featured in DDD/architecture communities
- VS Code / NeoVim plugin ecosystem

---

## Appendix

### Related Work

- **ArchUnit** (Java) - Architecture testing framework
- **NDepend** (.NET) - Code quality and architecture analysis
- **cargo-modules** (Rust) - Module visualization
- **Go's import cycle detection** - Built into compiler

### References

- [Domain-Driven Design by Eric Evans](https://www.domainlanguage.com/ddd/)
- [Hexagonal Architecture by Alistair Cockburn](https://alistair.cockburn.us/hexagonal-architecture/)
- [Clean Architecture by Robert C. Martin](https://blog.cleancoder.com/uncle-bob/2012/08/13/the-clean-architecture.html)
- [Tree-sitter Documentation](https://tree-sitter.github.io/tree-sitter/)
- [Boundary Scoring Specification](../specs/scoring.md) — defines all score dimensions, formulas, and pattern detection

### Glossary

- **Port** - Interface defining a boundary (in hexagonal architecture)
- **Adapter** - Implementation of a port
- **Layer** - Architectural separation (domain, application, infrastructure)
- **Violation** - Code that breaks architectural rules
- **Component** - Unit of analysis (struct, interface, function, etc.)
- **DDD** - Domain-Driven Design
- **AST** - Abstract Syntax Tree

---

**Document History:**

| Version | Date | Author | Changes |
|---------|------|--------|---------|
| 1.0 | 2026-02-15 | Stephen Morgan | Initial draft |
| 2.0 | 2026-02-17 | Stephen Morgan | Updated to reflect Phases 0-4 completion, added Phase 5 (Real-World Accuracy) based on 639-core analysis findings, resolved open questions Q1-Q6, added new FRs 18-24 |
| 2.1 | 2026-02-17 | Stephen Morgan | Added FR-25 (Module Forensics Reports), updated Phase 4 with forensics completion, enriched data model, and scoring fixes |
| 2.2 | 2026-02-21 | Stephen Morgan | Added scoring spec gap analysis to FR-8; added FR-26 (R.C. Martin metrics), FR-27 (pattern detection), FR-28 (layer conformance); added Phase 6 (Scoring Spec Completion); added Q11–Q12 |
| 2.3 | 2026-02-24 | Stephen Morgan | Marked Phase 5 and Phase 6 complete; updated FR-8, FR-28, Q11, Q12 to reflect shipped implementation; resolved all scoring spec gaps |
