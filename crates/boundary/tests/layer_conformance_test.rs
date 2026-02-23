/// Acceptance tests for FR-28: True Layer Conformance Scoring.
///
/// Each test maps to a scenario in features/layer_conformance.feature.
/// Run `cargo test --test layer_conformance_test` to check the current state.
use std::process::Command;

fn boundary_cmd() -> Command {
    Command::new(env!("CARGO_BIN_EXE_boundary"))
}

fn fixture(name: &str) -> String {
    format!("{}/tests/fixtures/{name}", env!("CARGO_MANIFEST_DIR"))
}

/// Run `boundary analyze <fixture> --score-only --format json` and parse the JSON score object.
fn score_json(fixture_name: &str) -> serde_json::Value {
    let path = fixture(fixture_name);
    let output = boundary_cmd()
        .args(["analyze", &path, "--score-only", "--format", "json"])
        .output()
        .unwrap_or_else(|e| panic!("failed to run boundary on {fixture_name}: {e}"));

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(
        output.status.success(),
        "boundary failed on {fixture_name}: stdout={stdout}, stderr={stderr}"
    );

    serde_json::from_str(stdout.trim())
        .unwrap_or_else(|e| panic!("invalid JSON from {fixture_name}: {e}\noutput: {stdout}"))
}

/// Run `boundary analyze <fixture>` in text mode and return stdout.
fn analyze_text(fixture_name: &str) -> String {
    let path = fixture(fixture_name);
    let output = boundary_cmd()
        .args(["analyze", &path])
        .output()
        .unwrap_or_else(|e| panic!("failed to run boundary (text) on {fixture_name}: {e}"));

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(
        output.status.success(),
        "boundary (text) failed on {fixture_name}: stdout={stdout}, stderr={stderr}"
    );

    stdout.to_string()
}

// --------------------------------------------------------------------------
// Scenario: JSON score output uses layer_conformance field
// --------------------------------------------------------------------------

#[test]
fn test_score_json_has_layer_conformance_not_layer_isolation() {
    let score = score_json("sample-go-project");

    assert!(
        score.get("layer_conformance").is_some(),
        "score JSON must contain 'layer_conformance', got: {score}"
    );
    assert!(
        score.get("layer_isolation").is_none(),
        "score JSON must NOT contain old 'layer_isolation', got: {score}"
    );
}

// --------------------------------------------------------------------------
// Scenario: JSON score output uses dependency_compliance field
// --------------------------------------------------------------------------

#[test]
fn test_score_json_has_dependency_compliance_not_dependency_direction() {
    let score = score_json("sample-go-project");

    assert!(
        score.get("dependency_compliance").is_some(),
        "score JSON must contain 'dependency_compliance', got: {score}"
    );
    assert!(
        score.get("dependency_direction").is_none(),
        "score JSON must NOT contain old 'dependency_direction', got: {score}"
    );
}

// --------------------------------------------------------------------------
// Scenario: Layer conformance is a valid percentage
// --------------------------------------------------------------------------

#[test]
fn test_layer_conformance_is_valid_percentage() {
    let score = score_json("sample-go-project");

    let lc = score["layer_conformance"]
        .as_f64()
        .expect("layer_conformance must be a number");

    assert!(
        (0.0..=100.0).contains(&lc),
        "layer_conformance must be in [0, 100], got {lc}"
    );
}

// --------------------------------------------------------------------------
// Scenario: Interface coverage uses min-over-max formula
// --------------------------------------------------------------------------

#[test]
fn test_interface_coverage_uses_min_max_formula() {
    // The interface-coverage-project fixture has 2 ports and 1 adapter.
    // New formula: min(2,1)/max(2,1) = 0.5 → ~50%
    // Old formula: min(2/1, 1.0) * 100 = 100% (the bug this fixes)
    let score = score_json("interface-coverage-project");

    let iface = score["interface_coverage"]
        .as_f64()
        .expect("interface_coverage must be a number");

    assert!(
        (iface - 50.0).abs() <= 5.0,
        "interface_coverage with 2 ports / 1 adapter should be ~50% (±5), got {iface}"
    );
}

// --------------------------------------------------------------------------
// Scenario: Text output uses new label names
// --------------------------------------------------------------------------

#[test]
fn test_text_output_has_layer_conformance_label() {
    let text = analyze_text("sample-go-project");

    assert!(
        text.contains("Layer Conformance"),
        "text output must contain 'Layer Conformance', got:\n{text}"
    );
    assert!(
        !text.contains("Layer Isolation"),
        "text output must NOT contain old 'Layer Isolation', got:\n{text}"
    );
}

#[test]
fn test_text_output_has_dependency_compliance_label() {
    let text = analyze_text("sample-go-project");

    assert!(
        text.contains("Dependency Compliance"),
        "text output must contain 'Dependency Compliance', got:\n{text}"
    );
    assert!(
        !text.contains("Dependency Direction"),
        "text output must NOT contain old 'Dependency Direction', got:\n{text}"
    );
}
