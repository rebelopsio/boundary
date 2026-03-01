/// Acceptance tests for PA003: constructor-returns-concrete-type.
///
/// Verifies that constructors returning concrete types are flagged,
/// constructors returning port interfaces are not, and PA003 suppresses PA001.
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
// PA003 violation detected for concrete-returning constructor
// ----------------------------------------------------------------------------
#[test]
fn test_pa003_violations_detected() {
    let parsed = analyze_json("pa003-concrete-constructor");

    let violations = parsed["violations"]
        .as_array()
        .expect("should have violations array");

    let pa003_violations: Vec<_> = violations
        .iter()
        .filter(|v| v["rule"].as_str() == Some("PA003"))
        .collect();

    // MailGunService has a concrete-returning constructor
    assert!(
        !pa003_violations.is_empty(),
        "should detect PA003 violation for concrete-returning constructor"
    );

    // Verify the violation mentions the concrete type
    let has_mailgun = pa003_violations.iter().any(|v| {
        v["message"]
            .as_str()
            .unwrap_or("")
            .contains("MailGunService")
    });
    assert!(
        has_mailgun,
        "PA003 violation should reference MailGunService, found: {pa003_violations:?}"
    );
}

// ----------------------------------------------------------------------------
// PA003 does not fire for port-returning constructor
// ----------------------------------------------------------------------------
#[test]
fn test_pa003_does_not_fire_for_port_returning_constructor() {
    let parsed = analyze_json("pa003-concrete-constructor");

    let violations = parsed["violations"]
        .as_array()
        .expect("should have violations array");

    let pa003_stripe: Vec<_> = violations
        .iter()
        .filter(|v| {
            v["rule"].as_str() == Some("PA003")
                && v["message"]
                    .as_str()
                    .unwrap_or("")
                    .contains("StripeProcessor")
        })
        .collect();

    assert!(
        pa003_stripe.is_empty(),
        "StripeProcessor returns a port interface, should not have PA003, found: {pa003_stripe:?}"
    );
}

// ----------------------------------------------------------------------------
// PA003 suppresses PA001 for the same adapter
// ----------------------------------------------------------------------------
#[test]
fn test_pa003_suppresses_pa001() {
    let parsed = analyze_json("pa003-concrete-constructor");

    let violations = parsed["violations"]
        .as_array()
        .expect("should have violations array");

    let pa001_mailgun: Vec<_> = violations
        .iter()
        .filter(|v| {
            v["rule"].as_str() == Some("PA001")
                && v["message"]
                    .as_str()
                    .unwrap_or("")
                    .contains("MailGunService")
        })
        .collect();

    assert!(
        pa001_mailgun.is_empty(),
        "PA001 should be suppressed for MailGunService when PA003 fires, found: {pa001_mailgun:?}"
    );
}

// ----------------------------------------------------------------------------
// PA003 default severity is warning
// ----------------------------------------------------------------------------
#[test]
fn test_pa003_severity_is_warning() {
    let parsed = analyze_json("pa003-concrete-constructor");

    let violations = parsed["violations"]
        .as_array()
        .expect("should have violations array");

    let pa003_violations: Vec<_> = violations
        .iter()
        .filter(|v| v["rule"].as_str() == Some("PA003"))
        .collect();

    for v in &pa003_violations {
        assert_eq!(
            v["severity"].as_str(),
            Some("warning"),
            "PA003 default severity should be warning, got: {v}"
        );
    }
}
