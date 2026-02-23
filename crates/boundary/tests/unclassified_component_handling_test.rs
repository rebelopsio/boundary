/// Acceptance tests for FR-23: Unclassified Component Handling.
///
/// Each test maps to a scenario in features/unclassified_component_handling.feature.
/// Run `cargo test --test unclassified_component_handling_test` to check the current state.
use std::process::Command;

fn boundary_cmd() -> Command {
    Command::new(env!("CARGO_BIN_EXE_boundary"))
}

fn fixture(name: &str) -> String {
    format!("{}/tests/fixtures/{name}", env!("CARGO_MANIFEST_DIR"))
}

fn analyze_json(fixture_name: &str) -> serde_json::Value {
    let path = fixture(fixture_name);
    let output = boundary_cmd()
        .args(["analyze", &path, "--format", "json"])
        .output()
        .unwrap_or_else(|e| panic!("failed to run boundary analyze on {fixture_name}: {e}"));

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(
        output.status.success(),
        "boundary analyze failed on {fixture_name}: stdout={stdout}, stderr={stderr}"
    );

    serde_json::from_str(stdout.trim())
        .unwrap_or_else(|e| panic!("invalid JSON from {fixture_name}: {e}\noutput: {stdout}"))
}

fn analyze_text(fixture_name: &str) -> String {
    let path = fixture(fixture_name);
    let output = boundary_cmd()
        .args(["analyze", &path])
        .output()
        .unwrap_or_else(|e| panic!("failed to run boundary analyze (text) on {fixture_name}: {e}"));

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(
        output.status.success(),
        "boundary analyze (text) failed on {fixture_name}: stdout={stdout}, stderr={stderr}"
    );

    stdout.to_string()
}

// ============================================================================
// Fixture: fr23-unclassified
//   No .boundary.toml — uses default patterns.
//
//   Files:
//     domain/user.go        — User struct (classified: domain)
//     worker/processor.go   — Processor + Scheduler structs (unclassified)
//
//   Expected: 1 classified, 2 unclassified, coverage = 33.3%
// ============================================================================

/// @contract Scenario: Unclassified percentage appears in JSON output
#[test]
fn fr23_unclassified_count_nonzero_in_json() {
    let json = analyze_json("fr23-unclassified");

    let unclassified = json["metrics"]["classification_coverage"]["unclassified"]
        .as_u64()
        .unwrap_or_else(|| {
            panic!("'classification_coverage.unclassified' missing or not a number: {json}")
        });

    assert!(
        unclassified > 0,
        "expected unclassified count > 0 for project with 'worker/' directory and no layer config: {json}"
    );

    let coverage = json["metrics"]["classification_coverage"]["coverage_percentage"]
        .as_f64()
        .unwrap_or_else(|| {
            panic!("'classification_coverage.coverage_percentage' missing or not a number: {json}")
        });

    assert!(
        coverage < 100.0,
        "expected coverage < 100% when components are unclassified, got {coverage:.1}%: {json}"
    );
}

/// @contract Scenario: Unclassified paths are listed in JSON output
#[test]
fn fr23_unclassified_paths_listed_in_json() {
    let json = analyze_json("fr23-unclassified");

    let paths = json["metrics"]["classification_coverage"]["unclassified_paths"]
        .as_array()
        .unwrap_or_else(|| {
            panic!("'classification_coverage.unclassified_paths' missing or not an array: {json}")
        });

    assert!(
        !paths.is_empty(),
        "expected at least one unclassified path (worker/) listed in JSON: {json}"
    );

    let has_worker = paths
        .iter()
        .any(|p| p.as_str().map(|s| s.contains("worker")).unwrap_or(false));

    assert!(
        has_worker,
        "expected 'worker' path to appear in unclassified_paths: {paths:?}"
    );
}

/// Scenario: Text output shows unclassified count > 0
#[test]
fn fr23_text_output_shows_unclassified_count() {
    let text = analyze_text("fr23-unclassified");

    assert!(
        text.contains("Unclassified:"),
        "text output should contain 'Unclassified:' in Classification Coverage: {text}"
    );

    let unclassified_line = text
        .lines()
        .find(|l| l.contains("Unclassified:"))
        .unwrap_or_else(|| panic!("'Unclassified:' line not found: {text}"));

    let count: u64 = unclassified_line
        .split(':')
        .nth(1)
        .and_then(|s| s.trim().parse().ok())
        .unwrap_or_else(|| panic!("could not parse unclassified count from: {unclassified_line}"));

    assert!(
        count > 0,
        "expected unclassified count > 0 in text output, got {count}"
    );
}

/// Scenario: Text output suggests adding patterns to .boundary.toml
#[test]
fn fr23_text_output_suggests_boundary_toml() {
    let text = analyze_text("fr23-unclassified");

    assert!(
        text.contains(".boundary.toml"),
        "text output should suggest adding patterns to .boundary.toml for unclassified paths: {text}"
    );
}

/// Scenario: DDD scores are not inflated by ignoring unclassified components
///
/// With 1 classified and 2 unclassified, structural_presence should be ~33%.
#[test]
fn fr23_structural_presence_reflects_unclassified_components() {
    let json = analyze_json("fr23-unclassified");

    // When pattern confidence < 0.5 the score object may be absent;
    // structural_presence is also inside score. Check whether score exists.
    if let Some(score) = json.get("score") {
        let presence = score["structural_presence"]
            .as_f64()
            .unwrap_or_else(|| panic!("'score.structural_presence' not a number: {json}"));

        assert!(
            presence <= 60.0,
            "expected structural_presence <= 60% (unclassified components excluded from layer score), got {presence:.1}%: {json}"
        );
    } else {
        // score is absent when top pattern confidence < 0.5 — verify unclassified is > 0
        let unclassified = json["metrics"]["classification_coverage"]["unclassified"]
            .as_u64()
            .unwrap_or(0);
        assert!(
            unclassified > 0,
            "either score or unclassified count should confirm unclassified components: {json}"
        );
    }
}
