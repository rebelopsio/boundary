/// Acceptance tests for FR-26: R.C. Martin Package Metrics.
///
/// Each test maps to a scenario in docs/features/04-rcm-package-metrics.feature.
/// Run `cargo test --test rcm_package_metrics_test` to check the current state.
///
/// All tests are RED until FR-26 is implemented in boundary-core.
use std::process::Command;

fn boundary_cmd() -> Command {
    Command::new(env!("CARGO_BIN_EXE_boundary"))
}

fn fixture(name: &str) -> String {
    format!("{}/tests/fixtures/{name}", env!("CARGO_MANIFEST_DIR"))
}

/// Run `boundary analyze <fixture> --format json` and return the parsed JSON.
/// Panics with a helpful message if the command fails or output is not valid JSON.
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

/// Find a package entry in the `package_metrics` array by package name.
fn find_package_metrics<'a>(
    json: &'a serde_json::Value,
    package: &str,
) -> Option<&'a serde_json::Value> {
    json["package_metrics"]
        .as_array()?
        .iter()
        .find(|entry| entry["package"].as_str() == Some(package))
}

/// Assert that a f64 metric is within 0.005 of the expected value (half a rounding unit).
fn assert_metric_near(actual: f64, expected: f64, label: &str) {
    assert!(
        (actual - expected).abs() < 0.005,
        "{label}: expected {expected:.2}, got {actual:.2}"
    );
}

// ============================================================================
// Background scenarios — rcm-ddd-project
// domain: Na=1, Nc=2 → A=0.5 | Ca=2, Ce=0 → I=0.0 | D=0.5
// application: Na=0, Nc=1 → A=0.0 | Ca=0, Ce=1 → I=1.0 | D=0.0
// infrastructure: Na=0, Nc=1 → A=0.0 | Ca=0, Ce=1 → I=1.0 | D=0.0
// ============================================================================

/// Scenario: A mixed package has abstractness proportional to its abstract type count
#[test]
fn rcm_domain_abstractness_is_half() {
    let json = analyze_json("rcm-ddd-project");
    let pkg = find_package_metrics(&json, "domain")
        .unwrap_or_else(|| panic!("'domain' not found in package_metrics: {json}"));
    let a = pkg["abstractness"]
        .as_f64()
        .expect("abstractness should be a number");
    assert_metric_near(a, 0.5, "domain.abstractness");
}

/// Scenario: A fully concrete package has abstractness 0.0
#[test]
fn rcm_infrastructure_abstractness_is_zero() {
    let json = analyze_json("rcm-ddd-project");
    let pkg = find_package_metrics(&json, "infrastructure")
        .unwrap_or_else(|| panic!("'infrastructure' not found in package_metrics: {json}"));
    let a = pkg["abstractness"]
        .as_f64()
        .expect("abstractness should be a number");
    assert_metric_near(a, 0.0, "infrastructure.abstractness");
}

/// Scenario: A package imported by others but importing nothing has instability 0.0
#[test]
fn rcm_domain_instability_is_zero() {
    let json = analyze_json("rcm-ddd-project");
    let pkg = find_package_metrics(&json, "domain")
        .unwrap_or_else(|| panic!("'domain' not found in package_metrics: {json}"));
    let i = pkg["instability"]
        .as_f64()
        .expect("instability should be a number");
    assert_metric_near(i, 0.0, "domain.instability");
}

/// Scenario: A leaf package that imports others but has no dependents has instability 1.0
#[test]
fn rcm_application_instability_is_one() {
    let json = analyze_json("rcm-ddd-project");
    let pkg = find_package_metrics(&json, "application")
        .unwrap_or_else(|| panic!("'application' not found in package_metrics: {json}"));
    let i = pkg["instability"]
        .as_f64()
        .expect("instability should be a number");
    assert_metric_near(i, 1.0, "application.instability");
}

/// Scenario: A concrete unstable package is on the main sequence (distance = 0.0)
#[test]
fn rcm_infrastructure_distance_is_zero() {
    let json = analyze_json("rcm-ddd-project");
    let pkg = find_package_metrics(&json, "infrastructure")
        .unwrap_or_else(|| panic!("'infrastructure' not found in package_metrics: {json}"));
    let d = pkg["distance"]
        .as_f64()
        .expect("distance should be a number");
    assert_metric_near(d, 0.0, "infrastructure.distance");
}

