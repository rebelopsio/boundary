use boundary_core::forensics::ForensicsAnalysis;
use boundary_core::types::{ComponentKind, ViolationKind};

/// Format a forensics analysis as a Markdown report.
pub fn format_forensics_report(analysis: &ForensicsAnalysis) -> String {
    let mut out = String::new();

    // Header
    out.push_str(&format!("# Module Forensics: {}\n\n", analysis.module_name));
    out.push_str(&format!(
        "**Module Path:** `{}`\n\n",
        analysis.module_path.display()
    ));

    let conformance = if analysis.violations.is_empty() {
        "Clean - No violations detected"
    } else {
        "Violations detected"
    };
    out.push_str(&format!("**Architecture:** {conformance}\n\n"));

    // Directory tree
    out.push_str("---\n\n## Module Structure Overview\n\n```\n");
    for entry in &analysis.directory_tree {
        let indent = "  ".repeat(entry.depth.saturating_sub(1));
        out.push_str(&format!("{indent}{}\n", entry.rel_path));
    }
    out.push_str("```\n\n");

    // Aggregates
    if !analysis.aggregates.is_empty() {
        out.push_str("---\n\n## Aggregates\n\n");

        for agg in &analysis.aggregates {
            out.push_str(&format!("### {}\n\n", agg.component.name));
            out.push_str(&format!(
                "**File:** `{}`\n\n",
                agg.component.location.file.display()
            ));

            // Fields
            if let ComponentKind::Entity(ref info) = agg.component.kind {
                if !info.fields.is_empty() {
                    out.push_str("#### Fields\n\n");
                    out.push_str("| Field | Type |\n");
                    out.push_str("|-------|------|\n");
                    for field in &info.fields {
                        out.push_str(&format!("| {} | {} |\n", field.name, field.type_name));
                    }
                    out.push('\n');
                }

                // Methods
                if !info.methods.is_empty() {
                    out.push_str("#### Business Operations\n\n");
                    out.push_str("| Method | Parameters | Returns |\n");
                    out.push_str("|--------|-----------|--------|\n");
                    for method in &info.methods {
                        out.push_str(&format!(
                            "| {} | {} | {} |\n",
                            method.name,
                            if method.parameters.is_empty() {
                                "-"
                            } else {
                                &method.parameters
                            },
                            if method.return_type.is_empty() {
                                "-"
                            } else {
                                &method.return_type
                            }
                        ));
                    }
                    out.push('\n');
                }
            }

            // Value Objects
            if !agg.value_objects.is_empty() {
                out.push_str("#### Value Objects\n\n");
                for vo in &agg.value_objects {
                    out.push_str(&format!("- `{}`\n", vo.name));
                }
                out.push('\n');
            }

            // DDD Patterns
            if !agg.ddd_patterns.is_empty() {
                out.push_str("#### DDD Patterns\n\n");
                for pattern in &agg.ddd_patterns {
                    let check = if pattern.detected { "x" } else { " " };
                    out.push_str(&format!("- [{}] {}\n", check, pattern.name));
                }
                out.push('\n');
            }

            // Dependencies
            out.push_str("#### Dependencies\n\n");
            if !agg.dependency_audit.stdlib_imports.is_empty() {
                for imp in &agg.dependency_audit.stdlib_imports {
                    out.push_str(&format!("- `{imp}` (stdlib)\n"));
                }
            }
            if !agg.dependency_audit.internal_domain_imports.is_empty() {
                for imp in &agg.dependency_audit.internal_domain_imports {
                    out.push_str(&format!("- `{imp}` (internal domain)\n"));
                }
            }
            if !agg.dependency_audit.external_imports.is_empty() {
                for imp in &agg.dependency_audit.external_imports {
                    out.push_str(&format!("- `{imp}` (external)\n"));
                }
            }
            if agg.dependency_audit.infrastructure_leaks.is_empty() {
                out.push_str("- **Infrastructure leaks:** NONE\n");
            } else {
                for leak in &agg.dependency_audit.infrastructure_leaks {
                    out.push_str(&format!("- **INFRASTRUCTURE LEAK:** `{leak}`\n"));
                }
            }
            out.push_str(&format!(
                "- **Clean domain?** {}\n\n",
                if agg.dependency_audit.is_clean {
                    "YES"
                } else {
                    "NO"
                }
            ));
        }
    }

    // Domain Events
    out.push_str("---\n\n## Domain Events\n\n");
    if analysis.domain_events.is_empty() {
        out.push_str("No domain events found.\n\n");
    } else {
        out.push_str("| Event | Fields | File |\n");
        out.push_str("|-------|--------|------|\n");
        for event in &analysis.domain_events {
            let fields = if let ComponentKind::DomainEvent(ref info) = event.kind {
                info.fields
                    .iter()
                    .map(|f| f.name.as_str())
                    .collect::<Vec<_>>()
                    .join(", ")
            } else {
                String::new()
            };
            out.push_str(&format!(
                "| {} | {} | {} |\n",
                event.name,
                fields,
                event.location.file.display()
            ));
        }
        out.push('\n');
    }

    // Outbound Ports
    out.push_str("---\n\n## Outbound Ports\n\n");
    if analysis.ports.is_empty() {
        out.push_str("No port interfaces found.\n\n");
    } else {
        out.push_str("| Port | Methods | File |\n");
        out.push_str("|------|---------|------|\n");
        for port in &analysis.ports {
            let methods = if let ComponentKind::Port(ref info) = port.kind {
                info.methods
                    .iter()
                    .map(|m| m.name.as_str())
                    .collect::<Vec<_>>()
                    .join(", ")
            } else {
                String::new()
            };
            out.push_str(&format!(
                "| {} | {} | {} |\n",
                port.name,
                methods,
                port.location.file.display()
            ));
        }
        out.push('\n');
    }

    // Application Services
    out.push_str("---\n\n## Application Services\n\n");
    if analysis.application_services.is_empty() {
        out.push_str("No application services found.\n\n");
    } else {
        out.push_str("| Service | File |\n");
        out.push_str("|---------|------|\n");
        for svc in &analysis.application_services {
            out.push_str(&format!(
                "| {} | {} |\n",
                svc.name,
                svc.location.file.display()
            ));
        }
        out.push('\n');
    }

    // Infrastructure Adapters
    out.push_str("---\n\n## Infrastructure Adapters\n\n");
    if analysis.infrastructure_adapters.is_empty() {
        out.push_str("No infrastructure adapters found.\n\n");
    } else {
        out.push_str("| Adapter | Implements | File |\n");
        out.push_str("|---------|-----------|------|\n");
        for mapping in &analysis.infrastructure_adapters {
            let implements = if mapping.implements_ports.is_empty() {
                "-".to_string()
            } else {
                mapping.implements_ports.join(", ")
            };
            out.push_str(&format!(
                "| {} | {} | {} |\n",
                mapping.adapter.name,
                implements,
                mapping.adapter.location.file.display()
            ));
        }
        out.push('\n');
    }

    // Architecture Conformance
    out.push_str("---\n\n## Architecture Conformance\n\n");
    out.push_str(&format!(
        "**Overall Score:** {:.1}/100\n\n",
        analysis.score.overall
    ));
    out.push_str(&format!(
        "- Layer Isolation: {:.1}/100\n",
        analysis.score.layer_isolation
    ));
    out.push_str(&format!(
        "- Dependency Direction: {:.1}/100\n",
        analysis.score.dependency_direction
    ));
    out.push_str(&format!(
        "- Interface Coverage: {:.1}/100\n\n",
        analysis.score.interface_coverage
    ));

    // Violations
    out.push_str("### Violations\n\n");
    if analysis.violations.is_empty() {
        out.push_str("No violations found.\n\n");
    } else {
        for v in &analysis.violations {
            let kind_label = match &v.kind {
                ViolationKind::LayerBoundary {
                    from_layer,
                    to_layer,
                } => format!("{from_layer} -> {to_layer}"),
                ViolationKind::CircularDependency { .. } => "circular dependency".to_string(),
                ViolationKind::MissingPort { adapter_name } => {
                    format!("missing port for {adapter_name}")
                }
                ViolationKind::CustomRule { rule_name } => format!("custom: {rule_name}"),
                ViolationKind::DomainInfrastructureLeak { detail } => {
                    format!("infra leak: {detail}")
                }
                ViolationKind::InitFunctionCoupling {
                    from_layer,
                    to_layer,
                    ..
                } => {
                    format!("init coupling: {from_layer} -> {to_layer}")
                }
            };
            out.push_str(&format!(
                "- **{}** [{}] {}: {}\n",
                v.severity, kind_label, v.location, v.message
            ));
        }
        out.push('\n');
    }

    // Improvement Opportunities
    out.push_str("---\n\n## Improvement Opportunities\n\n");
    out.push_str(
        "> **Note:** This section is generated heuristically and may require manual review.\n\n",
    );
    if analysis.improvements.is_empty() {
        out.push_str("No improvement suggestions at this time.\n\n");
    } else {
        for suggestion in &analysis.improvements {
            out.push_str(&format!("- {suggestion}\n"));
        }
        out.push('\n');
    }

    out
}
