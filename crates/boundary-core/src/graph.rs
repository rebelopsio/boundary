use std::collections::HashMap;

use petgraph::graph::{DiGraph, NodeIndex};
use petgraph::visit::EdgeRef;
use serde::{Deserialize, Serialize};

use crate::types::{ArchLayer, Component, ComponentId, Dependency, DependencyKind, SourceLocation};

/// Node in the dependency graph
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphNode {
    pub id: ComponentId,
    pub name: String,
    pub layer: Option<ArchLayer>,
}

/// Edge in the dependency graph
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphEdge {
    pub kind: DependencyKind,
    pub location: SourceLocation,
    pub import_path: Option<String>,
}

/// Directed dependency graph of architectural components.
pub struct DependencyGraph {
    graph: DiGraph<GraphNode, GraphEdge>,
    index: HashMap<ComponentId, NodeIndex>,
}

impl DependencyGraph {
    pub fn new() -> Self {
        Self {
            graph: DiGraph::new(),
            index: HashMap::new(),
        }
    }

    /// Add a component as a node. Returns the node index.
    pub fn add_component(&mut self, component: &Component) -> NodeIndex {
        if let Some(&idx) = self.index.get(&component.id) {
            return idx;
        }
        let node = GraphNode {
            id: component.id.clone(),
            name: component.name.clone(),
            layer: component.layer,
        };
        let idx = self.graph.add_node(node);
        self.index.insert(component.id.clone(), idx);
        idx
    }

    /// Ensure a component ID exists as a node (create a minimal node if needed).
    pub fn ensure_node(&mut self, id: &ComponentId, layer: Option<ArchLayer>) -> NodeIndex {
        if let Some(&idx) = self.index.get(id) {
            return idx;
        }
        let node = GraphNode {
            id: id.clone(),
            name: id.0.clone(),
            layer,
        };
        let idx = self.graph.add_node(node);
        self.index.insert(id.clone(), idx);
        idx
    }

    /// Add a dependency as an edge.
    pub fn add_dependency(&mut self, dep: &Dependency) {
        let from_idx = self.ensure_node(&dep.from, None);
        let to_idx = self.ensure_node(&dep.to, None);
        let edge = GraphEdge {
            kind: dep.kind.clone(),
            location: dep.location.clone(),
            import_path: dep.import_path.clone(),
        };
        self.graph.add_edge(from_idx, to_idx, edge);
    }

    /// Iterate over all edges with their source and target nodes.
    pub fn edges_with_nodes(&self) -> Vec<(&GraphNode, &GraphNode, &GraphEdge)> {
        self.graph
            .edge_references()
            .map(|e| {
                let src = &self.graph[e.source()];
                let tgt = &self.graph[e.target()];
                (src, tgt, e.weight())
            })
            .collect()
    }

    /// Find cycles using DFS. Returns groups of component IDs that form cycles.
    pub fn find_cycles(&self) -> Vec<Vec<ComponentId>> {
        let sccs = petgraph::algo::kosaraju_scc(&self.graph);
        sccs.into_iter()
            .filter(|scc| scc.len() > 1)
            .map(|scc| scc.iter().map(|&idx| self.graph[idx].id.clone()).collect())
            .collect()
    }

    /// Count ports and adapters in the graph.
    pub fn node_count(&self) -> usize {
        self.graph.node_count()
    }

    /// Get all nodes
    pub fn nodes(&self) -> Vec<&GraphNode> {
        self.graph.node_weights().collect()
    }
}

impl Default for DependencyGraph {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
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
                line: 1,
                column: 1,
            },
            import_path: None,
        }
    }

    #[test]
    fn test_add_component_and_dependency() {
        let mut graph = DependencyGraph::new();
        let c1 = make_component("a", "A", Some(ArchLayer::Domain));
        let c2 = make_component("b", "B", Some(ArchLayer::Infrastructure));

        graph.add_component(&c1);
        graph.add_component(&c2);
        assert_eq!(graph.node_count(), 2);

        graph.add_dependency(&make_dep("a", "b"));
        let edges = graph.edges_with_nodes();
        assert_eq!(edges.len(), 1);
    }

    #[test]
    fn test_find_cycles() {
        let mut graph = DependencyGraph::new();
        let c1 = make_component("a", "A", None);
        let c2 = make_component("b", "B", None);

        graph.add_component(&c1);
        graph.add_component(&c2);
        graph.add_dependency(&make_dep("a", "b"));
        graph.add_dependency(&make_dep("b", "a"));

        let cycles = graph.find_cycles();
        assert!(!cycles.is_empty(), "should detect cycle");
    }

    #[test]
    fn test_no_duplicate_nodes() {
        let mut graph = DependencyGraph::new();
        let c1 = make_component("a", "A", None);

        graph.add_component(&c1);
        graph.add_component(&c1); // duplicate
        assert_eq!(graph.node_count(), 1);
    }
}
