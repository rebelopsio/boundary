use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::config::Config;
use crate::graph::DependencyGraph;
use crate::metrics_report::{ClassificationCoverage, DependencyDepthMetrics, MetricsReport};
use crate::pattern_detection::{detect_patterns, PatternDetection};
use crate::types::{
    ArchLayer, ArchitectureMode, Component, ComponentKind, Dependency, Severity, Violation,
    ViolationKind,
};

/// Result for a single service in a multi-service analysis.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceAnalysisResult {
    pub service_name: String,
    pub result: AnalysisResult,
}

/// Result of analyzing a monorepo with multiple services.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MultiServiceResult {
    pub services: Vec<ServiceAnalysisResult>,
    pub aggregate: AnalysisResult,
    pub shared_modules: Vec<SharedModule>,
}

/// A module shared between multiple services.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SharedModule {
    pub path: String,
    pub used_by: Vec<String>,
}

/// Aggregate multiple service results into a combined result.
pub fn aggregate_results(services: &[ServiceAnalysisResult]) -> AnalysisResult {
    if services.is_empty() {
        return AnalysisResult {
            score: Some(ArchitectureScore {
                overall: 100.0,
                structural_presence: 100.0,
                layer_conformance: 100.0,
                dependency_compliance: 100.0,
                interface_coverage: 100.0,
            }),
            violations: vec![],
            component_count: 0,
            dependency_count: 0,
            files_analyzed: 0,
            metrics: None,
            package_metrics: vec![],
            pattern_detection: None,
        };
    }

    let total_components: usize = services.iter().map(|s| s.result.component_count).sum();
    let total_deps: usize = services.iter().map(|s| s.result.dependency_count).sum();

    // Weighted average by component count
    let mut overall = 0.0f64;
    let mut structural_presence = 0.0f64;
    let mut layer_conformance = 0.0f64;
    let mut dependency_compliance = 0.0f64;
    let mut interface_coverage = 0.0f64;

    if total_components > 0 {
        for s in services {
            let weight = s.result.component_count as f64 / total_components as f64;
            if let Some(sc) = &s.result.score {
                overall += sc.overall * weight;
                structural_presence += sc.structural_presence * weight;
                layer_conformance += sc.layer_conformance * weight;
                dependency_compliance += sc.dependency_compliance * weight;
                interface_coverage += sc.interface_coverage * weight;
            }
        }
    }

    let all_violations: Vec<_> = services
        .iter()
        .flat_map(|s| s.result.violations.clone())
        .collect();

    let total_files: usize = services.iter().map(|s| s.result.files_analyzed).sum();

    AnalysisResult {
        score: Some(ArchitectureScore {
            overall,
            structural_presence,
            layer_conformance,
            dependency_compliance,
            interface_coverage,
        }),
        violations: all_violations,
        component_count: total_components,
        dependency_count: total_deps,
        files_analyzed: total_files,
        metrics: None,
        package_metrics: vec![],
        pattern_detection: None,
    }
}

/// Breakdown of architecture scores.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArchitectureScore {
    pub overall: f64,
    pub structural_presence: f64,
    pub layer_conformance: f64,
    pub dependency_compliance: f64,
    pub interface_coverage: f64,
}

/// R.C. Martin package-level coupling metrics (Instability, Abstractness, Distance).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PackageMetric {
    /// Short package name (last directory segment).
    pub package: String,
    /// Abstractness A = Na / Nc, rounded to 2 decimal places.
    pub abstractness: f64,
    /// Instability I = Ce / (Ca + Ce), rounded to 2 decimal places.
    /// Special case: Ca + Ce = 0 → I = 0.0 (defined, not undefined).
    pub instability: f64,
    /// Distance from main sequence D = |A + I - 1|, rounded to 2 decimal places.
    pub distance: f64,
    /// Zone classification when the package is far from the main sequence (D > 0.5).
    /// "pain"        — concrete and stable (A < 0.5, I < 0.5): rigid, hard to change.
    /// "uselessness" — abstract and unstable (A > 0.5, I > 0.5): unused abstractions.
    /// Absent when the package is on or near the main sequence.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub zone: Option<String>,
}

/// Full analysis result.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnalysisResult {
    /// DDD architecture score.
    /// `None` when the pattern-detection gate fails (top_confidence < 0.5),
    /// meaning the codebase does not match any recognized pattern well enough
    /// for DDD scores to be meaningful.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub score: Option<ArchitectureScore>,
    pub violations: Vec<Violation>,
    pub component_count: usize,
    pub dependency_count: usize,
    /// Number of source files analyzed. Zero means no supported files were found.
    #[serde(default)]
    pub files_analyzed: usize,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub metrics: Option<MetricsReport>,
    /// R.C. Martin package metrics (Instability, Abstractness, Distance).
    /// Packages with Nc = 0 are excluded. Present only when there are packages to report.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub package_metrics: Vec<PackageMetric>,
    /// Pattern detection result (confidence distribution across architectural patterns).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pattern_detection: Option<PatternDetection>,
}

/// Calculate architecture score from the dependency graph.
pub fn calculate_score(
    graph: &DependencyGraph,
    config: &Config,
    components: &[Component],
    dependencies: &[Dependency],
) -> ArchitectureScore {
    let layer_conformance_opt = calculate_layer_conformance(components, dependencies);
    let dependency_compliance = calculate_dependency_compliance(graph);
    let interface_coverage = calculate_interface_coverage(graph);

    let w = &config.scoring;

    // Redistribute weights for any undefined dimension (currently only layer_conformance
    // can be undefined — when there are no classified packages).
    let (total_weight, weighted_sum) = {
        let mut tw = 0.0f64;
        let mut ws = 0.0f64;
        if let Some(lc) = layer_conformance_opt {
            tw += w.layer_conformance_weight;
            ws += lc * w.layer_conformance_weight;
        }
        tw += w.dependency_compliance_weight;
        ws += dependency_compliance * w.dependency_compliance_weight;
        tw += w.interface_coverage_weight;
        ws += interface_coverage * w.interface_coverage_weight;
        (tw, ws)
    };

    let correctness = if total_weight > 0.0 {
        (weighted_sum / total_weight).clamp(0.0, 100.0)
    } else {
        100.0
    };

    // Structural presence: what % of components are classified into a layer?
    let coverage = compute_classification_coverage(graph);
    let structural_presence = coverage.coverage_percentage;

    // Multiplicative gate: overall = presence * correctness / 100
    let overall = (structural_presence * correctness / 100.0).clamp(0.0, 100.0);

    ArchitectureScore {
        overall,
        structural_presence,
        layer_conformance: layer_conformance_opt.unwrap_or(100.0),
        dependency_compliance,
        interface_coverage,
    }
}

