/// Acceptance tests for PA002: port-without-implementation.
///
/// Verifies that domain ports without a matching infrastructure adapter
/// are flagged, and that implemented ports are not.
use std::process::Command;

fn boundary_cmd() -> Command {
    Command::new(env!("CARGO_BIN_EXE_boundary"))
}

fn fixture(name: &str) -> String {
    format!("{}/tests/fixtures/{name}", env!("CARGO_MANIFEST_DIR"))
}

fn analyze_json(fixture_name: &str) -> serde_json::Value {
    let output = boundary_cmd()
        .args(["analyze", &fixture(fixture_name), "--format", "json"])
        .output()
        .expect("failed to run boundary analyze");

    let stdout = String::from_utf8_lossy(&output.stdout);
    serde_json::from_str(&stdout).expect("output should be valid JSON")
}

// ----------------------------------------------------------------------------
// PA002 fires for unimplemented port
// ----------------------------------------------------------------------------
#[test]
fn test_pa002_fires_for_unimplemented_port() {
    let parsed = analyze_json("pa002-port-without-impl");

    let violations = parsed["violations"]
        .as_array()
        .expect("should have violations array");

    let pa002_violations: Vec<_> = violations
        .iter()
        .filter(|v| v["rule"].as_str() == Some("PA002"))
        .collect();

    assert!(
        !pa002_violations.is_empty(),
        "should detect PA002 for unimplemented port AuditLogger"
    );

    let has_audit_logger = pa002_violations
        .iter()
        .any(|v| v["message"].as_str().unwrap_or("").contains("AuditLogger"));
    assert!(
        has_audit_logger,
        "PA002 should reference AuditLogger, found: {pa002_violations:?}"
    );
}

// ----------------------------------------------------------------------------
// PA002 does not fire for implemented port
// ----------------------------------------------------------------------------
#[test]
fn test_pa002_does_not_fire_for_implemented_port() {
    let parsed = analyze_json("pa002-port-without-impl");

    let violations = parsed["violations"]
        .as_array()
        .expect("should have violations array");

    let pa002_user_repo: Vec<_> = violations
        .iter()
        .filter(|v| {
            v["rule"].as_str() == Some("PA002")
                && v["message"]
                    .as_str()
                    .unwrap_or("")
                    .contains("UserRepository")
        })
        .collect();

    assert!(
        pa002_user_repo.is_empty(),
        "UserRepository has an adapter, should not have PA002, found: {pa002_user_repo:?}"
    );
}

// ----------------------------------------------------------------------------
// PA002 default severity is info
// ----------------------------------------------------------------------------
#[test]
fn test_pa002_severity_is_info() {
    let parsed = analyze_json("pa002-port-without-impl");

    let violations = parsed["violations"]
        .as_array()
        .expect("should have violations array");

    let pa002_violations: Vec<_> = violations
        .iter()
        .filter(|v| v["rule"].as_str() == Some("PA002"))
        .collect();

    for v in &pa002_violations {
        assert_eq!(
            v["severity"].as_str(),
            Some("info"),
            "PA002 default severity should be info, got: {v}"
        );
    }
}

// ----------------------------------------------------------------------------
// PA002 has doc_url
// ----------------------------------------------------------------------------
#[test]
fn test_pa002_has_doc_url() {
    let parsed = analyze_json("pa002-port-without-impl");

    let violations = parsed["violations"]
        .as_array()
        .expect("should have violations array");

    let pa002_violations: Vec<_> = violations
        .iter()
        .filter(|v| v["rule"].as_str() == Some("PA002"))
        .collect();

    for v in &pa002_violations {
        let url = v["doc_url"].as_str();
        assert!(url.is_some(), "PA002 should have doc_url");
        assert!(
            url.unwrap().contains("#pa002"),
            "doc_url should contain #pa002 anchor"
        );
    }
}
