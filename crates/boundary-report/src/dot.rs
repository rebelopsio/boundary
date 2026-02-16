use std::collections::HashMap;

use boundary_core::graph::DependencyGraph;
use boundary_core::types::ArchLayer;

/// Generate a GraphViz DOT diagram showing layers as subgraphs with components inside.
pub fn generate_layer_diagram(graph: &DependencyGraph) -> String {
    let mut out = String::new();
    out.push_str("digraph architecture {\n");
    out.push_str("  rankdir=TB;\n");
    out.push_str("  node [shape=box, style=filled];\n\n");

    // Group nodes by layer
    let mut layer_nodes: HashMap<String, Vec<(String, String)>> = HashMap::new();
    let mut unclassified = Vec::new();

    for node in graph.nodes() {
        let id = sanitize_dot_id(&node.id.0);
        let label = &node.name;
        match node.layer {
            Some(ArchLayer::Domain) => layer_nodes
                .entry("Domain".to_string())
                .or_default()
                .push((id, label.clone())),
            Some(ArchLayer::Application) => layer_nodes
                .entry("Application".to_string())
                .or_default()
                .push((id, label.clone())),
            Some(ArchLayer::Infrastructure) => layer_nodes
                .entry("Infrastructure".to_string())
                .or_default()
                .push((id, label.clone())),
            Some(ArchLayer::Presentation) => layer_nodes
                .entry("Presentation".to_string())
                .or_default()
                .push((id, label.clone())),
            None => unclassified.push((id, label.clone())),
        }
    }

    // Layer colors and render order
    let layer_styles = [
        ("Domain", "#e8f5e9"),
        ("Application", "#e3f2fd"),
        ("Infrastructure", "#fff3e0"),
        ("Presentation", "#fce4ec"),
    ];

    for (layer, color) in &layer_styles {
        if let Some(nodes) = layer_nodes.get(*layer) {
            out.push_str(&format!("  subgraph cluster_{} {{\n", layer.to_lowercase()));
            out.push_str(&format!("    label=\"{layer}\";\n"));
            out.push_str("    style=filled;\n");
            out.push_str(&format!("    color=\"{color}\";\n"));
            out.push_str("    node [fillcolor=white];\n");
            for (id, label) in nodes {
                out.push_str(&format!("    {id} [label=\"{label}\"];\n"));
            }
            out.push_str("  }\n\n");
        }
    }

    if !unclassified.is_empty() {
        out.push_str("  subgraph cluster_unclassified {\n");
        out.push_str("    label=\"Unclassified\";\n");
        out.push_str("    style=dashed;\n");
        out.push_str("    node [fillcolor=white];\n");
        for (id, label) in &unclassified {
            out.push_str(&format!("    {id} [label=\"{label}\"];\n"));
        }
        out.push_str("  }\n\n");
    }

    // Render edges
    for (src, tgt, edge) in graph.edges_with_nodes() {
        let from = sanitize_dot_id(&src.id.0);
        let to = sanitize_dot_id(&tgt.id.0);

        let is_violation = match (src.layer, tgt.layer) {
            (Some(from_layer), Some(to_layer)) => from_layer.violates_dependency_on(&to_layer),
            _ => false,
        };

        let label = edge
            .import_path
            .as_deref()
            .map(|p| {
                let parts: Vec<&str> = p.split('/').collect();
                if parts.len() > 2 {
                    parts[parts.len() - 2..].join("/")
                } else {
                    p.to_string()
                }
            })
            .unwrap_or_default();

        if is_violation {
            if label.is_empty() {
                out.push_str(&format!(
                    "  {from} -> {to} [color=red, style=dashed, label=\"violation\"];\n"
                ));
            } else {
                out.push_str(&format!(
                    "  {from} -> {to} [color=red, style=dashed, label=\"{label} (violation)\"];\n"
                ));
            }
        } else if label.is_empty() {
            out.push_str(&format!("  {from} -> {to};\n"));
        } else {
            out.push_str(&format!("  {from} -> {to} [label=\"{label}\"];\n"));
        }
    }

    out.push_str("}\n");
    out
}

