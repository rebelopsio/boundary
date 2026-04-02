/// Acceptance tests for DM001: anemic-domain-model.
///
/// Verifies that domain entities with only trivial methods (getters/setters)
/// or no methods are flagged, while rich entities with business methods are not.
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
// DM001 flags anemic entity with only getter methods
// ----------------------------------------------------------------------------
#[test]
fn test_dm001_anemic_order_detected() {
    let parsed = analyze_json("dm001-anemic-model");

    let violations = parsed["violations"]
        .as_array()
        .expect("should have violations array");

    let dm001_violations: Vec<_> = violations
        .iter()
        .filter(|v| v["rule"].as_str() == Some("DM001"))
        .collect();

    assert!(
        !dm001_violations.is_empty(),
        "should detect DM001 violation for anemic Order entity, got violations: {violations:?}"
    );

    let has_order = dm001_violations
        .iter()
        .any(|v| v["message"].as_str().unwrap_or("").contains("Order"));
    assert!(
        has_order,
        "DM001 should flag Order as anemic: {dm001_violations:?}"
    );
}

// ----------------------------------------------------------------------------
// Rich entity with business methods is NOT flagged
// ----------------------------------------------------------------------------
#[test]
fn test_dm001_rich_invoice_not_flagged() {
    let parsed = analyze_json("dm001-anemic-model");

    let violations = parsed["violations"]
        .as_array()
        .expect("should have violations array");

    let dm001_violations: Vec<_> = violations
        .iter()
        .filter(|v| v["rule"].as_str() == Some("DM001"))
        .collect();

    let has_invoice = dm001_violations
        .iter()
        .any(|v| v["message"].as_str().unwrap_or("").contains("Invoice"));
    assert!(
        !has_invoice,
        "DM001 should not flag Invoice which has business methods: {dm001_violations:?}"
    );
}

// ----------------------------------------------------------------------------
// DM001 has correct severity and doc_url
// ----------------------------------------------------------------------------
#[test]
fn test_dm001_severity_and_doc_url() {
    let parsed = analyze_json("dm001-anemic-model");

    let violations = parsed["violations"]
        .as_array()
        .expect("should have violations array");

    let dm001 = violations
        .iter()
        .find(|v| v["rule"].as_str() == Some("DM001"))
        .expect("should have at least one DM001 violation");

    assert_eq!(
        dm001["severity"].as_str(),
        Some("warning"),
        "DM001 default severity should be warning"
    );

    assert_eq!(
        dm001["doc_url"].as_str(),
        Some("https://rebelopsio.github.io/boundary/features/rules.html#dm001"),
        "DM001 should have correct doc URL"
    );
}

// ----------------------------------------------------------------------------
// DM001 can be suppressed with --ignore
// ----------------------------------------------------------------------------
#[test]
fn test_dm001_suppressed_by_ignore() {
    let output = boundary_cmd()
        .args([
            "analyze",
            &fixture("dm001-anemic-model"),
            "--format",
            "json",
            "--ignore",
            "DM001",
        ])
        .output()
        .expect("failed to run boundary analyze");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let parsed: serde_json::Value = serde_json::from_str(&stdout).expect("valid JSON");

    let violations = parsed["violations"]
        .as_array()
        .expect("should have violations array");

    let dm001_violations: Vec<_> = violations
        .iter()
        .filter(|v| v["rule"].as_str() == Some("DM001"))
        .collect();

    assert!(
        dm001_violations.is_empty(),
        "DM001 should be suppressed by --ignore: {dm001_violations:?}"
    );
}
