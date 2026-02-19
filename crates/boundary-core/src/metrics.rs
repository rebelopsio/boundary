use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::config::Config;
use crate::graph::DependencyGraph;
use crate::metrics_report::{ClassificationCoverage, DependencyDepthMetrics, MetricsReport};
use crate::types::{
    ArchLayer, ArchitectureMode, Component, ComponentKind, Severity, Violation, ViolationKind,
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
            score: ArchitectureScore {
                overall: 100.0,
                structural_presence: 100.0,
                layer_isolation: 100.0,
                dependency_direction: 100.0,
                interface_coverage: 100.0,
            },
            violations: vec![],
            component_count: 0,
            dependency_count: 0,
            metrics: None,
        };
    }

    let total_components: usize = services.iter().map(|s| s.result.component_count).sum();
    let total_deps: usize = services.iter().map(|s| s.result.dependency_count).sum();

    // Weighted average by component count
    let mut overall = 0.0f64;
    let mut structural_presence = 0.0f64;
    let mut layer_isolation = 0.0f64;
    let mut dependency_direction = 0.0f64;
    let mut interface_coverage = 0.0f64;

    if total_components > 0 {
        for s in services {
            let weight = s.result.component_count as f64 / total_components as f64;
            overall += s.result.score.overall * weight;
            structural_presence += s.result.score.structural_presence * weight;
            layer_isolation += s.result.score.layer_isolation * weight;
            dependency_direction += s.result.score.dependency_direction * weight;
            interface_coverage += s.result.score.interface_coverage * weight;
        }
    }

    let all_violations: Vec<_> = services
        .iter()
        .flat_map(|s| s.result.violations.clone())
        .collect();

    AnalysisResult {
        score: ArchitectureScore {
            overall,
            structural_presence,
            layer_isolation,
            dependency_direction,
            interface_coverage,
        },
        violations: all_violations,
        component_count: total_components,
        dependency_count: total_deps,
        metrics: None,
    }
}

/// Breakdown of architecture scores.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArchitectureScore {
    pub overall: f64,
    pub structural_presence: f64,
    pub layer_isolation: f64,
    pub dependency_direction: f64,
    pub interface_coverage: f64,
}

/// Full analysis result.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnalysisResult {
    pub score: ArchitectureScore,
    pub violations: Vec<Violation>,
    pub component_count: usize,
    pub dependency_count: usize,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub metrics: Option<MetricsReport>,
}

