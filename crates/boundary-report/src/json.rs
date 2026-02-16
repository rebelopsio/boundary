use serde::Serialize;

use boundary_core::metrics::AnalysisResult;
use boundary_core::types::Severity;

/// Format a full analysis report as JSON.
pub fn format_report(result: &AnalysisResult, compact: bool) -> String {
    if compact {
        serde_json::to_string(result).expect("AnalysisResult should be serializable")
    } else {
        serde_json::to_string_pretty(result).expect("AnalysisResult should be serializable")
    }
}

/// Wrapper for check output that adds pass/fail metadata.
#[derive(Debug, Serialize)]
pub struct CheckOutput<'a> {
    #[serde(flatten)]
    pub result: &'a AnalysisResult,
    pub check: CheckStatus,
}

#[derive(Debug, Serialize)]
pub struct CheckStatus {
    pub passed: bool,
    pub fail_on: Severity,
    pub failing_violation_count: usize,
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
        result,
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
            score: ArchitectureScore {
                overall: 75.0,
                layer_isolation: 80.0,
                dependency_direction: 70.0,
                interface_coverage: 75.0,
            },
            violations,
            component_count: 5,
            dependency_count: 3,
            metrics: None,
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
