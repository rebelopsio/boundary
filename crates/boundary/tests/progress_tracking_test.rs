/// Acceptance tests for the Progress Tracking feature.
///
/// Each test maps directly to a scenario in docs/features/03-progress-tracking.feature.
/// Run `cargo test --test progress_tracking_test` to see the current state.
use std::path::Path;
use std::process::Command;

fn boundary_cmd() -> Command {
    Command::new(env!("CARGO_BIN_EXE_boundary"))
}

/// Copy a fixture to a fresh temp directory so tests can write .boundary/ without
/// polluting the checked-in fixture tree.
fn copy_fixture_to_tempdir(name: &str) -> tempfile::TempDir {
    let tmpdir = tempfile::tempdir().expect("failed to create temp dir");
    let src = std::path::PathBuf::from(format!(
        "{}/tests/fixtures/{name}",
        env!("CARGO_MANIFEST_DIR")
    ));
    for entry in walkdir::WalkDir::new(&src) {
        let entry = entry.expect("failed to read dir entry");
        let rel = entry.path().strip_prefix(&src).unwrap();
        let dest = tmpdir.path().join(rel);
        if entry.file_type().is_dir() {
            std::fs::create_dir_all(&dest).unwrap();
        } else {
            std::fs::copy(entry.path(), &dest).unwrap();
        }
    }
    tmpdir
}

/// Write a pre-seeded `.boundary/history.ndjson` with a single snapshot at `score`.
fn seed_history(dir: &Path, score: f64) {
    let boundary_dir = dir.join(".boundary");
    std::fs::create_dir_all(&boundary_dir).unwrap();
    let history_path = boundary_dir.join("history.ndjson");
    let line = format!(
        r#"{{"timestamp":"2024-01-01T00:00:00Z","git_commit":null,"git_branch":null,"result":{{"score":{{"overall":{score},"structural_presence":100.0,"layer_isolation":100.0,"dependency_direction":100.0,"interface_coverage":100.0}},"violations":[],"component_count":3,"dependency_count":0,"files_analyzed":3}}}}"#
    );
    std::fs::write(history_path, format!("{line}\n")).unwrap();
}

// ----------------------------------------------------------------------------
// Scenario: --track records a snapshot that persists across runs
// Given a valid Go project with a current boundary score of 100
// And no previous snapshot has been recorded
// When I run "boundary check . --track"
// Then a subsequent run of "boundary check . --no-regression" exits 0
// ----------------------------------------------------------------------------
#[test]
fn progress_track_creates_snapshot_that_persists() {
    let tmpdir = copy_fixture_to_tempdir("full-ddd-module");
    let path = tmpdir.path().to_str().unwrap();

    let track = boundary_cmd()
        .args(["check", path, "--track"])
        .output()
        .expect("failed to run boundary check --track");

    assert!(
        track.status.success(),
        "check --track should exit 0 before asserting on follow-up"
    );

    let check = boundary_cmd()
        .args(["check", path, "--no-regression"])
        .output()
        .expect("failed to run boundary check --no-regression");

    assert!(
        check.status.success(),
        "subsequent --no-regression should exit 0 after snapshot was saved with --track"
    );
}

// ----------------------------------------------------------------------------
// Scenario: --track records a snapshot at a known path
// Given a valid Go project with a current boundary score of 100
// When I run "boundary check . --track"
// Then a snapshot file exists at ".boundary/history.ndjson"
// ----------------------------------------------------------------------------
#[test]
fn progress_track_creates_file_at_known_path() {
    let tmpdir = copy_fixture_to_tempdir("full-ddd-module");

    boundary_cmd()
        .args(["check", tmpdir.path().to_str().unwrap(), "--track"])
        .output()
        .expect("failed to run boundary check --track");

    assert!(
        tmpdir.path().join(".boundary/history.ndjson").exists(),
        "snapshot file should exist at .boundary/history.ndjson after --track"
    );
}

// ----------------------------------------------------------------------------
// Scenario Outline: --no-regression does not block the build
// row: no previous snapshot has been recorded
// (--no-regression is a no-op when no baseline has been established)
// ----------------------------------------------------------------------------
#[test]
fn progress_no_regression_no_history_exits_zero() {
    let tmpdir = copy_fixture_to_tempdir("full-ddd-module");

    let output = boundary_cmd()
        .args(["check", tmpdir.path().to_str().unwrap(), "--no-regression"])
        .output()
        .expect("failed to run boundary check --no-regression");

    assert!(
        output.status.success(),
        "--no-regression should exit 0 when no baseline has been established"
    );
}

