use std::collections::HashMap;
use std::io::{BufRead, Write};
use std::path::Path;

use anyhow::{Context, Result};
use chrono::Utc;
use serde::{Deserialize, Serialize};

use crate::metrics::AnalysisResult;
use crate::types::Violation;

/// A snapshot of an analysis run, stored for evolution tracking.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnalysisSnapshot {
    pub timestamp: String,
    pub git_commit: Option<String>,
    pub git_branch: Option<String>,
    pub result: AnalysisResult,
}

/// Per-rule violation count change between two snapshots.
#[derive(Debug, Clone)]
pub struct RuleTrend {
    pub rule_id: String,
    pub previous_count: usize,
    pub current_count: usize,
    pub delta: i64,
}

/// Trend report comparing two snapshots.
#[derive(Debug, Clone)]
pub struct TrendReport {
    pub previous_score: f64,
    pub current_score: f64,
    pub score_delta: f64,
    pub previous_violations: usize,
    pub current_violations: usize,
    pub violation_delta: i64,
    pub rule_trends: Vec<RuleTrend>,
}

/// Save an analysis snapshot to `.boundary/history.ndjson`.
pub fn save_snapshot(project_path: &Path, result: &AnalysisResult) -> Result<()> {
    let dir = project_path.join(".boundary");
    std::fs::create_dir_all(&dir).with_context(|| format!("failed to create {}", dir.display()))?;

    let snapshot = AnalysisSnapshot {
        timestamp: Utc::now().to_rfc3339(),
        git_commit: get_git_commit(project_path),
        git_branch: get_git_branch(project_path),
        result: AnalysisResult {
            score: result.score.clone(),
            violations: result.violations.clone(),
            component_count: result.component_count,
            dependency_count: result.dependency_count,
            files_analyzed: result.files_analyzed,
            metrics: result.metrics.clone(),
            package_metrics: result.package_metrics.clone(),
            pattern_detection: result.pattern_detection.clone(),
        },
    };

    let line = serde_json::to_string(&snapshot).context("failed to serialize snapshot")?;

    let history_path = dir.join("history.ndjson");
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&history_path)
        .with_context(|| format!("failed to open {}", history_path.display()))?;

    writeln!(file, "{line}").context("failed to write snapshot")?;

    eprintln!("Snapshot saved to {}", history_path.display());

    Ok(())
}

/// Count violations grouped by rule ID.
fn count_by_rule(violations: &[Violation]) -> HashMap<String, usize> {
    let mut counts = HashMap::new();
    for v in violations {
        *counts.entry(v.kind.rule_id().to_string()).or_insert(0) += 1;
    }
    counts
}

/// Build per-rule trend data from previous and current violation counts.
fn build_rule_trends(
    previous: &HashMap<String, usize>,
    current: &HashMap<String, usize>,
) -> Vec<RuleTrend> {
    let mut all_rules: std::collections::BTreeSet<&String> = std::collections::BTreeSet::new();
    all_rules.extend(previous.keys());
    all_rules.extend(current.keys());

    all_rules
        .into_iter()
        .map(|rule_id| {
            let prev = *previous.get(rule_id).unwrap_or(&0);
            let curr = *current.get(rule_id).unwrap_or(&0);
            RuleTrend {
                rule_id: rule_id.clone(),
                previous_count: prev,
                current_count: curr,
                delta: curr as i64 - prev as i64,
            }
        })
        .collect()
}

/// Check if the current score regresses compared to the last snapshot.
/// Returns Some(TrendReport) if there's a regression, None otherwise.
pub fn check_regression(
    project_path: &Path,
    current_result: &AnalysisResult,
) -> Result<Option<TrendReport>> {
    let history_path = project_path.join(".boundary/history.ndjson");
    if !history_path.exists() {
        return Ok(None);
    }

    let last = load_last_snapshot(&history_path)?;
    let Some(last) = last else {
        return Ok(None);
    };

    let prev_overall = last.result.score.as_ref().map(|s| s.overall).unwrap_or(0.0);
    let curr_overall = current_result
        .score
        .as_ref()
        .map(|s| s.overall)
        .unwrap_or(0.0);

    let prev_by_rule = count_by_rule(&last.result.violations);
    let curr_by_rule = count_by_rule(&current_result.violations);
    let rule_trends = build_rule_trends(&prev_by_rule, &curr_by_rule);

    let trend = TrendReport {
        previous_score: prev_overall,
        current_score: curr_overall,
        score_delta: curr_overall - prev_overall,
        previous_violations: last.result.violations.len(),
        current_violations: current_result.violations.len(),
        violation_delta: current_result.violations.len() as i64
            - last.result.violations.len() as i64,
        rule_trends,
    };

    if trend.score_delta < 0.0 {
        Ok(Some(trend))
    } else {
        Ok(None)
    }
}

