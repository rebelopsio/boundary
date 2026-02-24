/// Acceptance tests for FR-22: Hybrid Architecture Tolerance.
///
/// Each test maps to a scenario in features/hybrid_architecture.feature.
/// Run `cargo test --test hybrid_architecture_test` to check the current state.
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

fn layer_boundary_violations(result: &serde_json::Value) -> Vec<serde_json::Value> {
    result["violations"]
        .as_array()
        .unwrap_or(&vec![])
        .iter()
        .filter(|v| {
            v["kind"]
                .as_object()
                .is_some_and(|o| o.contains_key("LayerBoundary"))
        })
        .cloned()
        .collect()
}

/// FR-22 @contract: Legacy module in active-record mode has no LayerBoundary violations.
#[test]
fn hybrid_legacy_active_record_mode_no_layer_boundary() {
    let result = analyze_json("fr22-hybrid");
    let violations = layer_boundary_violations(&result);
    // The legacy module (services/legacy) imports infra from domain — this is allowed
    // because the override sets architecture_mode = "active-record" for services/legacy/**
    let legacy_violations: Vec<_> = violations
        .iter()
        .filter(|v| {
            v["location"]["file"]
                .as_str()
                .is_some_and(|f| f.contains("legacy"))
        })
        .collect();
    assert!(
        legacy_violations.is_empty(),
        "legacy module with active-record mode should have no LayerBoundary violations; got: {legacy_violations:?}"
    );
}

/// FR-22 @contract: Modern module in DDD mode has no violations when architecture is clean.
#[test]
fn hybrid_modern_ddd_mode_no_layer_boundary() {
    let result = analyze_json("fr22-hybrid");
    let violations = layer_boundary_violations(&result);
    let modern_violations: Vec<_> = violations
        .iter()
        .filter(|v| {
            v["location"]["file"]
                .as_str()
                .is_some_and(|f| f.contains("modern"))
        })
        .collect();
    assert!(
        modern_violations.is_empty(),
        "modern module with clean DDD architecture should have no LayerBoundary violations; got: {modern_violations:?}"
    );
}
