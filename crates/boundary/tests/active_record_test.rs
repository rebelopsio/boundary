/// Acceptance tests for FR-20: Active Record Pattern Recognition.
///
/// Each test maps to a scenario in features/active_record_pattern.feature.
/// Run `cargo test --test active_record_test` to check the current state.
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
        .unwrap_or_else(|e| panic!("failed to run boundary on {fixture_name}: {e}"));
    let stdout = String::from_utf8_lossy(&output.stdout);
    serde_json::from_str(stdout.trim())
        .unwrap_or_else(|e| panic!("invalid JSON from {fixture_name}: {e}\noutput: {stdout}"))
}

fn violation_kinds(result: &serde_json::Value) -> Vec<String> {
    result["violations"]
        .as_array()
        .unwrap_or(&vec![])
        .iter()
        .filter_map(|v| v["kind"].as_object())
        .flat_map(|o| o.keys().cloned())
        .collect()
}

/// FR-20 @contract: In strict mode a domain entity importing infrastructure is a violation.
#[test]
fn active_record_strict_mode_produces_layer_boundary_violation() {
    let result = analyze_json("fr20-active-record-strict");
    let kinds = violation_kinds(&result);
    assert!(
        kinds.contains(&"LayerBoundary".to_string()),
        "strict mode should report a LayerBoundary violation; got: {kinds:?}"
    );
}

/// FR-20 @contract: In active-record mode a domain entity importing infrastructure is not a violation.
#[test]
fn active_record_permissive_mode_no_layer_boundary_violation() {
    let result = analyze_json("fr20-active-record-permissive");
    let kinds = violation_kinds(&result);
    assert!(
        !kinds.contains(&"LayerBoundary".to_string()),
        "active-record mode should NOT report LayerBoundary; got: {kinds:?}"
    );
}

/// FR-20: Pattern detection output includes an entry for "active-record".
#[test]
fn active_record_pattern_detection_entry_present() {
    let result = analyze_json("fr20-active-record-strict");
    let patterns = result["pattern_detection"]["patterns"]
        .as_array()
        .expect("pattern_detection.patterns should be an array");
    let has_ar = patterns
        .iter()
        .any(|p| p["name"].as_str() == Some("active-record"));
    assert!(
        has_ar,
        "pattern_detection.patterns should contain an 'active-record' entry"
    );
}
