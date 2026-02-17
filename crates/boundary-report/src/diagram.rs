use std::collections::HashMap;

use boundary_core::graph::DependencyGraph;
use boundary_core::types::ArchLayer;

/// Generate a Mermaid flowchart showing layers as subgraphs with components inside.
pub fn generate_layer_diagram(graph: &DependencyGraph) -> String {
    let mut out = String::new();
    out.push_str("flowchart TB\n");

    // Group nodes by layer
    let mut layer_nodes: HashMap<String, Vec<String>> = HashMap::new();
    let mut unclassified = Vec::new();

    for node in graph.nodes() {
        let name = sanitize_mermaid_id(&node.id.0);
        let label = &node.name;
        match node.layer {
            Some(ArchLayer::Domain) => layer_nodes
                .entry("Domain".to_string())
                .or_default()
                .push(format!("    {name}[\"{label}\"]")),
            Some(ArchLayer::Application) => layer_nodes
                .entry("Application".to_string())
                .or_default()
                .push(format!("    {name}[\"{label}\"]")),
            Some(ArchLayer::Infrastructure) => layer_nodes
                .entry("Infrastructure".to_string())
                .or_default()
                .push(format!("    {name}[\"{label}\"]")),
            Some(ArchLayer::Presentation) => layer_nodes
                .entry("Presentation".to_string())
                .or_default()
                .push(format!("    {name}[\"{label}\"]")),
            None => unclassified.push(format!("    {name}[\"{label}\"]")),
        }
    }

    // Render subgraphs in layer order
    let layer_order = ["Domain", "Application", "Infrastructure", "Presentation"];
    for layer in &layer_order {
        if let Some(nodes) = layer_nodes.get(*layer) {
            out.push_str(&format!("  subgraph {layer}\n"));
            for node in nodes {
                out.push_str(&format!("{node}\n"));
            }
            out.push_str("  end\n");
        }
    }

    if !unclassified.is_empty() {
        out.push_str("  subgraph Unclassified\n");
        for node in &unclassified {
            out.push_str(&format!("{node}\n"));
        }
        out.push_str("  end\n");
    }

    // Render edges
    for (src, tgt, edge) in graph.edges_with_nodes() {
        let from = sanitize_mermaid_id(&src.id.0);
        let to = sanitize_mermaid_id(&tgt.id.0);

        // Check if this is a violation edge (inner -> outer layer dependency)
        let is_violation = match (src.layer, tgt.layer) {
            (Some(from_layer), Some(to_layer)) => from_layer.violates_dependency_on(&to_layer),
            _ => false,
        };

        let label = edge
            .import_path
            .as_deref()
            .map(|p| {
                // Shorten long import paths
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
                out.push_str(&format!("  {from} -.->|violation| {to}\n"));
            } else {
                out.push_str(&format!("  {from} -.->|\"{label} (violation)\"| {to}\n"));
            }
        } else if label.is_empty() {
            out.push_str(&format!("  {from} --> {to}\n"));
        } else {
            out.push_str(&format!("  {from} -->|\"{label}\"| {to}\n"));
        }
    }

    // Style violation edges in red
    out.push_str("\n  style Domain fill:#e8f5e9\n");
    out.push_str("  style Application fill:#e3f2fd\n");
    out.push_str("  style Infrastructure fill:#fff3e0\n");
    out.push_str("  style Presentation fill:#fce4ec\n");

    out
}

/// Generate a simplified Mermaid flowchart showing layer-to-layer edges with counts.
pub fn generate_dependency_flow(graph: &DependencyGraph) -> String {
    let mut out = String::new();
    out.push_str("flowchart LR\n");

    // Count edges between layers
    let mut layer_edges: HashMap<(String, String), (usize, usize)> = HashMap::new(); // (total, violations)

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

    // Render layer nodes
    let nodes_by_layer = graph.nodes_by_layer();
    for (layer, count) in &nodes_by_layer {
        let id = sanitize_mermaid_id(layer);
        out.push_str(&format!("  {id}[\"{layer} ({count})\"]\n"));
    }

    // Render edges
    for ((from, to), (total, violations)) in &layer_edges {
        let from_id = sanitize_mermaid_id(from);
        let to_id = sanitize_mermaid_id(to);
        if *violations > 0 {
            out.push_str(&format!(
                "  {from_id} -.->|\"{total} deps ({violations} violations)\"| {to_id}\n"
            ));
        } else {
            out.push_str(&format!("  {from_id} -->|\"{total} deps\"| {to_id}\n"));
        }
    }

    out
}

/// Sanitize a string to be a valid Mermaid node ID.
fn sanitize_mermaid_id(s: &str) -> String {
    s.replace("::", "_")
        .replace(['/', '.', '-', ' '], "_")
        .replace(['<', '>'], "")
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
                methods: vec![],
            }),
            layer,
            location: SourceLocation {
                file: PathBuf::from("test.go"),
                line: 1,
                column: 1,
            },
            is_cross_cutting: false,
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
        assert!(diagram.contains("flowchart TB"));
        assert!(diagram.contains("subgraph Domain"));
        assert!(diagram.contains("subgraph Infrastructure"));
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
        assert!(diagram.contains("flowchart LR"));
        assert!(diagram.contains("deps"));
    }

    #[test]
    fn test_violation_edges_marked() {
        let mut graph = DependencyGraph::new();
        let c1 = make_component("domain::User", "User", Some(ArchLayer::Domain));
        let c2 = make_component("infra::Repo", "Repo", Some(ArchLayer::Infrastructure));
        graph.add_component(&c1);
        graph.add_component(&c2);
        // Domain -> Infrastructure = violation
        graph.add_dependency(&make_dep("domain::User", "infra::Repo"));

        let diagram = generate_layer_diagram(&graph);
        assert!(diagram.contains("violation"));
    }
}
