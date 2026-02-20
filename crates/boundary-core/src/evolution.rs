use std::io::{BufRead, Write};
use std::path::Path;

use anyhow::{Context, Result};
use chrono::Utc;
use serde::{Deserialize, Serialize};

use crate::metrics::AnalysisResult;

/// A snapshot of an analysis run, stored for evolution tracking.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnalysisSnapshot {
    pub timestamp: String,
    pub git_commit: Option<String>,
    pub git_branch: Option<String>,
    pub result: AnalysisResult,
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

    let trend = TrendReport {
        previous_score: last.result.score.overall,
        current_score: current_result.score.overall,
        score_delta: current_result.score.overall - last.result.score.overall,
        previous_violations: last.result.violations.len(),
        current_violations: current_result.violations.len(),
        violation_delta: current_result.violations.len() as i64
            - last.result.violations.len() as i64,
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

    fn sample_result(score: f64) -> AnalysisResult {
        AnalysisResult {
            score: ArchitectureScore {
                overall: score,
                structural_presence: 100.0,
                layer_isolation: score,
                dependency_direction: score,
                interface_coverage: score,
            },
            violations: vec![],
            component_count: 5,
            dependency_count: 3,
            files_analyzed: 5,
            metrics: None,
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
}
