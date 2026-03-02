use serde::Serialize;

use boundary_core::metrics::AnalysisResult;
use boundary_core::types::{Severity, Violation};

/// A violation with rule ID and name added for JSON output.
#[derive(Serialize)]
struct ViolationOutput<'a> {
    rule: String,
    rule_name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    doc_url: Option<String>,
    #[serde(flatten)]
    violation: &'a Violation,
}

impl<'a> ViolationOutput<'a> {
    fn from(v: &'a Violation) -> Self {
        Self {
            rule: v.kind.rule_id().to_string(),
            rule_name: v.kind.name().to_string(),
            doc_url: v.kind.doc_url(),
            violation: v,
        }
    }
}

/// Wrapper for the full analysis result that enriches violations with rule metadata.
#[derive(Serialize)]
struct AnalysisOutput<'a> {
    #[serde(skip_serializing_if = "Option::is_none")]
    score: &'a Option<boundary_core::metrics::ArchitectureScore>,
    violations: Vec<ViolationOutput<'a>>,
    component_count: usize,
    dependency_count: usize,
    files_analyzed: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    metrics: &'a Option<boundary_core::metrics_report::MetricsReport>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    package_metrics: &'a Vec<boundary_core::metrics::PackageMetric>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pattern_detection: &'a Option<boundary_core::pattern_detection::PatternDetection>,
}

impl<'a> AnalysisOutput<'a> {
    fn from(result: &'a AnalysisResult) -> Self {
        Self {
            score: &result.score,
            violations: result
                .violations
                .iter()
                .map(ViolationOutput::from)
                .collect(),
            component_count: result.component_count,
            dependency_count: result.dependency_count,
            files_analyzed: result.files_analyzed,
            metrics: &result.metrics,
            package_metrics: &result.package_metrics,
            pattern_detection: &result.pattern_detection,
        }
    }
}

/// Format a full analysis report as JSON.
pub fn format_report(result: &AnalysisResult, compact: bool) -> String {
    let output = AnalysisOutput::from(result);
    if compact {
        serde_json::to_string(&output).expect("AnalysisOutput should be serializable")
    } else {
        serde_json::to_string_pretty(&output).expect("AnalysisOutput should be serializable")
    }
}

/// Wrapper for multi-service output that enriches violations with rule metadata.
#[derive(Serialize)]
struct MultiServiceOutput<'a> {
    services: Vec<ServiceOutput<'a>>,
    aggregate: AnalysisOutput<'a>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    shared_modules: &'a Vec<boundary_core::metrics::SharedModule>,
}

#[derive(Serialize)]
struct ServiceOutput<'a> {
    service_name: &'a str,
    result: AnalysisOutput<'a>,
}

/// Format a multi-service analysis report as JSON.
pub fn format_multi_service_report(
    multi: &boundary_core::metrics::MultiServiceResult,
    compact: bool,
) -> String {
    let output = MultiServiceOutput {
        services: multi
            .services
            .iter()
            .map(|s| ServiceOutput {
                service_name: &s.service_name,
                result: AnalysisOutput::from(&s.result),
            })
            .collect(),
        aggregate: AnalysisOutput::from(&multi.aggregate),
        shared_modules: &multi.shared_modules,
    };
    if compact {
        serde_json::to_string(&output).expect("MultiServiceOutput should be serializable")
    } else {
        serde_json::to_string_pretty(&output).expect("MultiServiceOutput should be serializable")
    }
}

/// Wrapper for check output that adds pass/fail metadata.
#[derive(Serialize)]
struct CheckOutput<'a> {
    #[serde(flatten)]
    result: AnalysisOutput<'a>,
    check: CheckStatus,
}

#[derive(Serialize)]
struct CheckStatus {
    passed: bool,
    fail_on: Severity,
    failing_violation_count: usize,
}

/// Format a check result as JSON. Returns (json_string, passed).
pub fn format_check(result: &AnalysisResult, fail_on: Severity, compact: bool) -> (String, bool) {
    let failing_count = result
        .violations
        .iter()
        .filter(|v| v.severity >= fail_on)
        .count();

    let passed = failing_count == 0;

    let output = CheckOutput {
        result: AnalysisOutput::from(result),
        check: CheckStatus {
            passed,
            fail_on,
            failing_violation_count: failing_count,
        },
    };

    let json = if compact {
        serde_json::to_string(&output).expect("CheckOutput should be serializable")
    } else {
        serde_json::to_string_pretty(&output).expect("CheckOutput should be serializable")
    };

    (json, passed)
}