/// Generate a simplified DOT diagram showing layer-to-layer edges with counts.
pub fn generate_dependency_flow(graph: &DependencyGraph) -> String {
    let mut out = String::new();
    out.push_str("digraph dependency_flow {\n");
    out.push_str("  rankdir=LR;\n");
    out.push_str("  node [shape=box, style=filled];\n\n");

    // Count edges between layers
    let mut layer_edges: HashMap<(String, String), (usize, usize)> = HashMap::new();

    for (src, tgt, _) in graph.edges_with_nodes() {
        let from_label = match src.layer {
            Some(l) => l.to_string(),
            None => "unclassified".to_string(),
        };
        let to_label = match tgt.layer {
            Some(l) => l.to_string(),
            None => "unclassified".to_string(),
        };

        let is_violation = match (src.layer, tgt.layer) {
            (Some(from_layer), Some(to_layer)) => from_layer.violates_dependency_on(&to_layer),
            _ => false,
        };

        let entry = layer_edges.entry((from_label, to_label)).or_insert((0, 0));
        entry.0 += 1;
        if is_violation {
            entry.1 += 1;
        }
    }

    // Layer node colors
    let layer_colors = [
        ("domain", "#e8f5e9"),
        ("application", "#e3f2fd"),
        ("infrastructure", "#fff3e0"),
        ("presentation", "#fce4ec"),
    ];

    // Render layer nodes
    let nodes_by_layer = graph.nodes_by_layer();
    for (layer, count) in &nodes_by_layer {
        let color = layer_colors
            .iter()
            .find(|(l, _)| *l == layer.as_str())
            .map(|(_, c)| *c)
            .unwrap_or("#f5f5f5");
        out.push_str(&format!(
            "  {layer} [label=\"{layer} ({count})\", fillcolor=\"{color}\"];\n"
        ));
    }
    out.push('\n');

    // Render edges
    for ((from, to), (total, violations)) in &layer_edges {
        if *violations > 0 {
            out.push_str(&format!(
                "  {from} -> {to} [color=red, style=dashed, label=\"{total} deps ({violations} violations)\"];\n"
            ));
        } else {
            out.push_str(&format!("  {from} -> {to} [label=\"{total} deps\"];\n"));
        }
    }

    out.push_str("}\n");
    out
}

/// Sanitize a string to be a valid DOT node ID.
fn sanitize_dot_id(s: &str) -> String {
    let cleaned: String = s
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect();
    // DOT IDs must start with a letter or underscore
    if cleaned.starts_with(|c: char| c.is_ascii_digit()) {
        format!("n_{cleaned}")
    } else {
        cleaned
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use boundary_core::graph::DependencyGraph;
    use boundary_core::types::*;
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
                line: 1,
                column: 1,
            },
            import_path: None,
        }
    }

    #[test]
    fn test_generate_layer_diagram() {
        let mut graph = DependencyGraph::new();
        let c1 = make_component("domain::User", "User", Some(ArchLayer::Domain));
        let c2 = make_component("infra::Repo", "Repo", Some(ArchLayer::Infrastructure));
        graph.add_component(&c1);
        graph.add_component(&c2);
        graph.add_dependency(&make_dep("infra::Repo", "domain::User"));

        let diagram = generate_layer_diagram(&graph);
        assert!(diagram.contains("digraph architecture"));
        assert!(diagram.contains("cluster_domain"));
        assert!(diagram.contains("cluster_infrastructure"));
        assert!(diagram.contains("->"));
    }

    #[test]
    fn test_generate_dependency_flow() {
        let mut graph = DependencyGraph::new();
        let c1 = make_component("domain::User", "User", Some(ArchLayer::Domain));
        let c2 = make_component("infra::Repo", "Repo", Some(ArchLayer::Infrastructure));
        graph.add_component(&c1);
        graph.add_component(&c2);
        graph.add_dependency(&make_dep("infra::Repo", "domain::User"));

        let diagram = generate_dependency_flow(&graph);
        assert!(diagram.contains("digraph dependency_flow"));
        assert!(diagram.contains("deps"));
    }

    #[test]
    fn test_violation_edges_marked_red() {
        let mut graph = DependencyGraph::new();
        let c1 = make_component("domain::User", "User", Some(ArchLayer::Domain));
        let c2 = make_component("infra::Repo", "Repo", Some(ArchLayer::Infrastructure));
        graph.add_component(&c1);
        graph.add_component(&c2);
        graph.add_dependency(&make_dep("domain::User", "infra::Repo"));

        let diagram = generate_layer_diagram(&graph);
        assert!(diagram.contains("color=red"));
        assert!(diagram.contains("violation"));
    }
}