// ----------------------------------------------------------------------------
// Scenario Outline: --no-regression does not block the build
// row: the last recorded snapshot has a score of 75
// (current score is 100 — improvement)
// ----------------------------------------------------------------------------
#[test]
fn progress_no_regression_improved_score_exits_zero() {
    let tmpdir = copy_fixture_to_tempdir("full-ddd-module");
    seed_history(tmpdir.path(), 75.0);

    let output = boundary_cmd()
        .args(["check", tmpdir.path().to_str().unwrap(), "--no-regression"])
        .output()
        .expect("failed to run boundary check --no-regression");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        output.status.success(),
        "--no-regression should exit 0 when score improved from 75 to 100: stdout={stdout}"
    );
}

// ----------------------------------------------------------------------------
// Scenario Outline: --no-regression does not block the build
// row: the last recorded snapshot has a score of 100
// (current score is 100 — unchanged)
// ----------------------------------------------------------------------------
#[test]
fn progress_no_regression_unchanged_score_exits_zero() {
    let tmpdir = copy_fixture_to_tempdir("full-ddd-module");
    seed_history(tmpdir.path(), 100.0);

    let output = boundary_cmd()
        .args(["check", tmpdir.path().to_str().unwrap(), "--no-regression"])
        .output()
        .expect("failed to run boundary check --no-regression");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        output.status.success(),
        "--no-regression should exit 0 when score is unchanged at 100: stdout={stdout}"
    );
}

// ----------------------------------------------------------------------------
// Scenario: --no-regression exits non-zero when the score has dropped
// Given a valid Go project with a current boundary score of 80
// And the last recorded snapshot has a score of 90
// When I run "boundary check . --no-regression"
// Then the exit code is non-zero
// ----------------------------------------------------------------------------
#[test]
fn progress_no_regression_score_dropped_exits_nonzero() {
    let tmpdir = copy_fixture_to_tempdir("adapters-override");
    seed_history(tmpdir.path(), 90.0);

    let output = boundary_cmd()
        .args(["check", tmpdir.path().to_str().unwrap(), "--no-regression"])
        .output()
        .expect("failed to run boundary check --no-regression");

    assert!(
        !output.status.success(),
        "--no-regression should exit non-zero when score dropped from 90 to 80"
    );
}

// ----------------------------------------------------------------------------
// Scenario: regression report identifies the previous and current scores
// Given a valid Go project with a current boundary score of 80
// And the last recorded snapshot has a score of 90
// When I run "boundary check . --no-regression"
// Then the output includes "90"
// And the output includes "80"
// ----------------------------------------------------------------------------
#[test]
fn progress_regression_report_includes_scores() {
    let tmpdir = copy_fixture_to_tempdir("adapters-override");
    seed_history(tmpdir.path(), 90.0);

    let output = boundary_cmd()
        .args(["check", tmpdir.path().to_str().unwrap(), "--no-regression"])
        .output()
        .expect("failed to run boundary check --no-regression");

    // Regression details are written to stderr; the score report goes to stdout.
    let combined = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    assert!(
        combined.contains("90"),
        "output should include the previous score (90): {combined}"
    );
    assert!(
        combined.contains("80"),
        "output should include the current score (80): {combined}"
    );
}

// ----------------------------------------------------------------------------
// Scenario: --track appends a new snapshot when combined with --no-regression
// Given a valid Go project with a current boundary score of 100
// And the last recorded snapshot has a score of 100
// When I run "boundary check . --track --no-regression"
// Then the exit code is 0
// And the snapshot history contains 2 entries
// ----------------------------------------------------------------------------
#[test]
fn progress_track_and_no_regression_appends_snapshot() {
    let tmpdir = copy_fixture_to_tempdir("full-ddd-module");
    seed_history(tmpdir.path(), 100.0);

    let output = boundary_cmd()
        .args([
            "check",
            tmpdir.path().to_str().unwrap(),
            "--track",
            "--no-regression",
        ])
        .output()
        .expect("failed to run boundary check --track --no-regression");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        output.status.success(),
        "--track --no-regression should exit 0 when score is unchanged: stdout={stdout}"
    );

    let history = std::fs::read_to_string(tmpdir.path().join(".boundary/history.ndjson")).unwrap();
    let entry_count = history.lines().filter(|l| !l.trim().is_empty()).count();
    assert_eq!(
        entry_count, 2,
        "snapshot history should contain 2 entries after --track appends: {history}"
    );
}