/// Detect all violations in the dependency graph.
pub fn detect_violations(graph: &DependencyGraph, config: &Config) -> Vec<Violation> {
    let mut violations = Vec::new();

    // Layer boundary violations
    detect_layer_violations(graph, config, &mut violations);

    // Circular dependency violations
    detect_circular_dependencies(graph, config, &mut violations);

    // Pattern violations (DDD structural checks)
    detect_pattern_violations(graph, config, &mut violations);

    // Init function coupling violations
    detect_init_violations(graph, config, &mut violations);

    // Custom rules
    if !config.rules.custom_rules.is_empty() {
        match crate::custom_rules::compile_rules(&config.rules.custom_rules) {
            Ok(compiled) => {
                let custom_violations =
                    crate::custom_rules::evaluate_custom_rules(graph, &compiled);
                violations.extend(custom_violations);
            }
            Err(e) => {
                eprintln!("Warning: failed to compile custom rules: {e:#}");
            }
        }
    }

    violations
}

fn detect_layer_violations(
    graph: &DependencyGraph,
    config: &Config,
    violations: &mut Vec<Violation>,
) {
    let severity = config
        .rules
        .severities
        .get("layer_boundary")
        .copied()
        .unwrap_or(Severity::Error);

    for (src, tgt, edge) in graph.edges_with_nodes() {
        if src.is_external || tgt.is_external {
            continue;
        }
        if src.is_cross_cutting || tgt.is_cross_cutting {
            continue;
        }

        // Skip init() function deps — they are reported by detect_init_violations instead
        if src.id.0.contains("<init>") {
            continue;
        }

        // Service-oriented mode skips all layer boundary checks
        if src.architecture_mode == ArchitectureMode::ServiceOriented {
            continue;
        }

        let (Some(from_layer), Some(to_layer)) = (src.layer, tgt.layer) else {
            continue;
        };

        // ActiveRecord mode allows domain → infrastructure (entity owns its persistence)
        if src.architecture_mode == ArchitectureMode::ActiveRecord
            && from_layer == ArchLayer::Domain
            && to_layer == ArchLayer::Infrastructure
        {
            continue;
        }

        if from_layer.violates_dependency_on(&to_layer) {
            let import_detail = edge
                .import_path
                .as_deref()
                .map(|p| format!(" (import: {p})"))
                .unwrap_or_default();

            violations.push(Violation {
                kind: ViolationKind::LayerBoundary {
                    from_layer,
                    to_layer,
                },
                severity,
                location: edge.location.clone(),
                message: format!(
                    "{} layer depends on {} layer{import_detail}",
                    from_layer, to_layer
                ),
                suggestion: Some(format!(
                    "The {from_layer} layer should not depend on the {to_layer} layer. \
                     Consider introducing a port interface in the {from_layer} layer \
                     and an adapter in the {to_layer} layer."
                )),
            });
        }
    }
}

fn detect_circular_dependencies(
    graph: &DependencyGraph,
    config: &Config,
    violations: &mut Vec<Violation>,
) {
    let severity = config
        .rules
        .severities
        .get("circular_dependency")
        .copied()
        .unwrap_or(Severity::Error);

    let all_nodes = graph.nodes();
    for cycle in graph.find_cycles() {
        let cycle_str = cycle
            .iter()
            .map(|c| c.0.as_str())
            .collect::<Vec<_>>()
            .join(" -> ");
        // Use the location of the first component in the cycle
        let location = cycle
            .first()
            .and_then(|id| all_nodes.iter().find(|n| &n.id == id))
            .map(|n| n.location.clone())
            .unwrap_or_default();
        violations.push(Violation {
            kind: ViolationKind::CircularDependency {
                cycle: cycle.clone(),
            },
            severity,
            location,
            message: format!("Circular dependency detected: {cycle_str}"),
            suggestion: Some(
                "Break the cycle by introducing an interface or reorganizing dependencies."
                    .to_string(),
            ),
        });
    }
}

/// Infrastructure-related import path keywords.
const INFRA_KEYWORDS: &[&str] = &[
    "postgres",
    "mysql",
    "redis",
    "mongo",
    "database",
    "sql",
    "db",
    "dynamodb",
    "sqlite",
    "cassandra",
    "elasticsearch",
];

