use std::collections::HashMap;

use petgraph::graph::{DiGraph, NodeIndex};
use petgraph::visit::EdgeRef;
use serde::{Deserialize, Serialize};

use crate::metrics_report::LayerCouplingMatrix;
use crate::types::{
    ArchLayer, ArchitectureMode, Component, ComponentId, ComponentKind, Dependency, DependencyKind,
    SourceLocation,
};

/// Node in the dependency graph
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphNode {
    pub id: ComponentId,
    pub name: String,
    pub layer: Option<ArchLayer>,
    pub is_cross_cutting: bool,
    #[serde(default)]
    pub architecture_mode: ArchitectureMode,
    #[serde(default)]
    pub location: SourceLocation,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kind: Option<ComponentKind>,
    #[serde(default)]
    pub is_external: bool,
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
            is_cross_cutting: component.is_cross_cutting,
            architecture_mode: component.architecture_mode,
            location: component.location.clone(),
            kind: Some(component.kind.clone()),
            is_external: false,
        };
        let idx = self.graph.add_node(node);
        self.index.insert(component.id.clone(), idx);
        idx
    }

    /// Ensure a component ID exists as a node (create a minimal node if needed).
    pub fn ensure_node(
        &mut self,
        id: &ComponentId,
        layer: Option<ArchLayer>,
        is_cross_cutting: bool,
    ) -> NodeIndex {
        self.ensure_node_with_mode(id, layer, is_cross_cutting, ArchitectureMode::Ddd)
    }

    /// Ensure a component ID exists as a node with a specific architecture mode.
    pub fn ensure_node_with_mode(
        &mut self,
        id: &ComponentId,
        layer: Option<ArchLayer>,
        is_cross_cutting: bool,
        architecture_mode: ArchitectureMode,
    ) -> NodeIndex {
        if let Some(&idx) = self.index.get(id) {
            return idx;
        }
        let node = GraphNode {
            id: id.clone(),
            name: id.0.clone(),
            layer,
            is_cross_cutting,
            architecture_mode,
            location: SourceLocation::default(),
            kind: None,
            is_external: false,
        };
        let idx = self.graph.add_node(node);
        self.index.insert(id.clone(), idx);
        idx
    }

    /// Add a dependency as an edge.
    pub fn add_dependency(&mut self, dep: &Dependency) {
        let from_idx = self.ensure_node(&dep.from, None, false);
        let to_idx = self.ensure_node(&dep.to, None, false);
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

    /// Mark a node as external (not from analyzed source files).
    pub fn mark_external(&mut self, id: &ComponentId) {
        if let Some(&idx) = self.index.get(id) {
            self.graph[idx].is_external = true;
        }
    }

    /// Count nodes grouped by layer, skipping external nodes.
    pub fn nodes_by_layer(&self) -> HashMap<String, usize> {
        let mut counts: HashMap<String, usize> = HashMap::new();
        for node in self.graph.node_weights() {
            if node.is_external {
                continue;
            }
            if node.is_cross_cutting {
                *counts.entry("cross_cutting".to_string()).or_insert(0) += 1;
            } else {
                let key = match node.layer {
                    Some(layer) => layer.to_string(),
                    None => "unclassified".to_string(),
                };
                *counts.entry(key).or_insert(0) += 1;
            }
        }
        counts
    }

    /// Build a layer coupling matrix from edge data.
    pub fn layer_coupling_matrix(&self) -> LayerCouplingMatrix {
        let mut matrix = LayerCouplingMatrix::new();
        for edge in self.graph.edge_references() {
            let src = &self.graph[edge.source()];
            let tgt = &self.graph[edge.target()];
            if let (Some(from_layer), Some(to_layer)) = (src.layer, tgt.layer) {
                matrix.increment(&from_layer, &to_layer);
            }
        }
        matrix
    }

    /// Calculate max dependency depth using BFS from each root node.
    pub fn max_dependency_depth(&self) -> usize {
        use petgraph::visit::Bfs;
        let mut max_depth = 0;
        // Find root nodes (no incoming edges)
        for idx in self.graph.node_indices() {
            let has_incoming = self
                .graph
                .neighbors_directed(idx, petgraph::Direction::Incoming)
                .next()
                .is_some();
            if !has_incoming {
                let mut bfs = Bfs::new(&self.graph, idx);
                let mut depth = 0;
                let mut current_level_end = idx;
                let mut next_level_end = idx;
                while let Some(node) = bfs.next(&self.graph) {
                    if node == current_level_end || node == idx {
                        // Check neighbors to find end of next level
                        for neighbor in self.graph.neighbors(node) {
                            next_level_end = neighbor;
                        }
                    }
                    for neighbor in self.graph.neighbors(node) {
                        next_level_end = neighbor;
                    }
                    if node == current_level_end && node != idx {
                        depth += 1;
                        current_level_end = next_level_end;
                    }
                }
                // Simple approach: count edges in longest path from root
                max_depth = max_depth.max(depth);
            }
        }
        // Fallback: use edge count as rough depth estimate
        if max_depth == 0 && self.graph.edge_count() > 0 {
            max_depth = 1;
        }
        max_depth
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