#[cfg(test)]
mod tests {
    use super::*;
    use boundary_core::metrics::{AnalysisResult, ArchitectureScore};
    use boundary_core::types::{SourceLocation, Violation, ViolationKind};
    use std::path::PathBuf;

    fn sample_result(with_violations: bool) -> AnalysisResult {
        let violations = if with_violations {
            vec![Violation {
                kind: ViolationKind::LayerBoundary {
                    from_layer: boundary_core::types::ArchLayer::Domain,
                    to_layer: boundary_core::types::ArchLayer::Infrastructure,
                },
                severity: Severity::Error,
                location: SourceLocation {
                    file: PathBuf::from("domain/user.go"),
                    line: 10,
                    column: 1,
                },
                message: "Domain depends on infrastructure".to_string(),
                suggestion: Some("Use a port interface".to_string()),
            }]
        } else {
            vec![]
        };

        AnalysisResult {
            score: Some(ArchitectureScore {
                overall: 75.0,
                structural_presence: 100.0,
                layer_conformance: 80.0,
                dependency_compliance: 70.0,
                interface_coverage: 75.0,
            }),
            violations,
            component_count: 5,
            dependency_count: 3,
            files_analyzed: 5,
            metrics: None,
            package_metrics: vec![],
            pattern_detection: None,
        }
    }

    #[test]
    fn test_format_report_valid_json() {
        let result = sample_result(true);
        let json = format_report(&result, false);
        let parsed: serde_json::Value = serde_json::from_str(&json).expect("should be valid JSON");
        assert!(parsed.get("score").is_some());
        assert!(parsed.get("violations").is_some());
        assert_eq!(parsed["component_count"], 5);
        assert_eq!(parsed["dependency_count"], 3);
    }

    #[test]
    fn test_format_report_compact_is_single_line() {
        let result = sample_result(false);
        let json = format_report(&result, true);
        assert!(!json.contains('\n'), "compact JSON should be single line");
        let _: serde_json::Value = serde_json::from_str(&json).expect("should be valid JSON");
    }

    #[test]
    fn test_format_report_pretty_is_multiline() {
        let result = sample_result(false);
        let json = format_report(&result, false);
        assert!(json.contains('\n'), "pretty JSON should be multiline");
    }

    #[test]
    fn test_format_check_passed() {
        let result = sample_result(false);
        let (json, passed) = format_check(&result, Severity::Error, false);
        assert!(passed);
        let parsed: serde_json::Value = serde_json::from_str(&json).expect("should be valid JSON");
        assert_eq!(parsed["check"]["passed"], true);
        assert_eq!(parsed["check"]["failing_violation_count"], 0);
        assert_eq!(parsed["check"]["fail_on"], "error");
    }

    #[test]
    fn test_format_check_failed() {
        let result = sample_result(true);
        let (json, passed) = format_check(&result, Severity::Error, false);
        assert!(!passed);
        let parsed: serde_json::Value = serde_json::from_str(&json).expect("should be valid JSON");
        assert_eq!(parsed["check"]["passed"], false);
        assert_eq!(parsed["check"]["failing_violation_count"], 1);
    }

    #[test]
    fn test_format_check_compact() {
        let result = sample_result(true);
        let (json, _) = format_check(&result, Severity::Error, true);
        assert!(!json.contains('\n'), "compact JSON should be single line");
        let _: serde_json::Value = serde_json::from_str(&json).expect("should be valid JSON");
    }

    #[test]
    fn test_violation_doc_url_in_json() {
        let result = sample_result(true);
        let json = format_report(&result, false);
        let parsed: serde_json::Value = serde_json::from_str(&json).expect("should be valid JSON");
        let violation = &parsed["violations"][0];
        assert!(
            violation.get("doc_url").is_some(),
            "built-in violations should have doc_url"
        );
        let url = violation["doc_url"].as_str().unwrap();
        assert!(
            url.starts_with("https://rebelopsio.github.io/boundary/features/rules.html#"),
            "doc_url should point to rules page: {url}"
        );
    }

    #[test]
    fn test_check_flattened_fields() {
        let result = sample_result(true);
        let (json, _) = format_check(&result, Severity::Error, false);
        let parsed: serde_json::Value = serde_json::from_str(&json).expect("should be valid JSON");
        // Flattened AnalysisResult fields should be at top level
        assert!(parsed.get("score").is_some());
        assert!(parsed.get("violations").is_some());
        assert!(parsed.get("component_count").is_some());
        // Check section should also be present
        assert!(parsed.get("check").is_some());
    }
}
