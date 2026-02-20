use colored::Colorize;

use boundary_core::metrics::AnalysisResult;
use boundary_core::types::{Severity, ViolationKind};

/// Format a full analysis report for terminal output.
pub fn format_report(result: &AnalysisResult) -> String {
    let mut out = String::new();

    // Header
    out.push_str(&format!(
        "\n{}\n",
        "Boundary - Architecture Analysis".bold()
    ));
    out.push_str(&format!("{}\n\n", "=".repeat(40)));

    if result.files_analyzed == 0 {
        out.push_str(&format!(
            "{}\n",
            "No supported source files found".yellow().bold()
        ));
        out.push_str(
            "  The target directory contains no files that boundary can analyze.\n  \
             Ensure the directory contains Go, Rust, TypeScript, or Java source files.\n",
        );
    } else if result.component_count == 0 {
        out.push_str(&format!(
            "{}\n",
            "No components were detected".yellow().bold()
        ));
        out.push_str(
            "  Source files were found but no exported types could be extracted.\n  \
             Ensure types are exported (e.g. capitalized names in Go).\n",
        );
    } else if result.score.structural_presence == 0.0 {
        // Components exist but none match a known DDD layer pattern
        out.push_str(&format!(
            "{}\n",
            "No architectural layers detected".yellow().bold()
        ));
        out.push_str(
            "  Components were found but none match a known DDD layer pattern.\n  \
             Add layer path patterns to .boundary.toml to classify them.\n",
        );
    } else {
        out.push_str(&format_score_section(&result.score));
    }

    // Stats
    out.push_str(&format!(
        "\n{}: {} components, {} dependencies\n",
        "Summary".bold(),
        result.component_count,
        result.dependency_count,
    ));

    // Metrics
    if let Some(ref metrics) = result.metrics {
        out.push_str(&format!("\n{}\n{}\n", "Metrics".bold(), "-".repeat(40)));

        if !metrics.components_by_layer.is_empty() {
            out.push_str("  Components by layer:\n");
            let mut layers: Vec<_> = metrics.components_by_layer.iter().collect();
            layers.sort_by_key(|(k, _)| (*k).clone());
            for (layer, count) in layers {
                let label = capitalize(layer);
                out.push_str(&format!("    {label}: {count}\n"));
            }
        }

        if !metrics.components_by_kind.is_empty() {
            out.push_str("  Components by kind:\n");
            let mut kinds: Vec<_> = metrics.components_by_kind.iter().collect();
            kinds.sort_by_key(|(k, _)| (*k).clone());
            for (kind, count) in kinds {
                out.push_str(&format!("    {kind}: {count}\n"));
            }
        }

        out.push_str(&format!(
            "  Dependency depth: max={}, avg={:.1}\n",
            metrics.dependency_depth.max_depth, metrics.dependency_depth.avg_depth
        ));

        if let Some(ref coverage) = metrics.classification_coverage {
            out.push_str(&format!("\n{}\n", "Classification Coverage".bold()));
            out.push_str(&format!(
                "  Coverage: {:.1}% ({}/{})\n",
                coverage.coverage_percentage,
                coverage.classified + coverage.cross_cutting,
                coverage.total_components
            ));
            out.push_str(&format!("    Classified:    {}\n", coverage.classified));
            out.push_str(&format!("    Cross-cutting: {}\n", coverage.cross_cutting));
            out.push_str(&format!("    Unclassified:  {}\n", coverage.unclassified));

            if !coverage.unclassified_paths.is_empty() {
                out.push_str(&format!(
                    "\n  {} {}:\n",
                    "Unclassified paths".yellow(),
                    "(add patterns to .boundary.toml [layers])".dimmed()
                ));
                for path in &coverage.unclassified_paths {
                    out.push_str(&format!("    {path}\n"));
                }
            }
        }
    }

    // Violations — only claim "no violations" when we actually checked (layers were detected)
    let no_layers = result.score.structural_presence == 0.0 && result.component_count > 0;
    if result.violations.is_empty() && !no_layers {
        out.push_str(&format!("\n{}\n", "No violations found!".green().bold()));
    } else if !result.violations.is_empty() {
        out.push_str(&format!(
            "\n{} ({} found)\n{}\n",
            "Violations".red().bold(),
            result.violations.len(),
            "-".repeat(40),
        ));

        for v in &result.violations {
            let severity_str = match v.severity {
                Severity::Error => "ERROR".red().bold().to_string(),
                Severity::Warning => "WARN".yellow().bold().to_string(),
                Severity::Info => "INFO".blue().bold().to_string(),
            };

            let kind_label = match &v.kind {
                ViolationKind::LayerBoundary {
                    from_layer,
                    to_layer,
                } => {
                    format!("{from_layer} -> {to_layer}")
                }
                ViolationKind::CircularDependency { .. } => "circular dependency".to_string(),
                ViolationKind::MissingPort { adapter_name } => {
                    format!("missing port for {adapter_name}")
                }
                ViolationKind::CustomRule { rule_name } => {
                    format!("custom: {rule_name}")
                }
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
                "\n  {} [{}] {}\n",
                severity_str, kind_label, v.location,
            ));
            out.push_str(&format!("    {}\n", v.message));
            if let Some(ref suggestion) = v.suggestion {
                out.push_str(&format!("    {}: {}\n", "Suggestion".cyan(), suggestion));
            }
        }
    }

    out.push('\n');
    out
}

