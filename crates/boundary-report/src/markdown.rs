use boundary_core::metrics::AnalysisResult;
use boundary_core::types::{Severity, ViolationKind};

/// Format a full analysis report as Markdown.
pub fn format_report(result: &AnalysisResult) -> String {
    let mut out = String::new();

    out.push_str("# Boundary - Architecture Analysis\n\n");

    // Score summary
    out.push_str("## Scores\n\n");
    out.push_str("| Metric | Score |\n");
    out.push_str("|--------|-------|\n");
    out.push_str(&format!(
        "| **Overall** | **{:.1}/100** |\n",
        result.score.overall
    ));
    out.push_str(&format!(
        "| Layer Isolation | {:.1}/100 |\n",
        result.score.layer_isolation
    ));
    out.push_str(&format!(
        "| Dependency Direction | {:.1}/100 |\n",
        result.score.dependency_direction
    ));
    out.push_str(&format!(
        "| Interface Coverage | {:.1}/100 |\n",
        result.score.interface_coverage
    ));

    // Summary
    out.push_str(&format!(
        "\n## Summary\n\n- **Components:** {}\n- **Dependencies:** {}\n",
        result.component_count, result.dependency_count,
    ));

    // Metrics
    if let Some(ref metrics) = result.metrics {
        out.push_str("\n## Metrics\n\n");

        if !metrics.components_by_layer.is_empty() {
            out.push_str("### Components by Layer\n\n");
            out.push_str("| Layer | Count |\n");
            out.push_str("|-------|-------|\n");
            let mut layers: Vec<_> = metrics.components_by_layer.iter().collect();
            layers.sort_by_key(|(k, _)| (*k).clone());
            for (layer, count) in layers {
                out.push_str(&format!("| {layer} | {count} |\n"));
            }
        }

        if !metrics.components_by_kind.is_empty() {
            out.push_str("\n### Components by Kind\n\n");
            out.push_str("| Kind | Count |\n");
            out.push_str("|------|-------|\n");
            let mut kinds: Vec<_> = metrics.components_by_kind.iter().collect();
            kinds.sort_by_key(|(k, _)| (*k).clone());
            for (kind, count) in kinds {
                out.push_str(&format!("| {kind} | {count} |\n"));
            }
        }

        out.push_str(&format!(
            "\n**Dependency Depth:** max={}, avg={:.1}\n",
            metrics.dependency_depth.max_depth, metrics.dependency_depth.avg_depth
        ));
    }

    // Violations
    if result.violations.is_empty() {
        out.push_str("\n## Violations\n\nNo violations found.\n");
    } else {
        out.push_str(&format!(
            "\n## Violations ({} found)\n\n",
            result.violations.len()
        ));
        out.push_str("| Severity | Type | Location | Message |\n");
        out.push_str("|----------|------|----------|--------|\n");

        for v in &result.violations {
            let severity = match v.severity {
                Severity::Error => "ERROR",
                Severity::Warning => "WARN",
                Severity::Info => "INFO",
            };

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
            };

            out.push_str(&format!(
                "| {} | {} | {} | {} |\n",
                severity, kind_label, v.location, v.message
            ));
        }
    }

    out.push('\n');
    out
}

/// Format a check result as Markdown. Returns (markdown, passed).
pub fn format_check(result: &AnalysisResult, fail_on: Severity) -> (String, bool) {
    let failing_violations: Vec<_> = result
        .violations
        .iter()
        .filter(|v| v.severity >= fail_on)
        .collect();

    let passed = failing_violations.is_empty();

    let mut out = format_report(result);

    if passed {
        out.push_str("## Result\n\n**CHECK PASSED**\n");
    } else {
        out.push_str(&format!(
            "## Result\n\n**CHECK FAILED**: {} violation(s) at severity {} or above\n",
            failing_violations.len(),
            fail_on,
        ));
    }

    (out, passed)
}

#[cfg(test)]
mod tests {
    use super::*;
    use boundary_core::metrics::{AnalysisResult, ArchitectureScore};

    #[test]
    fn test_format_report_contains_score() {
        let result = AnalysisResult {
            score: ArchitectureScore {
                overall: 85.0,
                layer_isolation: 90.0,
                dependency_direction: 80.0,
                interface_coverage: 85.0,
            },
            violations: vec![],
            component_count: 3,
            dependency_count: 2,
            metrics: None,
        };
        let report = format_report(&result);
        assert!(report.contains("85.0/100"));
        assert!(report.contains("No violations found"));
    }

    #[test]
    fn test_format_check_passed() {
        let result = AnalysisResult {
            score: ArchitectureScore {
                overall: 100.0,
                layer_isolation: 100.0,
                dependency_direction: 100.0,
                interface_coverage: 100.0,
            },
            violations: vec![],
            component_count: 0,
            dependency_count: 0,
            metrics: None,
        };
        let (report, passed) = format_check(&result, Severity::Error);
        assert!(passed);
        assert!(report.contains("CHECK PASSED"));
    }
}