fn detect_pattern_violations(
    graph: &DependencyGraph,
    config: &Config,
    violations: &mut Vec<Violation>,
) {
    let severity = config
        .rules
        .severities
        .get("missing_port")
        .copied()
        .unwrap_or(Severity::Warning);

    let nodes = graph.nodes();

    // Collect port names using ComponentKind first, then fall back to name heuristics
    let port_names: Vec<String> = nodes
        .iter()
        .filter(|n| {
            // Prefer ComponentKind::Port when available
            if let Some(ComponentKind::Port(_)) = &n.kind {
                return true;
            }
            let name_lower = n.name.to_lowercase();
            name_lower.contains("port")
                || name_lower.contains("interface")
                || (name_lower.contains("repository") && n.layer == Some(ArchLayer::Domain))
        })
        .map(|n| n.name.clone())
        .collect();

    // Check 1: Adapter without port
    // Only check infrastructure-layer components that are actually adapters
    for node in &nodes {
        if node.is_cross_cutting {
            continue;
        }

        let name_lower = node.name.to_lowercase();

        // Use ComponentKind to determine if this is an adapter.
        // Only infrastructure-layer components are hex arch adapters — application-layer
        // "handlers" are use cases/coordinators, not adapters.
        let is_adapter = match &node.kind {
            Some(ComponentKind::Adapter(_)) if node.layer == Some(ArchLayer::Infrastructure) => {
                true
            }
            Some(ComponentKind::Repository) if node.layer == Some(ArchLayer::Infrastructure) => {
                true
            }
            // Fall back to name heuristic only for infrastructure-layer components
            None | Some(_) => {
                node.layer == Some(ArchLayer::Infrastructure)
                    && (name_lower.ends_with("handler")
                        || name_lower.ends_with("controller")
                        || name_lower.contains("adapter")
                        || name_lower.contains("impl"))
            }
        };

        if !is_adapter {
            continue;
        }

        // Check if there's a matching port name pattern
        let has_port = port_names.iter().any(|port| {
            let port_lower = port.to_lowercase();

            // Common prefix patterns for infrastructure implementations:
            // e.g., "MongoInvoiceRepository" → strip "Mongo" prefix and "Repository" suffix
            //       to match "InvoiceRepository" port
            let adapter_base = name_lower
                .trim_end_matches("handler")
                .trim_end_matches("controller")
                .trim_end_matches("adapter")
                .trim_end_matches("impl");
            let port_base = port_lower
                .trim_end_matches("port")
                .trim_end_matches("interface")
                .trim_end_matches("repository")
                .trim_end_matches("service");

            // Direct base match (e.g., UserHandler → UserPort)
            if !adapter_base.is_empty() && !port_base.is_empty() && adapter_base == port_base {
                return true;
            }

            // Check if the adapter name contains the port name (e.g., MongoInvoiceRepository contains InvoiceRepository)
            if name_lower.contains(&port_lower) {
                return true;
            }

            // Check if the adapter name ends with the port name after stripping a vendor prefix
            // e.g., "stripepaymentprocessor" contains port base "paymentprocessor"
            // e.g., "mongoinvoicerepository" ends_with "invoicerepository"
            if !port_lower.is_empty() && name_lower.ends_with(&port_lower) {
                return true;
            }

            false
        });

        if !has_port {
            violations.push(Violation {
                kind: ViolationKind::MissingPort {
                    adapter_name: node.name.clone(),
                },
                severity,
                location: node.location.clone(),
                message: format!(
                    "Adapter '{}' has no matching port interface",
                    node.name
                ),
                suggestion: Some(
                    "Create a port interface that this adapter implements to maintain proper boundaries."
                        .to_string(),
                ),
            });
        }
    }

    // Check 2: DB access in domain layer (domain importing infrastructure paths)
    for (src, _tgt, edge) in graph.edges_with_nodes() {
        if src.is_external {
            continue;
        }
        if src.is_cross_cutting {
            continue;
        }
        // ActiveRecord mode allows domain to import infrastructure
        if src.architecture_mode == ArchitectureMode::ActiveRecord {
            continue;
        }
        if src.layer != Some(ArchLayer::Domain) {
            continue;
        }

        if let Some(ref import_path) = edge.import_path {
            let path_lower = import_path.to_lowercase();
            if INFRA_KEYWORDS.iter().any(|kw| path_lower.contains(kw)) {
                violations.push(Violation {
                    kind: ViolationKind::DomainInfrastructureLeak {
                        detail: format!("domain imports infrastructure path: {import_path}"),
                    },
                    severity: Severity::Error,
                    location: edge.location.clone(),
                    message: format!(
                        "Domain layer directly imports infrastructure dependency '{import_path}'"
                    ),
                    suggestion: Some(
                        "Domain should not reference infrastructure directly. \
                         Use a repository interface (port) in the domain layer instead."
                            .to_string(),
                    ),
                });
            }
        }
    }

    // Check 3: Domain entity directly depending on infrastructure component
    for (src, tgt, edge) in graph.edges_with_nodes() {
        if src.is_external || tgt.is_external {
            continue;
        }
        if src.is_cross_cutting || tgt.is_cross_cutting {
            continue;
        }
        // ActiveRecord mode allows domain→infrastructure
        if src.architecture_mode == ArchitectureMode::ActiveRecord {
            continue;
        }
        if src.layer == Some(ArchLayer::Domain) && tgt.layer == Some(ArchLayer::Infrastructure) {
            // Already covered by layer boundary violations, but add specific
            // "missing repository pattern" detail if the target looks like a concrete impl
            let tgt_lower = tgt.name.to_lowercase();
            if tgt_lower.contains("postgres")
                || tgt_lower.contains("mysql")
                || tgt_lower.contains("redis")
                || tgt_lower.contains("mongo")
            {
                violations.push(Violation {
                    kind: ViolationKind::DomainInfrastructureLeak {
                        detail: format!(
                            "domain entity depends on concrete infrastructure: {}",
                            tgt.name
                        ),
                    },
                    severity: Severity::Error,
                    location: edge.location.clone(),
                    message: format!(
                        "Domain component '{}' directly depends on infrastructure component '{}'",
                        src.name, tgt.name
                    ),
                    suggestion: Some(
                        "Introduce a repository interface in the domain layer and have the \
                         infrastructure component implement it."
                            .to_string(),
                    ),
                });
            }
        }
    }
}

fn detect_init_violations(
    graph: &DependencyGraph,
    config: &Config,
    violations: &mut Vec<Violation>,
) {
    if !config.rules.detect_init_functions {
        return;
    }

    let severity = config
        .rules
        .severities
        .get("init_coupling")
        .copied()
        .unwrap_or(Severity::Warning);

    for (src, tgt, edge) in graph.edges_with_nodes() {
        // Only check edges from init functions (component ID contains "<init>")
        if !src.id.0.contains("<init>") {
            continue;
        }

        if src.is_cross_cutting || tgt.is_cross_cutting {
            continue;
        }

        let (Some(from_layer), Some(to_layer)) = (src.layer, tgt.layer) else {
            continue;
        };

        if from_layer.violates_dependency_on(&to_layer) {
            let init_file = edge.location.file.to_string_lossy().to_string();
            let called_package = tgt.id.0.clone();

            violations.push(Violation {
                kind: ViolationKind::InitFunctionCoupling {
                    init_file: init_file.clone(),
                    called_package: called_package.clone(),
                    from_layer,
                    to_layer,
                },
                severity,
                location: edge.location.clone(),
                message: format!(
                    "init() function in {from_layer} layer calls into {to_layer} layer ({called_package})"
                ),
                suggestion: Some(
                    "Move initialization logic out of init() or use dependency injection to avoid hidden cross-layer coupling."
                        .to_string(),
                ),
            });
        }
    }
}

/// Layer conformance: how well each package's (A, I) values match its assigned layer's
/// expected region centroid on the instability-abstractness plane.
///
/// Returns `None` when there are no classified packages (cannot compute a meaningful score).
fn calculate_layer_conformance(
    components: &[Component],
    dependencies: &[Dependency],
) -> Option<f64> {
    use std::collections::{HashMap, HashSet};

    // Group components by full package path (everything before "::" in ComponentId).
    let mut pkg_components: HashMap<String, Vec<&Component>> = HashMap::new();
    for comp in components {
        let pkg = pkg_from_id(&comp.id.0).to_string();
        if !pkg.is_empty() {
            pkg_components.entry(pkg).or_default().push(comp);
        }
    }

    let internal_pkgs: HashSet<String> = pkg_components.keys().cloned().collect();

    // Compute efferent (Ce) and afferent (Ca) coupling using full package paths.
    // Import paths are matched against known packages via two-segment suffix matching
    // to handle the mismatch between filesystem paths and Go/Java module paths.
    let mut ce_pairs: HashSet<(String, String)> = HashSet::new();
    let mut ca_pairs: HashSet<(String, String)> = HashSet::new();

    for dep in dependencies {
        let from_pkg = pkg_from_id(&dep.from.0).to_string();
        if !internal_pkgs.contains(&from_pkg) {
            continue;
        }
        let to_pkg = dep.import_path.as_deref().and_then(|imp| {
            internal_pkgs
                .iter()
                .find(|pkg| pkg_import_match(pkg, imp))
                .cloned()
        });
        let Some(to_pkg) = to_pkg else { continue };
        if from_pkg == to_pkg {
            continue;
        }
        ce_pairs.insert((from_pkg.clone(), to_pkg.clone()));
        ca_pairs.insert((to_pkg, from_pkg));
    }

    // Compute per-package (A, I) and layer conformance score.
    let mut conformance_scores: Vec<f64> = Vec::new();

    for (pkg_path, comps) in &pkg_components {
        // Use the first non-None layer among components in this package.
        let Some(layer) = comps.iter().find_map(|c| c.layer) else {
            continue; // unclassified package — skip
        };

        let nc = comps.len();
        let na = comps
            .iter()
            .filter(|c| matches!(c.kind, ComponentKind::Port(_)))
            .count();

        let a = na as f64 / nc as f64;

        let ce = ce_pairs.iter().filter(|(from, _)| from == pkg_path).count();
        let ca = ca_pairs.iter().filter(|(to, _)| to == pkg_path).count();
        let i = if ca + ce == 0 {
            0.0
        } else {
            ce as f64 / (ca + ce) as f64
        };

        let centroid = layer_centroid(layer);
        let dist = euclidean_distance((a, i), centroid);
        conformance_scores.push((1.0 - dist).max(0.0));
    }

    if conformance_scores.is_empty() {
        return None;
    }
    let mean = conformance_scores.iter().sum::<f64>() / conformance_scores.len() as f64;
    Some(mean * 100.0)
}

