/// Acceptance tests for FR-19: Cross-Cutting Concern Exclusions.
///
/// Each test maps to a scenario in features/cross_cutting_exclusions.feature.
/// Run `cargo test --test cross_cutting_exclusions_test` to check the current state.
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
// Fixture: fr19-cross-cutting
//   .boundary.toml: cross_cutting = ["pkg/logger/**"]
//
//   Files:
//     domain/user.go         — User (entity) + UserRepository (port)
//     pkg/logger/log.go      — Logger (cross-cutting utility)
//     infrastructure/repo.go — PostgresRepo, imports both domain AND pkg/logger
// ============================================================================

/// @contract Scenario: Dependency on cross-cutting package does not count as a layer boundary violation
///
/// infrastructure/repo.go imports pkg/logger/ which is cross-cutting.
/// Without cross_cutting config this import would be unclassified and counted
/// against structural presence; with it, no LayerBoundary violation is raised.
#[test]
fn fr19_import_of_cross_cutting_package_is_not_a_layer_violation() {
    let json = analyze_json("fr19-cross-cutting");

    let violations = json["violations"]
        .as_array()
        .unwrap_or_else(|| panic!("'violations' missing or not an array: {json}"));

    let layer_violations: Vec<_> = violations
        .iter()
        .filter(|v| v["kind"].get("LayerBoundary").is_some())
        .collect();

    assert!(
        layer_violations.is_empty(),
        "expected no LayerBoundary violations when importing a cross-cutting package, got: {layer_violations:?}"
    );
}

/// @contract Scenario: Cross-cutting components appear in structural presence count
///
/// The Logger in pkg/logger/ should be counted as cross-cutting (not unclassified),
/// contributing to structural presence coverage.
#[test]
fn fr19_cross_cutting_components_counted_in_structural_presence() {
    let json = analyze_json("fr19-cross-cutting");

    let cross_cutting = json["metrics"]["classification_coverage"]["cross_cutting"]
        .as_u64()
        .unwrap_or_else(|| {
            panic!("'classification_coverage.cross_cutting' missing or not a number: {json}")
        });

    assert!(
        cross_cutting >= 1,
        "expected at least 1 cross-cutting component (Logger in pkg/logger/), got {cross_cutting}: {json}"
    );
}

/// Scenario: Cross-cutting paths are not counted as unclassified
///
/// With pkg/logger/ declared cross-cutting, the Logger component should NOT
/// appear in the unclassified count.
#[test]
fn fr19_cross_cutting_paths_not_counted_as_unclassified() {
    let json = analyze_json("fr19-cross-cutting");

    let unclassified = json["metrics"]["classification_coverage"]["unclassified"]
        .as_u64()
        .unwrap_or_else(|| {
            panic!("'classification_coverage.unclassified' missing or not a number: {json}")
        });

    assert_eq!(
        unclassified, 0,
        "expected 0 unclassified components when cross-cutting paths are declared, got {unclassified}: {json}"
    );
}

/// Scenario: Text output shows cross-cutting count
#[test]
fn fr19_text_output_shows_cross_cutting_count() {
    let text = analyze_text("fr19-cross-cutting");

    assert!(
        text.contains("Cross-cutting:"),
        "text output should contain 'Cross-cutting:' in Classification Coverage section: {text}"
    );

    // The cross-cutting count should not be zero
    let cross_cutting_line = text
        .lines()
        .find(|l| l.contains("Cross-cutting:"))
        .unwrap_or_else(|| panic!("'Cross-cutting:' line not found in output: {text}"));

    // Extract the number after "Cross-cutting:" — should be >= 1
    let count: u64 = cross_cutting_line
        .split(':')
        .nth(1)
        .and_then(|s| s.trim().parse().ok())
        .unwrap_or_else(|| {
            panic!("could not parse cross-cutting count from: {cross_cutting_line}")
        });

    assert!(
        count >= 1,
        "expected cross-cutting count >= 1 in text output, got {count}"
    );
}
