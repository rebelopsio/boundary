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

    // Score summary
    out.push_str(&format_score_section(&result.score));

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
                out.push_str(&format!("    {layer}: {count}\n"));
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
    }

    // Violations
    if result.violations.is_empty() {
        out.push_str(&format!("\n{}\n", "No violations found!".green().bold()));
    } else {
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

    let overall_str = format!("{:.1}", score.overall);
    let overall_color = if score.overall >= 80.0 {
        overall_str.green()
    } else if score.overall >= 50.0 {
        overall_str.yellow()
    } else {
        overall_str.red()
    };

    out.push_str(&format!(
        "{}: {}/100\n",
        "Overall Score".bold(),
        overall_color
    ));
    out.push_str(&format!(
        "  Layer Isolation:       {:.1}/100\n",
        score.layer_isolation
    ));
    out.push_str(&format!(
        "  Dependency Direction:  {:.1}/100\n",
        score.dependency_direction
    ));
    out.push_str(&format!(
        "  Interface Coverage:    {:.1}/100\n",
        score.interface_coverage
    ));

    out
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
