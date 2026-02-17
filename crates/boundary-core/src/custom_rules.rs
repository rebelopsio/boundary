use anyhow::{Context, Result};
use regex::Regex;

use crate::config::CustomRuleConfig;
use crate::graph::DependencyGraph;
use crate::types::{Violation, ViolationKind};

/// A compiled custom rule ready for evaluation.
pub struct CompiledCustomRule {
    pub name: String,
    pub from_regex: Regex,
    pub to_regex: Regex,
    pub severity: crate::types::Severity,
    pub message: Option<String>,
}

/// Compile custom rule configs into regex-based rules.
pub fn compile_rules(configs: &[CustomRuleConfig]) -> Result<Vec<CompiledCustomRule>> {
    configs
        .iter()
        .map(|cfg| {
            let from_regex = Regex::new(&cfg.from_pattern)
                .with_context(|| format!("invalid from_pattern in rule '{}'", cfg.name))?;
            let to_regex = Regex::new(&cfg.to_pattern)
                .with_context(|| format!("invalid to_pattern in rule '{}'", cfg.name))?;
            Ok(CompiledCustomRule {
                name: cfg.name.clone(),
                from_regex,
                to_regex,
                severity: cfg.severity,
                message: cfg.message.clone(),
            })
        })
        .collect()
}

/// Evaluate custom rules against the dependency graph, returning any violations.
pub fn evaluate_custom_rules(
    graph: &DependencyGraph,
    rules: &[CompiledCustomRule],
) -> Vec<Violation> {
    let mut violations = Vec::new();

    for (src, tgt, edge) in graph.edges_with_nodes() {
        let from_path = &src.id.0;
        let to_path = edge.import_path.as_deref().unwrap_or(&tgt.id.0);

        for rule in rules {
            if rule.from_regex.is_match(from_path) && rule.to_regex.is_match(to_path) {
                let message = rule.message.clone().unwrap_or_else(|| {
                    format!(
                        "Custom rule '{}' violated: {} -> {}",
                        rule.name, from_path, to_path
                    )
                });

                violations.push(Violation {
                    kind: ViolationKind::CustomRule {
                        rule_name: rule.name.clone(),
                    },
                    severity: rule.severity,
                    location: edge.location.clone(),
                    message,
                    suggestion: Some(format!(
                        "This dependency is forbidden by custom rule '{}'.",
                        rule.name
                    )),
                });
            }
        }
    }

    violations
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::CustomRuleConfig;
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

    fn make_dep(from: &str, to: &str, import: &str) -> Dependency {
        Dependency {
            from: ComponentId(from.to_string()),
            to: ComponentId(to.to_string()),
            kind: DependencyKind::Import,
            location: SourceLocation {
                file: PathBuf::from("test.go"),
                line: 10,
                column: 1,
            },
            import_path: Some(import.to_string()),
        }
    }

    #[test]
    fn test_compile_and_evaluate_custom_rules() {
        let configs = vec![CustomRuleConfig {
            name: "no-internal-to-external".to_string(),
            from_pattern: ".*/internal/.*".to_string(),
            to_pattern: ".*/external/.*".to_string(),
            action: "deny".to_string(),
            severity: Severity::Error,
            message: Some("Internal must not import external".to_string()),
        }];

        let rules = compile_rules(&configs).unwrap();
        assert_eq!(rules.len(), 1);

        let mut graph = DependencyGraph::new();
        let c1 = make_component(
            "app/internal/service",
            "Service",
            Some(ArchLayer::Application),
        );
        let c2 = make_component(
            "app/external/client",
            "Client",
            Some(ArchLayer::Infrastructure),
        );
        graph.add_component(&c1);
        graph.add_component(&c2);
        graph.add_dependency(&make_dep(
            "app/internal/service",
            "app/external/client",
            "app/external/client",
        ));

        let violations = evaluate_custom_rules(&graph, &rules);
        assert_eq!(violations.len(), 1);
        assert!(matches!(
            violations[0].kind,
            ViolationKind::CustomRule { .. }
        ));
    }

    #[test]
    fn test_no_match_no_violation() {
        let configs = vec![CustomRuleConfig {
            name: "no-internal-to-external".to_string(),
            from_pattern: ".*/internal/.*".to_string(),
            to_pattern: ".*/external/.*".to_string(),
            action: "deny".to_string(),
            severity: Severity::Error,
            message: None,
        }];

        let rules = compile_rules(&configs).unwrap();

        let mut graph = DependencyGraph::new();
        let c1 = make_component("app/domain/user", "User", Some(ArchLayer::Domain));
        let c2 = make_component("app/domain/order", "Order", Some(ArchLayer::Domain));
        graph.add_component(&c1);
        graph.add_component(&c2);
        graph.add_dependency(&make_dep(
            "app/domain/user",
            "app/domain/order",
            "app/domain/order",
        ));

        let violations = evaluate_custom_rules(&graph, &rules);
        assert!(violations.is_empty());
    }
}
