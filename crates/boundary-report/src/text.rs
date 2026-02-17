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