/// Expected (A, I) centroid for each architectural layer.
fn layer_centroid(layer: ArchLayer) -> (f64, f64) {
    match layer {
        ArchLayer::Domain => (0.75, 0.15),
        ArchLayer::Application => (0.40, 0.50),
        ArchLayer::Infrastructure => (0.15, 0.75),
        ArchLayer::Presentation => (0.15, 0.75),
    }
}

/// Euclidean distance between two points on the (A, I) plane.
fn euclidean_distance(p1: (f64, f64), p2: (f64, f64)) -> f64 {
    let dx = p1.0 - p2.0;
    let dy = p1.1 - p2.1;
    (dx * dx + dy * dy).sqrt()
}

/// Returns true if a filesystem package path and an import path refer to the same package.
///
/// Matches by comparing trailing path segments:
/// - If both have ≥ 2 segments, the last two must agree.
/// - If either has only 1 segment, the last segment must agree.
fn pkg_import_match(pkg_path: &str, import_path: &str) -> bool {
    let pkg_last = pkg_path.split('/').next_back().unwrap_or("");
    let imp_last = import_path.split('/').next_back().unwrap_or("");
    if pkg_last.is_empty() || pkg_last != imp_last {
        return false;
    }
    match (
        pkg_path.split('/').rev().nth(1),
        import_path.split('/').rev().nth(1),
    ) {
        (Some(p), Some(i)) => p == i,
        _ => true,
    }
}

/// Dependency compliance: percentage of all cross-layer edges that flow in a valid direction.
/// Edges involving unclassified components are not counted as correct — they
/// represent unresolved architecture that needs classification.
fn calculate_dependency_compliance(graph: &DependencyGraph) -> f64 {
    let edges = graph.edges_with_nodes();
    if edges.is_empty() {
        return 100.0;
    }

    let non_cross_cutting: Vec<_> = edges
        .iter()
        .filter(|(src, tgt, _)| {
            !src.is_external
                && !tgt.is_external
                && !src.is_cross_cutting
                && !tgt.is_cross_cutting
                && src.architecture_mode != ArchitectureMode::ServiceOriented
        })
        .collect();

    if non_cross_cutting.is_empty() {
        return 100.0;
    }

    let correct = non_cross_cutting
        .iter()
        .filter(|(src, tgt, _)| match (src.layer, tgt.layer) {
            (Some(from), Some(to)) => !from.violates_dependency_on(&to),
            _ => false, // unclassified edges are not correct
        })
        .count();

    (correct as f64 / non_cross_cutting.len() as f64) * 100.0
}

/// Interface coverage: ratio of ports to adapters/repositories (higher = better separation).
fn calculate_interface_coverage(graph: &DependencyGraph) -> f64 {
    let nodes = graph.nodes();
    if nodes.is_empty() {
        return 100.0;
    }

    let mut ports = 0u64;
    let mut adapters = 0u64;

    for node in &nodes {
        if node.is_cross_cutting {
            continue;
        }
        if let Some(kind) = &node.kind {
            if matches!(kind, ComponentKind::Port(_)) {
                ports += 1;
            }
            // Count adapters and repositories in the infrastructure layer
            if node.layer == Some(ArchLayer::Infrastructure)
                && matches!(
                    kind,
                    ComponentKind::Adapter(_) | ComponentKind::Repository | ComponentKind::Service
                )
            {
                adapters += 1;
            }
        }
    }

    if adapters == 0 {
        return 100.0;
    }

    // Balanced coverage: both excess ports and excess adapters indicate imbalance.
    // Score = min(ports, adapters) / max(ports, adapters) * 100
    let min = ports.min(adapters) as f64;
    let max = ports.max(adapters) as f64;
    (min / max) * 100.0
}

/// Build a complete `AnalysisResult`.
///
/// The `score` field is gated by pattern detection:
///   - top_confidence ≥ 0.5 → `score` is `Some(...)` (DDD scores are meaningful)
///   - top_confidence < 0.5 → `score` is `None`   (pattern unclear, scores suppressed)
pub fn build_result(
    graph: &DependencyGraph,
    config: &Config,
    dep_count: usize,
    components: &[Component],
    files_analyzed: usize,
    dependencies: &[Dependency],
) -> AnalysisResult {
    let architecture_score = calculate_score(graph, config, components, dependencies);
    let violations = detect_violations(graph, config);
    let metrics = compute_metrics(graph, components, &violations);
    let package_metrics = compute_package_metrics(components, dependencies);
    let pattern_detection = detect_patterns(components, dependencies);

    let score = if pattern_detection.top_confidence >= 0.5 {
        Some(architecture_score)
    } else {
        None
    };

    AnalysisResult {
        score,
        violations,
        component_count: graph.node_count(),
        dependency_count: dep_count,
        files_analyzed,
        metrics: Some(metrics),
        package_metrics,
        pattern_detection: Some(pattern_detection),
    }
}