/// Load the most recent snapshot from the NDJSON history file.
fn load_last_snapshot(path: &Path) -> Result<Option<AnalysisSnapshot>> {
    let file =
        std::fs::File::open(path).with_context(|| format!("failed to open {}", path.display()))?;
    let reader = std::io::BufReader::new(file);

    let mut last: Option<AnalysisSnapshot> = None;
    for line in reader.lines() {
        let line = line.context("failed to read line from history")?;
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        match serde_json::from_str::<AnalysisSnapshot>(trimmed) {
            Ok(snapshot) => last = Some(snapshot),
            Err(e) => {
                eprintln!("Warning: skipping malformed history line: {e}");
            }
        }
    }

    Ok(last)
}

/// Get the current git commit hash, if available.
fn get_git_commit(project_path: &Path) -> Option<String> {
    std::process::Command::new("git")
        .args(["rev-parse", "HEAD"])
        .current_dir(project_path)
        .output()
        .ok()
        .and_then(|o| {
            if o.status.success() {
                Some(String::from_utf8_lossy(&o.stdout).trim().to_string())
            } else {
                None
            }
        })
}

/// Get the current git branch name, if available.
fn get_git_branch(project_path: &Path) -> Option<String> {
    std::process::Command::new("git")
        .args(["rev-parse", "--abbrev-ref", "HEAD"])
        .current_dir(project_path)
        .output()
        .ok()
        .and_then(|o| {
            if o.status.success() {
                Some(String::from_utf8_lossy(&o.stdout).trim().to_string())
            } else {
                None
            }
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::metrics::{AnalysisResult, ArchitectureScore};
    use crate::types::{ArchLayer, Severity, SourceLocation, ViolationKind};
    use std::path::PathBuf;

    fn sample_result(score: f64) -> AnalysisResult {
        AnalysisResult {
            score: Some(ArchitectureScore {
                overall: score,
                structural_presence: 100.0,
                layer_conformance: score,
                dependency_compliance: score,
                interface_coverage: score,
            }),
            violations: vec![],
            component_count: 5,
            dependency_count: 3,
            files_analyzed: 5,
            metrics: None,
            package_metrics: vec![],
            pattern_detection: None,
        }
    }

    fn make_violation(kind: ViolationKind) -> Violation {
        Violation {
            kind,
            severity: Severity::Error,
            location: SourceLocation {
                file: PathBuf::from("test.go"),
                line: 1,
                column: 1,
            },
            message: "test".to_string(),
            suggestion: None,
        }
    }

    fn sample_result_with_violations(score: f64, kinds: Vec<ViolationKind>) -> AnalysisResult {
        let violations = kinds.into_iter().map(make_violation).collect();
        AnalysisResult {
            score: Some(ArchitectureScore {
                overall: score,
                structural_presence: 100.0,
                layer_conformance: score,
                dependency_compliance: score,
                interface_coverage: score,
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
    fn test_save_and_check_no_regression() {
        let dir = tempfile::tempdir().unwrap();
        let result = sample_result(80.0);
        save_snapshot(dir.path(), &result).unwrap();

        let better_result = sample_result(90.0);
        let trend = check_regression(dir.path(), &better_result).unwrap();
        assert!(trend.is_none(), "no regression when score improves");
    }

    #[test]
    fn test_save_and_check_regression() {
        let dir = tempfile::tempdir().unwrap();
        let result = sample_result(90.0);
        save_snapshot(dir.path(), &result).unwrap();

        let worse_result = sample_result(70.0);
        let trend = check_regression(dir.path(), &worse_result).unwrap();
        assert!(trend.is_some(), "should detect regression");
        let trend = trend.unwrap();
        assert_eq!(trend.previous_score, 90.0);
        assert_eq!(trend.current_score, 70.0);
        assert_eq!(trend.score_delta, -20.0);
    }

    #[test]
    fn test_no_history_file() {
        let dir = tempfile::tempdir().unwrap();
        let result = sample_result(80.0);
        let trend = check_regression(dir.path(), &result).unwrap();
        assert!(trend.is_none(), "no regression when no history exists");
    }

    #[test]
    fn test_count_by_rule() {
        let violations = vec![
            make_violation(ViolationKind::LayerBoundary {
                from_layer: ArchLayer::Domain,
                to_layer: ArchLayer::Infrastructure,
            }),
            make_violation(ViolationKind::LayerBoundary {
                from_layer: ArchLayer::Domain,
                to_layer: ArchLayer::Infrastructure,
            }),
            make_violation(ViolationKind::MissingPort {
                adapter_name: "X".into(),
            }),
        ];
        let counts = count_by_rule(&violations);
        assert_eq!(counts.get("L001"), Some(&2));
        assert_eq!(counts.get("PA001"), Some(&1));
        assert_eq!(counts.get("PA003"), None);
    }

    #[test]
    fn test_build_rule_trends_new_rule_appears() {
        let prev = HashMap::from([("L001".to_string(), 2usize)]);
        let curr = HashMap::from([("L001".to_string(), 2usize), ("PA001".to_string(), 1usize)]);
        let trends = build_rule_trends(&prev, &curr);
        assert_eq!(trends.len(), 2);

        let l001 = trends.iter().find(|t| t.rule_id == "L001").unwrap();
        assert_eq!(l001.delta, 0);

        let pa001 = trends.iter().find(|t| t.rule_id == "PA001").unwrap();
        assert_eq!(pa001.previous_count, 0);
        assert_eq!(pa001.current_count, 1);
        assert_eq!(pa001.delta, 1);
    }

    #[test]
    fn test_build_rule_trends_rule_disappears() {
        let prev = HashMap::from([("L001".to_string(), 3usize), ("PA001".to_string(), 2usize)]);
        let curr = HashMap::from([("L001".to_string(), 1usize)]);
        let trends = build_rule_trends(&prev, &curr);

        let l001 = trends.iter().find(|t| t.rule_id == "L001").unwrap();
        assert_eq!(l001.delta, -2);

        let pa001 = trends.iter().find(|t| t.rule_id == "PA001").unwrap();
        assert_eq!(pa001.previous_count, 2);
        assert_eq!(pa001.current_count, 0);
        assert_eq!(pa001.delta, -2);
    }

    #[test]
    fn test_regression_includes_rule_trends() {
        let dir = tempfile::tempdir().unwrap();

        // Save a high-scoring snapshot with violations
        let prev = sample_result_with_violations(
            90.0,
            vec![ViolationKind::MissingPort {
                adapter_name: "X".into(),
            }],
        );
        save_snapshot(dir.path(), &prev).unwrap();

        // Current result is worse score with different violations
        let curr = sample_result_with_violations(
            70.0,
            vec![
                ViolationKind::MissingPort {
                    adapter_name: "X".into(),
                },
                ViolationKind::LayerBoundary {
                    from_layer: ArchLayer::Domain,
                    to_layer: ArchLayer::Infrastructure,
                },
            ],
        );

        let trend = check_regression(dir.path(), &curr).unwrap();
        assert!(trend.is_some(), "should detect regression");
        let trend = trend.unwrap();

        assert!(!trend.rule_trends.is_empty(), "should have rule trends");

        let l001 = trend
            .rule_trends
            .iter()
            .find(|t| t.rule_id == "L001")
            .unwrap();
        assert_eq!(l001.previous_count, 0);
        assert_eq!(l001.current_count, 1);
        assert_eq!(l001.delta, 1);

        let pa001 = trend
            .rule_trends
            .iter()
            .find(|t| t.rule_id == "PA001")
            .unwrap();
        assert_eq!(pa001.previous_count, 1);
        assert_eq!(pa001.current_count, 1);
        assert_eq!(pa001.delta, 0);
    }
}
