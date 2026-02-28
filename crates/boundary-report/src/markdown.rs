use boundary_core::metrics::AnalysisResult;
use boundary_core::types::Severity;

/// Format a full analysis report as Markdown.
pub fn format_report(result: &AnalysisResult) -> String {
    let mut out = String::new();

    out.push_str("# Boundary - Architecture Analysis\n\n");

    // Score summary (omitted when pattern-detection gate fails)
    if let Some(score) = &result.score {
        out.push_str("## Scores\n\n");
        out.push_str("| Metric | Score |\n");
        out.push_str("|--------|-------|\n");
        out.push_str(&format!("| **Overall** | **{:.1}/100** |\n", score.overall));
        out.push_str(&format!(
            "| Structural Presence | {:.1}/100 |\n",
            score.structural_presence
        ));
        out.push_str(&format!(
            "| Layer Conformance | {:.1}/100 |\n",
            score.layer_conformance
        ));
        out.push_str(&format!(
            "| Dependency Compliance | {:.1}/100 |\n",
            score.dependency_compliance
        ));
        out.push_str(&format!(
            "| Interface Coverage | {:.1}/100 |\n",
            score.interface_coverage
        ));
    }

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

        if let Some(ref coverage) = metrics.classification_coverage {
            out.push_str("\n### Classification Coverage\n\n");
            out.push_str(&format!(
                "**Coverage:** {:.1}% ({}/{})\n\n",
                coverage.coverage_percentage,
                coverage.classified + coverage.cross_cutting,
                coverage.total_components
            ));
            out.push_str("| Category | Count |\n");
            out.push_str("|----------|-------|\n");
            out.push_str(&format!("| Classified | {} |\n", coverage.classified));
            out.push_str(&format!("| Cross-cutting | {} |\n", coverage.cross_cutting));
            out.push_str(&format!("| Unclassified | {} |\n", coverage.unclassified));

            if !coverage.unclassified_paths.is_empty() {
                out.push_str(
                    "\n**Unclassified paths** (add patterns to `.boundary.toml` `[layers]`):\n\n",
                );
                for path in &coverage.unclassified_paths {
                    out.push_str(&format!("- `{path}`\n"));
                }
            }
        }
    }

    // Package Metrics
    if !result.package_metrics.is_empty() {
        out.push_str("\n## Package Metrics\n\n");
        out.push_str("| Package | A | I | D | Zone |\n");
        out.push_str("|---------|---|---|---|------|\n");
        for pm in &result.package_metrics {
            let zone = match pm.zone.as_deref() {
                Some("pain") => "⚠ Pain",
                Some("uselessness") => "⚠ Uselessness",
                _ => "—",
            };
            out.push_str(&format!(
                "| {} | {:.2} | {:.2} | {:.2} | {} |\n",
                pm.package, pm.abstractness, pm.instability, pm.distance, zone
            ));
        }
    }

    // Pattern Detection
    if let Some(ref pd) = result.pattern_detection {
        out.push_str("\n## Pattern Detection\n\n");
        out.push_str(&format!(
            "Top Pattern: **{}** ({:.0}% confidence)\n\n",
            pd.top_pattern,
            pd.top_confidence * 100.0
        ));
        out.push_str("| Pattern | Confidence |\n");
        out.push_str("|---------|------------|\n");
        for p in &pd.patterns {
            out.push_str(&format!("| {} | {:.0}% |\n", p.name, p.confidence * 100.0));
        }
    }

    // Violations
    if result.violations.is_empty() {
        out.push_str("\n## Violations\n\nNo violations found.\n");
    } else {
        out.push_str(&format!(
            "\n## Violations ({} found)\n\n",
            result.violations.len()
        ));
        out.push_str("| Rule | Severity | Name | Location | Message |\n");
        out.push_str("|------|----------|------|----------|--------|\n");

        for v in &result.violations {
            let severity = match v.severity {
                Severity::Error => "ERROR",
                Severity::Warning => "WARN",
                Severity::Info => "INFO",
            };

            out.push_str(&format!(
                "| {} | {} | {} | {} | {} |\n",
                v.kind.rule_id(),
                severity,
                v.kind.name(),
                v.location,
                v.message
            ));
        }
    }

    out.push('\n');
    out
}

