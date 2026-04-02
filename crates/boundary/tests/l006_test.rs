/// Acceptance tests for L006: framework-imports-in-domain.
///
/// Verifies that domain-layer imports of framework packages (ORM, HTTP frameworks,
/// cloud SDKs) are flagged, while allowed stdlib and infrastructure-layer imports
/// are not.
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
// L006 violations detected for framework imports in domain
// ----------------------------------------------------------------------------
#[test]
fn test_l006_violations_detected() {
    let parsed = analyze_json("l006-framework-in-domain");

    let violations = parsed["violations"]
        .as_array()
        .expect("should have violations array");

    let l006_violations: Vec<_> = violations
        .iter()
        .filter(|v| v["rule"].as_str() == Some("L006"))
        .collect();

    assert!(
        !l006_violations.is_empty(),
        "should detect L006 violations for framework imports in domain, got violations: {violations:?}"
    );

    // Should find gorm.io import
    let has_gorm = l006_violations
        .iter()
        .any(|v| v["message"].as_str().unwrap_or("").contains("gorm"));
    assert!(
        has_gorm,
        "L006 should detect gorm.io import in domain: {l006_violations:?}"
    );

    // Should find gin import
    let has_gin = l006_violations
        .iter()
        .any(|v| v["message"].as_str().unwrap_or("").contains("gin"));
    assert!(
        has_gin,
        "L006 should detect gin import in domain: {l006_violations:?}"
    );
}

// ----------------------------------------------------------------------------
// Allowed stdlib does not trigger L006
// ----------------------------------------------------------------------------
#[test]
fn test_l006_allowed_stdlib_not_flagged() {
    let parsed = analyze_json("l006-framework-in-domain");

    let violations = parsed["violations"]
        .as_array()
        .expect("should have violations array");

    let l006_violations: Vec<_> = violations
        .iter()
        .filter(|v| v["rule"].as_str() == Some("L006"))
        .collect();

    // The domain/customer.go imports "time" which should NOT trigger L006
    let has_time = l006_violations
        .iter()
        .any(|v| v["message"].as_str().unwrap_or("").contains("\"time\""));
    assert!(
        !has_time,
        "L006 should not flag allowed stdlib 'time': {l006_violations:?}"
    );
}

// ----------------------------------------------------------------------------
// Infrastructure layer is allowed to use frameworks
// ----------------------------------------------------------------------------
#[test]
fn test_l006_infrastructure_not_flagged() {
    let parsed = analyze_json("l006-framework-in-domain");

    let violations = parsed["violations"]
        .as_array()
        .expect("should have violations array");

    let l006_violations: Vec<_> = violations
        .iter()
        .filter(|v| v["rule"].as_str() == Some("L006"))
        .collect();

    // infrastructure/db.go imports gorm — should NOT trigger L006
    let infra_gorm = l006_violations.iter().any(|v| {
        let msg = v["message"].as_str().unwrap_or("");
        msg.contains("GormInvoiceRepository")
    });
    assert!(
        !infra_gorm,
        "L006 should not flag infrastructure-layer gorm imports: {l006_violations:?}"
    );
}

// ----------------------------------------------------------------------------
// L006 has correct severity and doc_url
// ----------------------------------------------------------------------------
#[test]
fn test_l006_severity_and_doc_url() {
    let parsed = analyze_json("l006-framework-in-domain");

    let violations = parsed["violations"]
        .as_array()
        .expect("should have violations array");

    let l006 = violations
        .iter()
        .find(|v| v["rule"].as_str() == Some("L006"))
        .expect("should have at least one L006 violation");

    assert_eq!(
        l006["severity"].as_str(),
        Some("error"),
        "L006 default severity should be error"
    );

    assert_eq!(
        l006["doc_url"].as_str(),
        Some("https://rebelopsio.github.io/boundary/features/rules.html#l006"),
        "L006 should have correct doc URL"
    );
}