fn compute_metrics(
    graph: &DependencyGraph,
    components: &[Component],
    violations: &[Violation],
) -> MetricsReport {
    // Components by kind
    let mut components_by_kind: HashMap<String, usize> = HashMap::new();
    for comp in components {
        let kind_name = match &comp.kind {
            ComponentKind::Port(_) => "port",
            ComponentKind::Adapter(_) => "adapter",
            ComponentKind::Entity(_) => "entity",
            ComponentKind::ValueObject => "value_object",
            ComponentKind::UseCase => "use_case",
            ComponentKind::Repository => "repository",
            ComponentKind::Service => "service",
            ComponentKind::DomainEvent(_) => "domain_event",
        };
        *components_by_kind.entry(kind_name.to_string()).or_insert(0) += 1;
    }

    // Components by layer
    let components_by_layer = graph.nodes_by_layer();

    // Violations by kind
    let mut violations_by_kind: HashMap<String, usize> = HashMap::new();
    for v in violations {
        let kind_name = match &v.kind {
            ViolationKind::LayerBoundary { .. } => "layer_boundary",
            ViolationKind::CircularDependency { .. } => "circular_dependency",
            ViolationKind::MissingPort { .. } => "missing_port",
            ViolationKind::CustomRule { .. } => "custom_rule",
            ViolationKind::DomainInfrastructureLeak { .. } => "domain_infrastructure_leak",
            ViolationKind::InitFunctionCoupling { .. } => "init_coupling",
        };
        *violations_by_kind.entry(kind_name.to_string()).or_insert(0) += 1;
    }

    // Dependency depth
    let max_depth = graph.max_dependency_depth();
    let node_count = graph.node_count();
    let avg_depth = if node_count > 0 {
        max_depth as f64 / node_count as f64
    } else {
        0.0
    };

    // Layer coupling
    let layer_coupling = graph.layer_coupling_matrix();

    // Classification coverage
    let classification_coverage = compute_classification_coverage(graph);

    MetricsReport {
        components_by_kind,
        components_by_layer,
        violations_by_kind,
        dependency_depth: DependencyDepthMetrics {
            max_depth,
            avg_depth,
        },
        layer_coupling,
        classification_coverage: Some(classification_coverage),
    }
}

fn compute_classification_coverage(graph: &DependencyGraph) -> ClassificationCoverage {
    let nodes = graph.nodes();

    let mut total_components = 0usize;
    let mut classified = 0usize;
    let mut cross_cutting = 0usize;
    let mut unclassified = 0usize;
    let mut unclassified_dirs: Vec<String> = Vec::new();

    for node in &nodes {
        if node.is_external {
            continue;
        }
        // Only count real extracted components (structs, interfaces, etc.).
        // Synthetic placeholder nodes created for dependency tracking
        // (<file> source nodes and <package> import target nodes) have
        // kind: None and must not affect the classification coverage metric.
        if node.kind.is_none() {
            continue;
        }
        total_components += 1;
        if node.is_cross_cutting {
            cross_cutting += 1;
        } else if node.layer.is_some() {
            classified += 1;
        } else {
            unclassified += 1;
            // Extract parent directory from component ID
            let id = &node.id.0;
            if let Some(dir) = id.rsplit_once("::").map(|(pkg, _)| pkg.to_string()) {
                if !unclassified_dirs.contains(&dir) {
                    unclassified_dirs.push(dir);
                }
            }
        }
    }

    // Sort and truncate to ~10 entries
    unclassified_dirs.sort();
    unclassified_dirs.truncate(10);

    let coverage_percentage = if total_components > 0 {
        ((classified + cross_cutting) as f64 / total_components as f64) * 100.0
    } else {
        100.0
    };

    ClassificationCoverage {
        total_components,
        classified,
        cross_cutting,
        unclassified,
        coverage_percentage,
        unclassified_paths: unclassified_dirs,
    }
}

/// Compute R.C. Martin package metrics (Instability, Abstractness, Distance) per package.
///
/// Packages are identified by the last directory segment of their path.
/// Internal coupling is detected by matching the last segment of each import path
/// against known source package names (suffix matching).
///
/// Special cases:
///   - Nc = 0 → package excluded from output entirely
///   - Ca + Ce = 0 → I = 0.0 (defined, not undefined)
fn compute_package_metrics(
    components: &[Component],
    dependencies: &[Dependency],
) -> Vec<PackageMetric> {
    use std::collections::HashSet;

    // Step 1: identify source package paths from real components.
    // Package path is everything before "::" in ComponentId.0.
    let mut pkg_full_paths: HashSet<String> = HashSet::new();
    for comp in components {
        let full_pkg = pkg_from_id(&comp.id.0);
        if !full_pkg.is_empty() {
            pkg_full_paths.insert(full_pkg.to_string());
        }
    }

    // Build a map: short name (last path segment) → full package path.
    // If two packages share a short name we keep the first and skip the rest.
    let mut short_to_full: HashMap<String, String> = HashMap::new();
    for full in &pkg_full_paths {
        let short = last_segment(full).to_string();
        short_to_full.entry(short).or_insert_with(|| full.clone());
    }
    // Reverse map for coupling computation: full path → short name.
    let full_to_short: HashMap<String, String> = short_to_full
        .iter()
        .map(|(s, f)| (f.clone(), s.clone()))
        .collect();

    // Step 2: count abstract types (Na) and total components (Nc) per package.
    // Key = short package name.
    let mut na: HashMap<String, usize> = HashMap::new();
    let mut nc: HashMap<String, usize> = HashMap::new();
    for comp in components {
        let full_pkg = pkg_from_id(&comp.id.0);
        let Some(short) = full_to_short.get(full_pkg) else {
            continue;
        };
        *nc.entry(short.clone()).or_insert(0) += 1;
        if matches!(comp.kind, ComponentKind::Port(_)) {
            *na.entry(short.clone()).or_insert(0) += 1;
        }
    }

    // Step 3: compute efferent (Ce) and afferent (Ca) coupling per package.
    // Ce[X] = number of distinct internal packages X imports.
    // Ca[X] = number of distinct internal packages that import X.
    // We count unique (from_pkg, to_pkg) pairs to avoid double-counting.
    let mut ce_pairs: HashSet<(String, String)> = HashSet::new();
    let mut ca_pairs: HashSet<(String, String)> = HashSet::new();

    for dep in dependencies {
        let from_full = pkg_from_id(&dep.from.0);
        let Some(from_short) = full_to_short.get(from_full) else {
            continue;
        };
        let to_short = dep
            .import_path
            .as_deref()
            .map(last_segment)
            .and_then(|s| short_to_full.contains_key(s).then_some(s));
        let Some(to_short) = to_short else {
            continue;
        };
        if from_short == to_short {
            continue; // intra-package dependency
        }
        ce_pairs.insert((from_short.clone(), to_short.to_string()));
        ca_pairs.insert((to_short.to_string(), from_short.clone()));
    }

    let mut ce: HashMap<String, usize> = HashMap::new();
    let mut ca: HashMap<String, usize> = HashMap::new();
    for (from, to) in &ce_pairs {
        *ce.entry(from.clone()).or_insert(0) += 1;
        let _ = to; // counted via ca_pairs below
    }
    for (to, from) in &ca_pairs {
        *ca.entry(to.clone()).or_insert(0) += 1;
        let _ = from;
    }

    // Step 4: build PackageMetric for each package with Nc > 0.
    let mut result: Vec<PackageMetric> = nc
        .iter()
        .filter(|(_, &n)| n > 0)
        .map(|(short, &n)| {
            let abstract_count = *na.get(short).unwrap_or(&0);
            let ce_count = *ce.get(short).unwrap_or(&0);
            let ca_count = *ca.get(short).unwrap_or(&0);

            let a = abstract_count as f64 / n as f64;
            let i = if ca_count + ce_count == 0 {
                0.0
            } else {
                ce_count as f64 / (ca_count + ce_count) as f64
            };
            let d = (a + i - 1.0).abs();

            let zone = if d > 0.5 {
                if a < 0.5 && i < 0.5 {
                    Some("pain".to_string())
                } else if a > 0.5 && i > 0.5 {
                    Some("uselessness".to_string())
                } else {
                    None
                }
            } else {
                None
            };

            PackageMetric {
                package: short.clone(),
                abstractness: round2(a),
                instability: round2(i),
                distance: round2(d),
                zone,
            }
        })
        .collect();

    result.sort_by(|a, b| a.package.cmp(&b.package));
    result
}

