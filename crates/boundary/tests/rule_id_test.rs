/// Acceptance tests for the Rule ID system (Phase 1).
///
/// Verifies that rule IDs appear in output, JSON includes rule fields,
/// and --ignore filters violations correctly.
use std::process::Command;

fn boundary_cmd() -> Command {
    Command::new(env!("CARGO_BIN_EXE_boundary"))
}

fn fixture(name: &str) -> String {
    format!("{}/tests/fixtures/{name}", env!("CARGO_MANIFEST_DIR"))
}

// ----------------------------------------------------------------------------
// JSON output includes rule and rule_name fields on violations
// ----------------------------------------------------------------------------
#[test]
fn json_output_includes_rule_fields() {
    let output = boundary_cmd()
        .args([
            "analyze",
            &fixture("domain-imports-infra"),
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
    assert!(!violations.is_empty(), "should have at least one violation");

    let first = &violations[0];
    assert!(
        first.get("rule").is_some(),
        "violation should have 'rule' field: {first}"
    );
    assert!(
        first.get("rule_name").is_some(),
        "violation should have 'rule_name' field: {first}"
    );

    let rule = first["rule"].as_str().unwrap();
    assert!(
        rule == "L001" || rule.starts_with('L'),
        "layer boundary violation should have L-prefixed rule ID: {rule}"
    );
}

// ----------------------------------------------------------------------------
// --ignore suppresses matching violations in analyze output
// ----------------------------------------------------------------------------
#[test]
fn ignore_suppresses_violations_in_analyze() {
    // First run without ignore to get baseline count
    let baseline = boundary_cmd()
        .args([
            "analyze",
            &fixture("domain-imports-infra"),
            "--format",
            "json",
        ])
        .output()
        .expect("failed to run boundary analyze");

    let baseline_json: serde_json::Value =
        serde_json::from_str(&String::from_utf8_lossy(&baseline.stdout))
            .expect("baseline should be valid JSON");
    let baseline_count = baseline_json["violations"].as_array().unwrap().len();
    assert!(baseline_count > 0, "baseline should have violations");

    // Now run with --ignore L001
    let filtered = boundary_cmd()
        .args([
            "analyze",
            &fixture("domain-imports-infra"),
            "--format",
            "json",
            "--ignore",
            "L001",
        ])
        .output()
        .expect("failed to run boundary analyze with --ignore");

    let filtered_json: serde_json::Value =
        serde_json::from_str(&String::from_utf8_lossy(&filtered.stdout))
            .expect("filtered should be valid JSON");
    let filtered_count = filtered_json["violations"].as_array().unwrap().len();

    assert!(
        filtered_count < baseline_count,
        "--ignore L001 should reduce violation count: baseline={baseline_count}, filtered={filtered_count}"
    );

    // Verify no L001 violations remain
    for v in filtered_json["violations"].as_array().unwrap() {
        assert_ne!(
            v["rule"].as_str().unwrap(),
            "L001",
            "L001 violations should be filtered out"
        );
    }
}

// ----------------------------------------------------------------------------
// --ignore affects check exit code (ignored violations don't cause failure)
// ----------------------------------------------------------------------------
#[test]
fn ignore_affects_check_exit_code() {
    // Without --ignore, check should fail (domain-imports-infra has L001 errors)
    let without_ignore = boundary_cmd()
        .args(["check", &fixture("domain-imports-infra")])
        .output()
        .expect("failed to run boundary check");

    assert!(
        !without_ignore.status.success(),
        "check should fail without --ignore"
    );

    // With --ignore L001,L005, check should pass if those were the only error-level violations
    let with_ignore = boundary_cmd()
        .args([
            "check",
            &fixture("domain-imports-infra"),
            "--ignore",
            "L001,L005",
        ])
        .output()
        .expect("failed to run boundary check with --ignore");

    assert!(
        with_ignore.status.success(),
        "check should pass when all error violations are ignored: {}",
        String::from_utf8_lossy(&with_ignore.stdout)
    );
}

// ----------------------------------------------------------------------------
// Text output shows rule IDs in violation lines
// ----------------------------------------------------------------------------
#[test]
fn text_output_shows_rule_ids() {
    let output = boundary_cmd()
        .args(["analyze", &fixture("domain-imports-infra")])
        .output()
        .expect("failed to run boundary analyze");

    let stdout = String::from_utf8_lossy(&output.stdout);

    // Should contain rule ID like "L001" in the violation output
    assert!(
        stdout.contains("L001"),
        "text output should show rule ID L001: {stdout}"
    );
    assert!(
        stdout.contains("domain-depends-on-infrastructure"),
        "text output should show rule name: {stdout}"
    );
}

// ----------------------------------------------------------------------------
// Markdown output shows rule IDs in violation table
// ----------------------------------------------------------------------------
#[test]
fn markdown_output_shows_rule_ids() {
    let output = boundary_cmd()
        .args([
            "analyze",
            &fixture("domain-imports-infra"),
            "--format",
            "markdown",
        ])
        .output()
        .expect("failed to run boundary analyze");

    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(
        stdout.contains("L001"),
        "markdown output should show rule ID L001: {stdout}"
    );
    assert!(
        stdout.contains("domain-depends-on-infrastructure"),
        "markdown output should show rule name: {stdout}"
    );
}