/// Calculate architecture score from the dependency graph.
pub fn calculate_score(graph: &DependencyGraph, config: &Config) -> ArchitectureScore {
    let layer_isolation = calculate_layer_isolation(graph);
    let dependency_direction = calculate_dependency_direction(graph);
    let interface_coverage = calculate_interface_coverage(graph);

    let w = &config.scoring;
    let correctness = layer_isolation * w.layer_isolation_weight
        + dependency_direction * w.dependency_direction_weight
        + interface_coverage * w.interface_coverage_weight;
    let correctness = correctness.clamp(0.0, 100.0);

    // Structural presence: what % of components are classified into a layer?
    let coverage = compute_classification_coverage(graph);
    let structural_presence = coverage.coverage_percentage;

    // Multiplicative gate: overall = presence * correctness / 100
    let overall = (structural_presence * correctness / 100.0).clamp(0.0, 100.0);

    ArchitectureScore {
        overall,
        structural_presence,
        layer_isolation,
        dependency_direction,
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

        // Service-oriented mode skips all layer boundary checks
        if src.architecture_mode == ArchitectureMode::ServiceOriented {
            continue;
        }

        let (Some(from_layer), Some(to_layer)) = (src.layer, tgt.layer) else {
            continue;
        };

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

/// Layer isolation: percentage of cross-layer edges that go in the correct direction.
/// Edges involving unclassified components count against isolation since they
/// represent components that haven't been properly placed in a layer.
fn calculate_layer_isolation(graph: &DependencyGraph) -> f64 {
    let edges = graph.edges_with_nodes();
    if edges.is_empty() {
        return 100.0;
    }

    let mut total = 0u64;
    let mut correct = 0u64;

    for (src, tgt, _) in &edges {
        if src.is_external || tgt.is_external {
            continue;
        }
        if src.is_cross_cutting || tgt.is_cross_cutting {
            continue;
        }
        // Service-oriented mode is exempt from isolation scoring
        if src.architecture_mode == ArchitectureMode::ServiceOriented {
            continue;
        }
        match (src.layer, tgt.layer) {
            (Some(from_layer), Some(to_layer)) => {
                if from_layer == to_layer {
                    // Same-layer edges are fine, don't count them
                    continue;
                }
                total += 1;
                if !from_layer.violates_dependency_on(&to_layer) {
                    correct += 1;
                }
            }
            _ => {
                // Edges involving unclassified components penalize isolation
                total += 1;
            }
        }
    }

    if total == 0 {
        return 100.0;
    }
    (correct as f64 / total as f64) * 100.0
}

/// Dependency direction: percentage of all edges that flow in a valid direction.
/// Edges involving unclassified components are not counted as correct — they
/// represent unresolved architecture that needs classification.
fn calculate_dependency_direction(graph: &DependencyGraph) -> f64 {
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

    // Ideal: every adapter has a port. Score = min(ports/adapters, 1.0) * 100
    let ratio = (ports as f64 / adapters as f64).min(1.0);
    ratio * 100.0
}

/// Build a complete `AnalysisResult`.
pub fn build_result(
    graph: &DependencyGraph,
    config: &Config,
    dep_count: usize,
    components: &[Component],
) -> AnalysisResult {
    let score = calculate_score(graph, config);
    let violations = detect_violations(graph, config);

    let metrics = compute_metrics(graph, components, &violations);

    AnalysisResult {
        score,
        violations,
        component_count: graph.node_count(),
        dependency_count: dep_count,
        metrics: Some(metrics),
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
        let score = calculate_score(&graph, &config);

        assert_eq!(score.layer_isolation, 100.0);
        assert_eq!(score.dependency_direction, 100.0);

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
        let score = calculate_score(&graph, &config);
        assert_eq!(score.overall, 100.0);
    }

    #[test]
    fn test_build_result() {
        let graph = DependencyGraph::new();
        let config = Config::default();
        let result = build_result(&graph, &config, 0, &[]);
        assert_eq!(result.component_count, 0);
        assert_eq!(result.dependency_count, 0);
        assert!(result.violations.is_empty());
        assert!(result.metrics.is_some());
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
    fn test_cross_cutting_excluded_from_layer_isolation() {
        let mut graph = DependencyGraph::new();
        // This edge would normally reduce isolation score
        let c1 = make_component("domain", "Entity", Some(ArchLayer::Domain));
        let c2 = make_cross_cutting_component("infra", "Logger", Some(ArchLayer::Infrastructure));
        graph.add_component(&c1);
        graph.add_component(&c2);
        graph.add_dependency(&make_dep("domain", "infra"));

        let isolation = calculate_layer_isolation(&graph);
        assert_eq!(
            isolation, 100.0,
            "cross-cutting edges should be excluded from isolation"
        );
    }

    #[test]
    fn test_cross_cutting_excluded_from_dependency_direction() {
        let mut graph = DependencyGraph::new();
        let c1 = make_component("domain", "Entity", Some(ArchLayer::Domain));
        let c2 = make_cross_cutting_component("infra", "Logger", Some(ArchLayer::Infrastructure));
        graph.add_component(&c1);
        graph.add_component(&c2);
        graph.add_dependency(&make_dep("domain", "infra"));

        let direction = calculate_dependency_direction(&graph);
        assert_eq!(
            direction, 100.0,
            "cross-cutting edges should be excluded from dependency direction"
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
    fn test_service_oriented_excluded_from_isolation() {
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

        let isolation = calculate_layer_isolation(&graph);
        assert_eq!(
            isolation, 100.0,
            "service-oriented edges should be excluded from isolation"
        );
    }

    #[test]
    fn test_service_oriented_excluded_from_direction() {
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

        let direction = calculate_dependency_direction(&graph);
        assert_eq!(
            direction, 100.0,
            "service-oriented edges should be excluded from dependency direction"
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
    fn test_external_excluded_from_layer_isolation() {
        let mut graph = DependencyGraph::new();
        let c1 = make_component("domain", "Entity", Some(ArchLayer::Domain));
        let c2 = make_external_component("ext", "StripeGo", None);
        graph.add_component(&c1);
        graph.add_component(&c2);
        graph.mark_external(&ComponentId("ext".to_string()));
        graph.add_dependency(&make_dep("domain", "ext"));

        let isolation = calculate_layer_isolation(&graph);
        assert_eq!(
            isolation, 100.0,
            "external edges should be excluded from isolation"
        );
    }

    #[test]
    fn test_external_excluded_from_dependency_direction() {
        let mut graph = DependencyGraph::new();
        let c1 = make_component("domain", "Entity", Some(ArchLayer::Domain));
        let c2 = make_external_component("ext", "GoogleUUID", None);
        graph.add_component(&c1);
        graph.add_component(&c2);
        graph.mark_external(&ComponentId("ext".to_string()));
        graph.add_dependency(&make_dep("domain", "ext"));

        let direction = calculate_dependency_direction(&graph);
        assert_eq!(
            direction, 100.0,
            "external edges should be excluded from dependency direction"
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