/// Format a multi-service analysis report as Markdown.
pub fn format_multi_service_report(multi: &boundary_core::metrics::MultiServiceResult) -> String {
    let mut out = String::new();

    out.push_str("# Boundary - Multi-Service Analysis\n\n");

    // Per-service score table
    out.push_str("## Per-Service Scores\n\n");
    out.push_str("| Service | Overall | Conformance | Compliance | Interface Coverage |\n");
    out.push_str("|---------|---------|-------------|------------|--------------------|\n");

    for svc in &multi.services {
        let s = svc.result.score.as_ref();
        out.push_str(&format!(
            "| {} | {:.1} | {:.1} | {:.1} | {:.1} |\n",
            svc.service_name,
            s.map(|s| s.overall).unwrap_or(0.0),
            s.map(|s| s.layer_conformance).unwrap_or(0.0),
            s.map(|s| s.dependency_compliance).unwrap_or(0.0),
            s.map(|s| s.interface_coverage).unwrap_or(0.0),
        ));
    }

    // Aggregate
    let agg = multi.aggregate.score.as_ref();
    out.push_str("\n## Aggregate Score\n\n");
    out.push_str("| Metric | Score |\n");
    out.push_str("|--------|-------|\n");
    out.push_str(&format!(
        "| **Overall** | **{:.1}/100** |\n",
        agg.map(|s| s.overall).unwrap_or(0.0)
    ));
    out.push_str(&format!(
        "| Structural Presence | {:.1}/100 |\n",
        agg.map(|s| s.structural_presence).unwrap_or(0.0)
    ));
    out.push_str(&format!(
        "| Layer Conformance | {:.1}/100 |\n",
        agg.map(|s| s.layer_conformance).unwrap_or(0.0)
    ));
    out.push_str(&format!(
        "| Dependency Compliance | {:.1}/100 |\n",
        agg.map(|s| s.dependency_compliance).unwrap_or(0.0)
    ));
    out.push_str(&format!(
        "| Interface Coverage | {:.1}/100 |\n",
        agg.map(|s| s.interface_coverage).unwrap_or(0.0)
    ));

    // Shared modules
    if !multi.shared_modules.is_empty() {
        out.push_str("\n## Shared Modules\n\n");
        out.push_str("| Module | Used By |\n");
        out.push_str("|--------|--------|\n");
        for sm in &multi.shared_modules {
            out.push_str(&format!("| `{}` | {} |\n", sm.path, sm.used_by.join(", ")));
        }
    }

    // Per-service violations
    let total_violations: usize = multi
        .services
        .iter()
        .map(|s| s.result.violations.len())
        .sum();
    if total_violations == 0 {
        out.push_str("\n## Violations\n\nNo violations found.\n");
    } else {
        out.push_str(&format!("\n## Violations ({total_violations} total)\n\n"));
        for svc in &multi.services {
            if svc.result.violations.is_empty() {
                continue;
            }
            out.push_str(&format!(
                "### {} ({} violations)\n\n",
                svc.service_name,
                svc.result.violations.len()
            ));
            out.push_str("| Severity | Location | Message |\n");
            out.push_str("|----------|----------|--------|\n");
            for v in &svc.result.violations {
                let severity = match v.severity {
                    Severity::Error => "ERROR",
                    Severity::Warning => "WARN",
                    Severity::Info => "INFO",
                };
                out.push_str(&format!(
                    "| {} | {} | {} |\n",
                    severity, v.location, v.message
                ));
            }
            out.push('\n');
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
            score: Some(ArchitectureScore {
                overall: 85.0,
                structural_presence: 100.0,
                layer_conformance: 90.0,
                dependency_compliance: 80.0,
                interface_coverage: 85.0,
            }),
            violations: vec![],
            component_count: 3,
            dependency_count: 2,
            files_analyzed: 3,
            metrics: None,
            package_metrics: vec![],
            pattern_detection: None,
        };
        let report = format_report(&result);
        assert!(report.contains("85.0/100"));
        assert!(report.contains("No violations found"));
    }

    #[test]
    fn test_format_check_passed() {
        let result = AnalysisResult {
            score: Some(ArchitectureScore {
                overall: 100.0,
                structural_presence: 100.0,
                layer_conformance: 100.0,
                dependency_compliance: 100.0,
                interface_coverage: 100.0,
            }),
            violations: vec![],
            component_count: 0,
            dependency_count: 0,
            files_analyzed: 0,
            metrics: None,
            package_metrics: vec![],
            pattern_detection: None,
        };
        let (report, passed) = format_check(&result, Severity::Error);
        assert!(passed);
        assert!(report.contains("CHECK PASSED"));
    }
}