fn format_score_section(score: &boundary_core::metrics::ArchitectureScore) -> String {
    let mut out = String::new();

    let overall_pct = score.overall.round() as i64;
    let overall_str = format!("{overall_pct}%");
    let overall_color = if score.overall >= 80.0 {
        overall_str.green()
    } else if score.overall >= 50.0 {
        overall_str.yellow()
    } else {
        overall_str.red()
    };

    out.push_str(&format!("{}: {}\n", "Overall Score".bold(), overall_color));
    out.push_str(&format!(
        "  Structural Presence: {}%\n",
        score.structural_presence.round() as i64
    ));
    out.push_str(&format!(
        "  Layer Isolation: {}%\n",
        score.layer_isolation.round() as i64
    ));
    out.push_str(&format!(
        "  Dependency Direction: {}%\n",
        score.dependency_direction.round() as i64
    ));
    out.push_str(&format!(
        "  Interface Coverage: {}%\n",
        score.interface_coverage.round() as i64
    ));

    out
}

fn capitalize(s: &str) -> String {
    let mut chars = s.chars();
    match chars.next() {
        None => String::new(),
        Some(c) => c.to_uppercase().collect::<String>() + chars.as_str(),
    }
}

/// Format a multi-service analysis report for terminal output.
pub fn format_multi_service_report(multi: &boundary_core::metrics::MultiServiceResult) -> String {
    let mut out = String::new();

    out.push_str(&format!(
        "\n{}\n",
        "Boundary - Multi-Service Analysis".bold()
    ));
    out.push_str(&format!("{}\n\n", "=".repeat(40)));

    // Per-service score table
    out.push_str(&format!("{}\n", "Per-Service Scores".bold()));
    out.push_str(&format!(
        "  {:<20} {:>8} {:>10} {:>10} {:>10}\n",
        "Service", "Overall", "Isolation", "Direction", "Iface Cov"
    ));
    out.push_str(&format!("  {}\n", "-".repeat(62)));

    for svc in &multi.services {
        out.push_str(&format!(
            "  {:<20} {:>7.1} {:>9.1} {:>9.1} {:>9.1}\n",
            svc.service_name,
            svc.result.score.overall,
            svc.result.score.layer_isolation,
            svc.result.score.dependency_direction,
            svc.result.score.interface_coverage,
        ));
    }

    // Aggregate
    out.push_str(&format!("\n{}\n", "Aggregate Score".bold()));
    out.push_str(&format_score_section(&multi.aggregate.score));

    // Shared modules
    if !multi.shared_modules.is_empty() {
        out.push_str(&format!("\n{}\n", "Shared Modules".bold()));
        for sm in &multi.shared_modules {
            out.push_str(&format!(
                "  {} (used by: {})\n",
                sm.path,
                sm.used_by.join(", ")
            ));
        }
    }

    // Per-service violations
    let total_violations: usize = multi
        .services
        .iter()
        .map(|s| s.result.violations.len())
        .sum();
    if total_violations == 0 {
        out.push_str(&format!("\n{}\n", "No violations found!".green().bold()));
    } else {
        out.push_str(&format!(
            "\n{} ({total_violations} total)\n",
            "Violations".red().bold()
        ));
        for svc in &multi.services {
            if svc.result.violations.is_empty() {
                continue;
            }
            out.push_str(&format!(
                "\n  {} ({}):\n",
                svc.service_name.bold(),
                svc.result.violations.len()
            ));
            for v in &svc.result.violations {
                let severity_str = match v.severity {
                    Severity::Error => "ERROR".red().bold().to_string(),
                    Severity::Warning => "WARN".yellow().bold().to_string(),
                    Severity::Info => "INFO".blue().bold().to_string(),
                };
                out.push_str(&format!(
                    "    {} {} - {}\n",
                    severity_str, v.location, v.message
                ));
            }
        }
    }

    out.push('\n');
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use boundary_core::metrics::{AnalysisResult, ArchitectureScore};

    fn zero_presence_result() -> AnalysisResult {
        AnalysisResult {
            score: ArchitectureScore {
                overall: 0.0,
                structural_presence: 0.0,
                layer_isolation: 100.0,
                dependency_direction: 100.0,
                interface_coverage: 100.0,
            },
            violations: vec![],
            component_count: 2,
            dependency_count: 0,
            files_analyzed: 1,
            metrics: None,
        }
    }

    fn no_source_files_result() -> AnalysisResult {
        AnalysisResult {
            score: ArchitectureScore {
                overall: 100.0,
                structural_presence: 100.0,
                layer_isolation: 100.0,
                dependency_direction: 100.0,
                interface_coverage: 100.0,
            },
            violations: vec![],
            component_count: 0,
            dependency_count: 0,
            files_analyzed: 0,
            metrics: None,
        }
    }

    fn no_components_result() -> AnalysisResult {
        AnalysisResult {
            score: ArchitectureScore {
                overall: 100.0,
                structural_presence: 100.0,
                layer_isolation: 100.0,
                dependency_direction: 100.0,
                interface_coverage: 100.0,
            },
            violations: vec![],
            component_count: 0,
            dependency_count: 0,
            files_analyzed: 3,
            metrics: None,
        }
    }

    fn full_ddd_result() -> AnalysisResult {
        use boundary_core::metrics_report::{
            ClassificationCoverage, DependencyDepthMetrics, MetricsReport,
        };
        use std::collections::HashMap;

        let mut by_layer = HashMap::new();
        by_layer.insert("domain".to_string(), 2usize);
        by_layer.insert("application".to_string(), 1usize);
        by_layer.insert("infrastructure".to_string(), 1usize);

        AnalysisResult {
            score: ArchitectureScore {
                overall: 100.0,
                structural_presence: 100.0,
                layer_isolation: 100.0,
                dependency_direction: 100.0,
                interface_coverage: 100.0,
            },
            violations: vec![],
            component_count: 4,
            dependency_count: 0,
            files_analyzed: 3,
            metrics: Some(MetricsReport {
                components_by_kind: HashMap::new(),
                components_by_layer: by_layer,
                violations_by_kind: HashMap::new(),
                dependency_depth: DependencyDepthMetrics {
                    max_depth: 0,
                    avg_depth: 0.0,
                },
                layer_coupling: boundary_core::metrics_report::LayerCouplingMatrix {
                    matrix: HashMap::new(),
                },
                classification_coverage: Some(ClassificationCoverage {
                    total_components: 4,
                    classified: 4,
                    cross_cutting: 0,
                    unclassified: 0,
                    coverage_percentage: 100.0,
                    unclassified_paths: vec![],
                }),
            }),
        }
    }

    // Scenario: Codebase with complete DDD layer structure reports all layer components
    // Then the report lists components found in each layer (title-cased)
    #[test]
    fn format_report_complete_ddd_shows_title_cased_layers() {
        let result = full_ddd_result();
        let output = format_report(&result);
        assert!(output.contains("Domain"), "should show Domain: {output}");
        assert!(
            output.contains("Application"),
            "should show Application: {output}"
        );
        assert!(
            output.contains("Infrastructure"),
            "should show Infrastructure: {output}"
        );
    }

    // Scenario: Codebase where all components map to DDD layers receives full structural presence
    // Then the output contains "Structural Presence: 100%"
    #[test]
    fn format_report_structural_presence_percentage_format() {
        let result = full_ddd_result();
        let output = format_report(&result);
        assert!(
            output.contains("Structural Presence: 100%"),
            "should display Structural Presence: 100%: {output}"
        );
    }

    // Scenario: Codebase with no recognizable architectural structure
    // Then the report states that no architectural layers were detected
    #[test]
    fn format_report_no_layers_detected_message() {
        let result = zero_presence_result();
        let output = format_report(&result);
        assert!(
            output.contains("No architectural layers detected"),
            "should state no architectural layers detected: {output}"
        );
    }

    // The score section should not be shown when no layers are detected
    #[test]
    fn format_report_no_score_section_when_no_layers() {
        let result = zero_presence_result();
        let output = format_report(&result);
        assert!(
            !output.contains("Overall Score"),
            "should not show Overall Score when no layers detected: {output}"
        );
    }

    // Scenario: Target directory contains no Go files
    // Then the report states that no supported source files were found
    #[test]
    fn format_report_no_source_files_shows_message() {
        let result = no_source_files_result();
        let output = format_report(&result);
        assert!(
            output.to_lowercase().contains("no supported source files"),
            "should state no supported source files were found: {output}"
        );
    }

    // Scenario: Target directory contains Go files but no extractable components
    // Then the report states that no components were detected in the analyzed files
    #[test]
    fn format_report_no_components_detected_shows_message() {
        let result = no_components_result();
        let output = format_report(&result);
        assert!(
            output
                .to_lowercase()
                .contains("no components were detected"),
            "should state no components were detected: {output}"
        );
    }
}

/// Format a check result for CI use. Returns (text, passed).
pub fn format_check(result: &AnalysisResult, fail_on: Severity) -> (String, bool) {
    let failing_violations: Vec<_> = result
        .violations
        .iter()
        .filter(|v| v.severity >= fail_on)
        .collect();

    let passed = failing_violations.is_empty();

    let mut out = format_report(result);

    if passed {
        out.push_str(&format!("{}\n", "CHECK PASSED".green().bold()));
    } else {
        out.push_str(&format!(
            "{}: {} violation(s) at severity {} or above\n",
            "CHECK FAILED".red().bold(),
            failing_violations.len(),
            fail_on,
        ));
    }

    (out, passed)
}
