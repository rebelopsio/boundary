/// Acceptance tests for Zone of Pain / Zone of Uselessness detection (FR-26 extension).
///
/// Each test maps to a scenario in features/zone_detection.feature.
/// Run `cargo test --test zone_detection_test` to check the current state.
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

fn find_pkg<'a>(json: &'a serde_json::Value, package: &str) -> &'a serde_json::Value {
    json["package_metrics"]
        .as_array()
        .unwrap_or_else(|| panic!("package_metrics missing or not an array: {json}"))
        .iter()
        .find(|e| e["package"].as_str() == Some(package))
        .unwrap_or_else(|| panic!("package '{package}' not found in package_metrics: {json}"))
}

// ============================================================================
// @contract — JSON output shape
// ============================================================================

/// Scenario: Zone of Pain package has zone = "pain" in JSON
///
/// Fixture: rcm-zone-of-pain / common (A=0.0, I=0.0, D=1.0)
#[test]
fn zone_pain_package_has_zone_field_pain() {
    let json = analyze_json("rcm-zone-of-pain");
    let pkg = find_pkg(&json, "common");
    let zone = pkg["zone"]
        .as_str()
        .unwrap_or_else(|| panic!("'zone' field missing or not a string for 'common': {pkg}"));
    assert_eq!(
        zone, "pain",
        "expected zone = 'pain' for common (A=0, I=0, D=1), got '{zone}'"
    );
}

/// Scenario: Zone of Uselessness package has zone = "uselessness" in JSON
///
/// Fixture: rcm-zone-of-uselessness / abstractions (A=1.0, I=1.0, D=1.0)
#[test]
fn zone_uselessness_package_has_zone_field_uselessness() {
    let json = analyze_json("rcm-zone-of-uselessness");
    let pkg = find_pkg(&json, "abstractions");
    let zone = pkg["zone"].as_str().unwrap_or_else(|| {
        panic!("'zone' field missing or not a string for 'abstractions': {pkg}")
    });
    assert_eq!(
        zone, "uselessness",
        "expected zone = 'uselessness' for abstractions (A=1, I=1, D=1), got '{zone}'"
    );
}

/// Scenario: Main-sequence package has no zone field in JSON
///
/// Fixture: rcm-ddd-project / infrastructure (A=0.0, I=1.0, D=0.0)
#[test]
fn zone_main_sequence_package_has_no_zone_field() {
    let json = analyze_json("rcm-ddd-project");
    let pkg = find_pkg(&json, "infrastructure");
    assert!(
        pkg.get("zone").is_none() || pkg["zone"].is_null(),
        "expected no 'zone' field for main-sequence package 'infrastructure': {pkg}"
    );
}

// ============================================================================
// Text output
// ============================================================================

/// Scenario: Text output mentions Zone of Pain
#[test]
fn zone_text_output_shows_zone_of_pain() {
    let text = analyze_text("rcm-zone-of-pain");
    assert!(
        text.contains("Zone of Pain"),
        "text output should mention 'Zone of Pain' for a package with D=1.0, A=0, I=0: {text}"
    );
}

/// Scenario: Text output mentions Zone of Uselessness
#[test]
fn zone_text_output_shows_zone_of_uselessness() {
    let text = analyze_text("rcm-zone-of-uselessness");
    assert!(
        text.contains("Zone of Uselessness"),
        "text output should mention 'Zone of Uselessness' for a package with D=1.0, A=1, I=1: {text}"
    );
}

/// Scenario: Text output does not mention zones when no package is in one
///
/// Fixture: rcm-ddd-project — all packages are on or near the main sequence
#[test]
fn zone_text_output_silent_when_no_zones() {
    let text = analyze_text("rcm-ddd-project");
    assert!(
        !text.contains("Zone of Pain"),
        "text output should not mention 'Zone of Pain' when no package is in it: {text}"
    );
    assert!(
        !text.contains("Zone of Uselessness"),
        "text output should not mention 'Zone of Uselessness' when no package is in it: {text}"
    );
}