/// Scenario: Package metrics appear in JSON output (@contract)
#[test]
fn rcm_json_output_has_package_metrics_array() {
    let json = analyze_json("rcm-ddd-project");
    let arr = json["package_metrics"]
        .as_array()
        .unwrap_or_else(|| panic!("'package_metrics' array missing from JSON output: {json}"));
    assert!(
        !arr.is_empty(),
        "package_metrics array should not be empty: {json}"
    );
    for entry in arr {
        assert!(
            entry.get("package").is_some(),
            "entry missing 'package' field: {entry}"
        );
        assert!(
            entry.get("abstractness").is_some(),
            "entry missing 'abstractness' field: {entry}"
        );
        assert!(
            entry.get("instability").is_some(),
            "entry missing 'instability' field: {entry}"
        );
        assert!(
            entry.get("distance").is_some(),
            "entry missing 'distance' field: {entry}"
        );
    }
}

/// Scenario: Text output shows overall score but not per-package metric fields
#[test]
fn rcm_text_output_has_score_not_metric_fields() {
    let path = fixture("rcm-ddd-project");
    let output = boundary_cmd()
        .args(["analyze", &path])
        .output()
        .expect("failed to run boundary analyze (text)");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        output.status.success(),
        "analyze (text) should succeed: {stdout}"
    );
    assert!(
        stdout.contains("Overall Score") || stdout.to_lowercase().contains("score"),
        "text output should contain overall score: {stdout}"
    );
    assert!(
        !stdout.to_lowercase().contains("abstractness"),
        "text output should not contain 'abstractness': {stdout}"
    );
    assert!(
        !stdout.to_lowercase().contains("instability"),
        "text output should not contain 'instability': {stdout}"
    );
}

// ============================================================================
// Standalone scenarios (independent fixtures)
// ============================================================================

/// Scenario: An isolated package with no internal coupling has instability 0.0
/// Special case: Ca + Ce = 0 → I = 0.0
#[test]
fn rcm_isolated_util_instability_is_zero() {
    let json = analyze_json("rcm-isolated");
    let pkg = find_package_metrics(&json, "util")
        .unwrap_or_else(|| panic!("'util' not found in package_metrics: {json}"));
    let i = pkg["instability"]
        .as_f64()
        .expect("instability should be a number");
    assert_metric_near(i, 0.0, "util.instability");
}

/// Scenario: An isolated package with real components still appears in the metrics output
#[test]
fn rcm_isolated_util_present_in_output() {
    let json = analyze_json("rcm-isolated");
    assert!(
        find_package_metrics(&json, "util").is_some(),
        "'util' should appear in package_metrics: {json}"
    );
}

/// Scenario: A package in the Zone of Pain has distance 1.0
/// Concrete (A=0.0) and stable (I=0.0): D = |0.0 + 0.0 - 1| = 1.0
#[test]
fn rcm_zone_of_pain_common_distance_is_one() {
    let json = analyze_json("rcm-zone-of-pain");
    let pkg = find_package_metrics(&json, "common")
        .unwrap_or_else(|| panic!("'common' not found in package_metrics: {json}"));
    let d = pkg["distance"]
        .as_f64()
        .expect("distance should be a number");
    assert_metric_near(d, 1.0, "common.distance");
}

/// Scenario: A package in the Zone of Uselessness has distance 1.0
/// Abstract (A=1.0) and unstable (I=1.0): D = |1.0 + 1.0 - 1| = 1.0
#[test]
fn rcm_zone_of_uselessness_abstractions_distance_is_one() {
    let json = analyze_json("rcm-zone-of-uselessness");
    let pkg = find_package_metrics(&json, "abstractions")
        .unwrap_or_else(|| panic!("'abstractions' not found in package_metrics: {json}"));
    let d = pkg["distance"]
        .as_f64()
        .expect("distance should be a number");
    assert_metric_near(d, 1.0, "abstractions.distance");
}

/// Scenario: A package with no real components is excluded from the metrics output
/// Nc = 0 → excluded entirely (not present with zero values)
#[test]
fn rcm_empty_pkg_excluded_from_metrics() {
    let json = analyze_json("rcm-empty-pkg");
    assert!(
        find_package_metrics(&json, "empty").is_none(),
        "'empty' package should be excluded from package_metrics (Nc=0): {json}"
    );
}