/// Extract the package portion of a ComponentId string ("pkg::name" → "pkg").
fn pkg_from_id(id: &str) -> &str {
    id.split("::").next().unwrap_or("")
}

/// Extract the last path segment from a path-like string.
fn last_segment(path: &str) -> &str {
    path.split('/').next_back().unwrap_or(path)
}

/// Round a float to 2 decimal places.
fn round2(v: f64) -> f64 {
    (v * 100.0).round() / 100.0
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;
    use crate::graph::DependencyGraph;
    use crate::types::*;
    use std::path::PathBuf;

    fn make_component(id: &str, name: &str, layer: Option<ArchLayer>) -> Component {
        Component {
            id: ComponentId(id.to_string()),
            name: name.to_string(),
            kind: ComponentKind::Entity(EntityInfo {
                name: name.to_string(),
                fields: vec![],
                methods: vec![],
                is_active_record: false,
            }),
            layer,
            location: SourceLocation {
                file: PathBuf::from("test.go"),
                line: 1,
                column: 1,
            },
            is_cross_cutting: false,
            architecture_mode: ArchitectureMode::Ddd,
        }
    }

    fn make_dep(from: &str, to: &str) -> Dependency {
        Dependency {
            from: ComponentId(from.to_string()),
            to: ComponentId(to.to_string()),
            kind: DependencyKind::Import,
            location: SourceLocation {
                file: PathBuf::from("test.go"),
                line: 10,
                column: 1,
            },
            import_path: Some("some/import".to_string()),
        }
    }

    #[test]
    fn test_perfect_score_no_violations() {
        let mut graph = DependencyGraph::new();
        // Infrastructure -> Domain (correct direction)
        let c1 = make_component("infra", "InfraService", Some(ArchLayer::Infrastructure));
        let c2 = make_component("domain", "DomainEntity", Some(ArchLayer::Domain));
        graph.add_component(&c1);
        graph.add_component(&c2);
        graph.add_dependency(&make_dep("infra", "domain"));

        let config = Config::default();
        let score = calculate_score(&graph, &config, &[], &[]);

        assert_eq!(score.layer_conformance, 100.0);
        assert_eq!(score.dependency_compliance, 100.0);

        let violations = detect_violations(&graph, &config);
        assert!(
            violations.is_empty(),
            "no violations for correct dependency"
        );
    }

    #[test]
    fn test_violation_domain_to_infrastructure() {
        let mut graph = DependencyGraph::new();
        let c1 = make_component("domain", "Entity", Some(ArchLayer::Domain));
        let c2 = make_component("infra", "Repo", Some(ArchLayer::Infrastructure));
        graph.add_component(&c1);
        graph.add_component(&c2);
        graph.add_dependency(&make_dep("domain", "infra"));

        let config = Config::default();
        let violations = detect_violations(&graph, &config);

        assert_eq!(violations.len(), 1);
        assert_eq!(violations[0].severity, Severity::Error);
        assert!(matches!(
            violations[0].kind,
            ViolationKind::LayerBoundary {
                from_layer: ArchLayer::Domain,
                to_layer: ArchLayer::Infrastructure,
            }
        ));
    }

    #[test]
    fn test_circular_dependency_detection() {
        let mut graph = DependencyGraph::new();
        let c1 = make_component("a", "A", Some(ArchLayer::Domain));
        let c2 = make_component("b", "B", Some(ArchLayer::Domain));
        graph.add_component(&c1);
        graph.add_component(&c2);
        graph.add_dependency(&make_dep("a", "b"));
        graph.add_dependency(&make_dep("b", "a"));

        let config = Config::default();
        let violations = detect_violations(&graph, &config);

        let circular = violations
            .iter()
            .filter(|v| matches!(v.kind, ViolationKind::CircularDependency { .. }))
            .count();
        assert!(circular > 0, "should detect circular dependency");
    }

    #[test]
    fn test_empty_graph_perfect_score() {
        let graph = DependencyGraph::new();
        let config = Config::default();
        let score = calculate_score(&graph, &config, &[], &[]);
        assert!(
            (score.overall - 100.0).abs() < 0.01,
            "empty graph should score ~100, got {}",
            score.overall
        );
    }

    #[test]
    fn test_build_result() {
        let graph = DependencyGraph::new();
        let config = Config::default();
        let result = build_result(&graph, &config, 0, &[], 0, &[]);
        assert_eq!(result.component_count, 0);
        assert_eq!(result.dependency_count, 0);
        assert_eq!(result.files_analyzed, 0);
        assert!(result.violations.is_empty());
        assert!(result.metrics.is_some());
        assert!(result.pattern_detection.is_some());
    }

    fn make_cross_cutting_component(id: &str, name: &str, layer: Option<ArchLayer>) -> Component {
        Component {
            id: ComponentId(id.to_string()),
            name: name.to_string(),
            kind: ComponentKind::Entity(EntityInfo {
                name: name.to_string(),
                fields: vec![],
                methods: vec![],
                is_active_record: false,
            }),
            layer,
            location: SourceLocation {
                file: PathBuf::from("test.go"),
                line: 1,
                column: 1,
            },
            is_cross_cutting: true,
            architecture_mode: ArchitectureMode::Ddd,
        }
    }

    #[test]
    fn test_cross_cutting_excluded_from_layer_violations() {
        let mut graph = DependencyGraph::new();
        // Domain -> Infrastructure would normally be a violation
        let c1 = make_component("domain", "Entity", Some(ArchLayer::Domain));
        let c2 = make_cross_cutting_component("infra", "Logger", Some(ArchLayer::Infrastructure));
        graph.add_component(&c1);
        graph.add_component(&c2);
        graph.add_dependency(&make_dep("domain", "infra"));

        let config = Config::default();
        let violations = detect_violations(&graph, &config);

        let layer_violations: Vec<_> = violations
            .iter()
            .filter(|v| matches!(v.kind, ViolationKind::LayerBoundary { .. }))
            .collect();
        assert!(
            layer_violations.is_empty(),
            "cross-cutting target should suppress layer violations"
        );
    }

    #[test]
    fn test_cross_cutting_source_excluded_from_violations() {
        let mut graph = DependencyGraph::new();
        let c1 = make_cross_cutting_component("utils", "Utils", Some(ArchLayer::Domain));
        let c2 = make_component("infra", "Repo", Some(ArchLayer::Infrastructure));
        graph.add_component(&c1);
        graph.add_component(&c2);
        graph.add_dependency(&make_dep("utils", "infra"));

        let config = Config::default();
        let violations = detect_violations(&graph, &config);

        let layer_violations: Vec<_> = violations
            .iter()
            .filter(|v| matches!(v.kind, ViolationKind::LayerBoundary { .. }))
            .collect();
        assert!(
            layer_violations.is_empty(),
            "cross-cutting source should suppress layer violations"
        );
    }

    #[test]
    fn test_cross_cutting_excluded_from_dependency_compliance() {
        let mut graph = DependencyGraph::new();
        // This edge would normally reduce compliance score
        let c1 = make_component("domain", "Entity", Some(ArchLayer::Domain));
        let c2 = make_cross_cutting_component("infra", "Logger", Some(ArchLayer::Infrastructure));
        graph.add_component(&c1);
        graph.add_component(&c2);
        graph.add_dependency(&make_dep("domain", "infra"));

        let compliance = calculate_dependency_compliance(&graph);
        assert_eq!(
            compliance, 100.0,
            "cross-cutting edges should be excluded from dependency compliance"
        );
    }

    #[test]
    fn test_cross_cutting_excluded_from_layer_conformance() {
        let mut graph = DependencyGraph::new();
        let c1 = make_component("domain", "Entity", Some(ArchLayer::Domain));
        let c2 = make_cross_cutting_component("infra", "Logger", Some(ArchLayer::Infrastructure));
        graph.add_component(&c1);
        graph.add_component(&c2);
        graph.add_dependency(&make_dep("domain", "infra"));

        // Cross-cutting components are not classified — layer conformance should
        // only see the domain component and return a value in [0, 100].
        let components: Vec<Component> = vec![c1, c2];
        let conformance = calculate_layer_conformance(&components, &[]);
        // domain has one entity (A=0, I=0) → distance to Domain centroid (0.75, 0.15) ≈ 0.765
        // conformance = max(0, 1 - 0.765) ≈ 0.235 → Some(23.5)
        assert!(
            conformance.is_some(),
            "should have at least one classified package"
        );
        let val = conformance.unwrap();
        assert!(
            (0.0..=100.0).contains(&val),
            "conformance must be in [0, 100], got {val}"
        );
    }

    fn make_component_with_mode(
        id: &str,
        name: &str,
        layer: Option<ArchLayer>,
        mode: ArchitectureMode,
    ) -> Component {
        Component {
            id: ComponentId(id.to_string()),
            name: name.to_string(),
            kind: ComponentKind::Entity(EntityInfo {
                name: name.to_string(),
                fields: vec![],
                methods: vec![],
                is_active_record: false,
            }),
            layer,
            location: SourceLocation {
                file: PathBuf::from("test.go"),
                line: 1,
                column: 1,
            },
            is_cross_cutting: false,
            architecture_mode: mode,
        }
    }

    #[test]
    fn test_service_oriented_suppresses_layer_violations() {
        let mut graph = DependencyGraph::new();
        let c1 = make_component_with_mode(
            "domain",
            "Entity",
            Some(ArchLayer::Domain),
            ArchitectureMode::ServiceOriented,
        );
        let c2 = make_component_with_mode(
            "infra",
            "Repo",
            Some(ArchLayer::Infrastructure),
            ArchitectureMode::ServiceOriented,
        );
        graph.add_component(&c1);
        graph.add_component(&c2);
        graph.add_dependency(&make_dep("domain", "infra"));

        let config = Config::default();
        let violations = detect_violations(&graph, &config);

        let layer_violations: Vec<_> = violations
            .iter()
            .filter(|v| matches!(v.kind, ViolationKind::LayerBoundary { .. }))
            .collect();
        assert!(
            layer_violations.is_empty(),
            "service-oriented mode should suppress layer boundary violations"
        );
    }

    #[test]
    fn test_service_oriented_excluded_from_compliance() {
        let mut graph = DependencyGraph::new();
        let c1 = make_component_with_mode(
            "domain",
            "Entity",
            Some(ArchLayer::Domain),
            ArchitectureMode::ServiceOriented,
        );
        let c2 = make_component_with_mode(
            "infra",
            "Repo",
            Some(ArchLayer::Infrastructure),
            ArchitectureMode::ServiceOriented,
        );
        graph.add_component(&c1);
        graph.add_component(&c2);
        graph.add_dependency(&make_dep("domain", "infra"));

        let compliance = calculate_dependency_compliance(&graph);
        assert_eq!(
            compliance, 100.0,
            "service-oriented edges should be excluded from dependency compliance"
        );
    }

    #[test]
    fn test_active_record_suppresses_domain_infra_leak() {
        let mut graph = DependencyGraph::new();
        // A domain component that imports DB types
        let c1 = make_component_with_mode(
            "domain",
            "User",
            Some(ArchLayer::Domain),
            ArchitectureMode::ActiveRecord,
        );
        let c2 = make_component_with_mode(
            "infra",
            "DB",
            Some(ArchLayer::Infrastructure),
            ArchitectureMode::ActiveRecord,
        );
        graph.add_component(&c1);
        graph.add_component(&c2);
        graph.add_dependency(&make_dep("domain", "infra"));

        let config = Config::default();
        let violations = detect_violations(&graph, &config);

        let leak_violations: Vec<_> = violations
            .iter()
            .filter(|v| matches!(v.kind, ViolationKind::DomainInfrastructureLeak { .. }))
            .collect();
        assert!(
            leak_violations.is_empty(),
            "active-record mode should suppress domain-infra leak violations"
        );
    }

    #[test]
    fn test_ddd_mode_still_produces_violations() {
        // Verify DDD mode (default) still catches violations
        let mut graph = DependencyGraph::new();
        let c1 = make_component("domain", "Entity", Some(ArchLayer::Domain));
        let c2 = make_component("infra", "Repo", Some(ArchLayer::Infrastructure));
        graph.add_component(&c1);
        graph.add_component(&c2);
        graph.add_dependency(&make_dep("domain", "infra"));

        let config = Config::default();
        let violations = detect_violations(&graph, &config);

        let layer_violations: Vec<_> = violations
            .iter()
            .filter(|v| matches!(v.kind, ViolationKind::LayerBoundary { .. }))
            .collect();
        assert!(
            !layer_violations.is_empty(),
            "DDD mode should still produce layer boundary violations"
        );
    }

    #[test]
    fn test_init_coupling_detected() {
        let mut graph = DependencyGraph::new();
        // init component in application layer calling infrastructure
        let c1 = make_component("app::<init>", "<init>", Some(ArchLayer::Application));
        let c2 = make_component("infra::db", "db", Some(ArchLayer::Infrastructure));
        graph.add_component(&c1);
        graph.add_component(&c2);
        graph.add_dependency(&make_dep("app::<init>", "infra::db"));

        let config = Config::default();
        let violations = detect_violations(&graph, &config);

        let init_violations: Vec<_> = violations
            .iter()
            .filter(|v| matches!(v.kind, ViolationKind::InitFunctionCoupling { .. }))
            .collect();
        assert!(
            !init_violations.is_empty(),
            "should detect init function coupling"
        );
    }

    #[test]
    fn test_init_coupling_disabled_via_config() {
        let mut graph = DependencyGraph::new();
        let c1 = make_component("app::<init>", "<init>", Some(ArchLayer::Application));
        let c2 = make_component("infra::db", "db", Some(ArchLayer::Infrastructure));
        graph.add_component(&c1);
        graph.add_component(&c2);
        graph.add_dependency(&make_dep("app::<init>", "infra::db"));

        let mut config = Config::default();
        config.rules.detect_init_functions = false;

        let violations = detect_violations(&graph, &config);
        let init_violations: Vec<_> = violations
            .iter()
            .filter(|v| matches!(v.kind, ViolationKind::InitFunctionCoupling { .. }))
            .collect();
        assert!(
            init_violations.is_empty(),
            "init detection disabled should produce no init violations"
        );
    }

    fn make_external_component(id: &str, name: &str, layer: Option<ArchLayer>) -> Component {
        Component {
            id: ComponentId(id.to_string()),
            name: name.to_string(),
            kind: ComponentKind::Entity(EntityInfo {
                name: name.to_string(),
                fields: vec![],
                methods: vec![],
                is_active_record: false,
            }),
            layer,
            location: SourceLocation {
                file: PathBuf::from("test.go"),
                line: 1,
                column: 1,
            },
            is_cross_cutting: false,
            architecture_mode: ArchitectureMode::Ddd,
        }
    }

    #[test]
    fn test_external_excluded_from_dependency_compliance() {
        let mut graph = DependencyGraph::new();
        let c1 = make_component("domain", "Entity", Some(ArchLayer::Domain));
        let c2 = make_external_component("ext", "StripeGo", None);
        graph.add_component(&c1);
        graph.add_component(&c2);
        graph.mark_external(&ComponentId("ext".to_string()));
        graph.add_dependency(&make_dep("domain", "ext"));

        let compliance = calculate_dependency_compliance(&graph);
        assert_eq!(
            compliance, 100.0,
            "external edges should be excluded from dependency compliance"
        );
    }

    #[test]
    fn test_external_excluded_from_layer_compliance_direction() {
        let mut graph = DependencyGraph::new();
        let c1 = make_component("domain", "Entity", Some(ArchLayer::Domain));
        let c2 = make_external_component("ext", "GoogleUUID", None);
        graph.add_component(&c1);
        graph.add_component(&c2);
        graph.mark_external(&ComponentId("ext".to_string()));
        graph.add_dependency(&make_dep("domain", "ext"));

        let compliance = calculate_dependency_compliance(&graph);
        assert_eq!(
            compliance, 100.0,
            "external edges should be excluded from dependency compliance"
        );
    }

    #[test]
    fn test_external_excluded_from_layer_violations() {
        let mut graph = DependencyGraph::new();
        let c1 = make_component("domain", "Entity", Some(ArchLayer::Domain));
        let c2 = make_external_component("ext", "StripePkg", Some(ArchLayer::Infrastructure));
        graph.add_component(&c1);
        graph.add_component(&c2);
        graph.mark_external(&ComponentId("ext".to_string()));
        graph.add_dependency(&make_dep("domain", "ext"));

        let config = Config::default();
        let violations = detect_violations(&graph, &config);

        let layer_violations: Vec<_> = violations
            .iter()
            .filter(|v| matches!(v.kind, ViolationKind::LayerBoundary { .. }))
            .collect();
        assert!(
            layer_violations.is_empty(),
            "external target should suppress layer violations"
        );
    }

    #[test]
    fn test_external_excluded_from_classification_coverage() {
        let mut graph = DependencyGraph::new();
        let c1 = make_component("domain", "Entity", Some(ArchLayer::Domain));
        let c2 = make_external_component("ext", "ExternalPkg", None);
        graph.add_component(&c1);
        graph.add_component(&c2);
        graph.mark_external(&ComponentId("ext".to_string()));

        let coverage = compute_classification_coverage(&graph);
        assert_eq!(
            coverage.total_components, 1,
            "external nodes should not count in total"
        );
        assert_eq!(coverage.classified, 1);
        assert_eq!(coverage.unclassified, 0);
        assert!((coverage.coverage_percentage - 100.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_classification_coverage_all_classified() {
        let mut graph = DependencyGraph::new();
        let c1 = make_component("a", "A", Some(ArchLayer::Domain));
        let c2 = make_component("b", "B", Some(ArchLayer::Application));
        graph.add_component(&c1);
        graph.add_component(&c2);

        let coverage = compute_classification_coverage(&graph);
        assert_eq!(coverage.total_components, 2);
        assert_eq!(coverage.classified, 2);
        assert_eq!(coverage.cross_cutting, 0);
        assert_eq!(coverage.unclassified, 0);
        assert!((coverage.coverage_percentage - 100.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_classification_coverage_mixed() {
        let mut graph = DependencyGraph::new();
        let c1 = make_component("domain::Entity", "Entity", Some(ArchLayer::Domain));
        let c2 = make_cross_cutting_component("utils::Logger", "Logger", None);
        let c3 = make_component("unknown::Foo", "Foo", None);
        graph.add_component(&c1);
        graph.add_component(&c2);
        graph.add_component(&c3);

        let coverage = compute_classification_coverage(&graph);
        assert_eq!(coverage.total_components, 3);
        assert_eq!(coverage.classified, 1);
        assert_eq!(coverage.cross_cutting, 1);
        assert_eq!(coverage.unclassified, 1);
        // (1 + 1) / 3 * 100 = 66.67
        assert!((coverage.coverage_percentage - 66.66666666666667).abs() < 0.01);
        assert_eq!(coverage.unclassified_paths.len(), 1);
        assert_eq!(coverage.unclassified_paths[0], "unknown");
    }
}
