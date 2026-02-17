use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::types::ArchLayer;

/// Classification coverage: how much of the codebase is classified into layers.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClassificationCoverage {
    pub total_components: usize,
    pub classified: usize,
    pub cross_cutting: usize,
    pub unclassified: usize,
    pub coverage_percentage: f64,
    pub unclassified_paths: Vec<String>,
}

/// Detailed metrics beyond scores.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricsReport {
    pub components_by_kind: HashMap<String, usize>,
    pub components_by_layer: HashMap<String, usize>,
    pub violations_by_kind: HashMap<String, usize>,
    pub dependency_depth: DependencyDepthMetrics,
    pub layer_coupling: LayerCouplingMatrix,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub classification_coverage: Option<ClassificationCoverage>,
}

/// Dependency depth metrics.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DependencyDepthMetrics {
    pub max_depth: usize,
    pub avg_depth: f64,
}

/// Layer-to-layer coupling matrix: counts of edges between each pair of layers.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LayerCouplingMatrix {
    pub matrix: HashMap<String, HashMap<String, usize>>,
}

impl LayerCouplingMatrix {
    pub fn new() -> Self {
        let layers = [
            ArchLayer::Domain,
            ArchLayer::Application,
            ArchLayer::Infrastructure,
            ArchLayer::Presentation,
        ];
        let mut matrix = HashMap::new();
        for from in &layers {
            let mut row = HashMap::new();
            for to in &layers {
                row.insert(to.to_string(), 0);
            }
            matrix.insert(from.to_string(), row);
        }
        Self { matrix }
    }

    pub fn increment(&mut self, from: &ArchLayer, to: &ArchLayer) {
        if let Some(row) = self.matrix.get_mut(&from.to_string()) {
            if let Some(count) = row.get_mut(&to.to_string()) {
                *count += 1;
            }
        }
    }
}

impl Default for LayerCouplingMatrix {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_coupling_matrix_increment() {
        let mut matrix = LayerCouplingMatrix::new();
        matrix.increment(&ArchLayer::Domain, &ArchLayer::Infrastructure);
        matrix.increment(&ArchLayer::Domain, &ArchLayer::Infrastructure);
        assert_eq!(
            matrix.matrix["domain"]["infrastructure"], 2,
            "should count two edges"
        );
    }
}
