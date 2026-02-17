use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::config::Config;
use crate::graph::DependencyGraph;
use crate::metrics_report::{DependencyDepthMetrics, MetricsReport};
use crate::types::{
    ArchLayer, Component, ComponentKind, Severity, SourceLocation, Violation, ViolationKind,
};

/// Breakdown of architecture scores.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArchitectureScore {
    pub overall: f64,
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
    let overall = layer_isolation * w.layer_isolation_weight
        + dependency_direction * w.dependency_direction_weight
        + interface_coverage * w.interface_coverage_weight;

    // Clamp to 0-100
    let overall = overall.clamp(0.0, 100.0);

    ArchitectureScore {
        overall,
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

    for cycle in graph.find_cycles() {
        let cycle_str = cycle
            .iter()
            .map(|c| c.0.as_str())
            .collect::<Vec<_>>()
            .join(" -> ");
        violations.push(Violation {
            kind: ViolationKind::CircularDependency {
                cycle: cycle.clone(),
            },
            severity,
            location: SourceLocation {
                file: std::path::PathBuf::from("<cycle>"),
                line: 0,
                column: 0,
            },
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

    // Collect port names for adapter-without-port check
    let port_names: Vec<String> = nodes
        .iter()
        .filter(|n| {
            let name_lower = n.name.to_lowercase();
            name_lower.contains("port")
                || name_lower.contains("interface")
                || name_lower.contains("repository") && n.layer == Some(ArchLayer::Domain)
        })
        .map(|n| n.name.clone())
        .collect();

    // Check 1: Adapter without port
    for node in &nodes {
        let name_lower = node.name.to_lowercase();
        let is_adapter = name_lower.ends_with("handler")
            || name_lower.ends_with("controller")
            || (node.layer == Some(ArchLayer::Infrastructure)
                && (name_lower.contains("adapter") || name_lower.contains("impl")));

        if !is_adapter {
            continue;
        }

        // Check if there's a matching port name pattern
        let has_port = port_names.iter().any(|port| {
            let port_lower = port.to_lowercase();
            // e.g., "UserHandler" matches "UserPort" or "UserRepository"
            let adapter_base = name_lower
                .trim_end_matches("handler")
                .trim_end_matches("controller")
                .trim_end_matches("adapter")
                .trim_end_matches("impl");
            let port_base = port_lower
                .trim_end_matches("port")
                .trim_end_matches("interface")
                .trim_end_matches("repository");
            !adapter_base.is_empty() && !port_base.is_empty() && adapter_base == port_base
        });

        if !has_port {
            violations.push(Violation {
                kind: ViolationKind::MissingPort {
                    adapter_name: node.name.clone(),
                },
                severity,
                location: SourceLocation {
                    file: std::path::PathBuf::from("<pattern>"),
                    line: 0,
                    column: 0,
                },
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

    let correct = edges
        .iter()
        .filter(|(src, tgt, _)| match (src.layer, tgt.layer) {
            (Some(from), Some(to)) => !from.violates_dependency_on(&to),
            _ => false, // unclassified edges are not correct
        })
        .count();

    (correct as f64 / edges.len() as f64) * 100.0
}

/// Interface coverage: ratio of ports to total components (higher = better separation).
fn calculate_interface_coverage(graph: &DependencyGraph) -> f64 {
    let nodes = graph.nodes();
    if nodes.is_empty() {
        return 100.0;
    }

    // Count nodes in domain layer (likely ports) vs infrastructure (adapters)
    let mut ports = 0u64;
    let mut adapters = 0u64;

    for node in &nodes {
        // Heuristic: names containing "Port", "Interface", or in domain layer with interface-like names
        let name_lower = node.name.to_lowercase();
        if name_lower.contains("port")
            || name_lower.contains("interface")
            || (node.layer == Some(ArchLayer::Domain) && name_lower.ends_with("er"))
        {
            ports += 1;
        }
        if node.layer == Some(ArchLayer::Infrastructure) {
            adapters += 1;
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

    MetricsReport {
        components_by_kind,
        components_by_layer,
        violations_by_kind,
        dependency_depth: DependencyDepthMetrics {
            max_depth,
            avg_depth,
        },
        layer_coupling,
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
            }),
            layer,
            location: SourceLocation {
                file: PathBuf::from("test.go"),
                line: 1,
                column: 1,
            },
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
}
