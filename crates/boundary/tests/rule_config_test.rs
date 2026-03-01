/// Acceptance tests for Phase 2: config-based rule configuration.
///
/// Verifies severity overrides via rule IDs, path-specific ignores,
/// and rule ID precedence over category names.
use std::process::Command;

fn boundary_cmd() -> Command {
    Command::new(env!("CARGO_BIN_EXE_boundary"))
}

fn fixture(name: &str) -> String {
    format!("{}/tests/fixtures/{name}", env!("CARGO_MANIFEST_DIR"))
}

// ----------------------------------------------------------------------------
// Severity override: PA001 violations should have "info" severity
// ----------------------------------------------------------------------------
#[test]
fn severity_override_rule_id_in_json() {
    let output = boundary_cmd()
        .args([
            "analyze",
            &fixture("rule-config-override"),
            "--format",
            "json",
        ])
        .output()
        .expect("failed to run boundary analyze");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let parsed: serde_json::Value =
        serde_json::from_str(&stdout).expect("output should be valid JSON");

    let violations = parsed["violations"]
        .as_array()
        .expect("should have violations array");

    let pa001_violations: Vec<_> = violations
        .iter()
        .filter(|v| v["rule"].as_str() == Some("PA001"))
        .collect();

    // The fixture has an adapter (UserRepository) without a port, so PA001 should fire
    assert!(
        !pa001_violations.is_empty(),
        "should have at least one PA001 violation"
    );

    for v in &pa001_violations {
        assert_eq!(
            v["severity"].as_str(),
            Some("info"),
            "PA001 should have severity 'info' from config override, got: {v}"
        );
    }
}

// ----------------------------------------------------------------------------
// Path-specific ignore: L005 violations on bad_dep.go should be suppressed
// ----------------------------------------------------------------------------
#[test]
fn path_specific_ignore_suppresses_l005() {
    let output = boundary_cmd()
        .args([
            "analyze",
            &fixture("rule-config-override"),
            "--format",
            "json",
        ])
        .output()
        .expect("failed to run boundary analyze");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let parsed: serde_json::Value =
        serde_json::from_str(&stdout).expect("output should be valid JSON");

    let violations = parsed["violations"]
        .as_array()
        .expect("should have violations array");

    let l005_violations: Vec<_> = violations
        .iter()
        .filter(|v| v["rule"].as_str() == Some("L005"))
        .collect();

    assert!(
        l005_violations.is_empty(),
        "L005 violations on bad_dep.go should be suppressed by config ignore, but found: {l005_violations:?}"
    );
}

// ----------------------------------------------------------------------------
// Rule ID precedence over category name
// ----------------------------------------------------------------------------
#[test]
fn rule_id_takes_precedence_over_category() {
    // The fixture config sets PA001 = "info" but doesn't set missing_port category.
    // The default for missing_port is "warning". PA001 should override to "info".
    let output = boundary_cmd()
        .args([
            "analyze",
            &fixture("rule-config-override"),
            "--format",
            "json",
        ])
        .output()
        .expect("failed to run boundary analyze");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let parsed: serde_json::Value =
        serde_json::from_str(&stdout).expect("output should be valid JSON");

    let violations = parsed["violations"]
        .as_array()
        .expect("should have violations array");

    // PA001 should be "info" (rule ID), not "warning" (category default)
    for v in violations {
        if v["rule"].as_str() == Some("PA001") {
            assert_eq!(
                v["severity"].as_str(),
                Some("info"),
                "rule ID PA001=info should override category default missing_port=warning"
            );
        }
    }
}

// ----------------------------------------------------------------------------
// L001 violations should still be present (not ignored)
// ----------------------------------------------------------------------------
#[test]
fn non_ignored_violations_remain() {
    let output = boundary_cmd()
        .args([
            "analyze",
            &fixture("rule-config-override"),
            "--format",
            "json",
        ])
        .output()
        .expect("failed to run boundary analyze");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let parsed: serde_json::Value =
        serde_json::from_str(&stdout).expect("output should be valid JSON");

    let violations = parsed["violations"]
        .as_array()
        .expect("should have violations array");

    let l001_violations: Vec<_> = violations
        .iter()
        .filter(|v| v["rule"].as_str() == Some("L001"))
        .collect();

    assert!(
        !l001_violations.is_empty(),
        "L001 violations should not be suppressed (only L005 is ignored)"
    );
}
