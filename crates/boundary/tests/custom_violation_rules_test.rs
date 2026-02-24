/// Acceptance tests for FR-7: Custom Violation Rules.
///
/// Each test maps to a scenario in docs/features/custom_violation_rules.feature.
/// Run `cargo test --test custom_violation_rules_test` to see the current state.
use std::process::Command;

fn boundary_cmd() -> Command {
    Command::new(env!("CARGO_BIN_EXE_boundary"))
}

fn fixture(name: &str) -> String {
    format!("{}/tests/fixtures/{name}", env!("CARGO_MANIFEST_DIR"))
}

// ----------------------------------------------------------------------------
// Scenario: Custom rule matching produces a CustomRule violation
// Given a project with a .boundary.toml defining a deny rule from "domain" to "external"
// And the domain package imports from the external package
// When I run "boundary analyze ."
// Then the output contains a "custom" violation type
// ----------------------------------------------------------------------------
#[test]
fn custom_rule_matching_produces_custom_violation() {
    let output = boundary_cmd()
        .args(["analyze", &fixture("fr7-custom-rules")])
        .output()
        .expect("failed to run boundary analyze");

    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(
        output.status.success(),
        "exit code should be 0: stdout={stdout}"
    );
    assert!(
        stdout.contains("custom"),
        "output should contain a custom rule violation type: {stdout}"
    );
}

// ----------------------------------------------------------------------------
// Scenario: Custom violation has the configured severity (warning)
// Given a project with a custom rule configured with severity "warning"
// When I run "boundary analyze ."
// Then the violation is reported with severity "warning" (or "WARN")
// ----------------------------------------------------------------------------
#[test]
fn custom_rule_violation_has_configured_severity() {
    let output = boundary_cmd()
        .args(["analyze", &fixture("fr7-custom-rules")])
        .output()
        .expect("failed to run boundary analyze");

    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(
        output.status.success(),
        "exit code should be 0: stdout={stdout}"
    );
    assert!(
        stdout.to_lowercase().contains("warn"),
        "custom violation should have warning severity: {stdout}"
    );
}

// ----------------------------------------------------------------------------
// Scenario: Custom violation message is set from the config
// Given a custom rule with message "Domain must not import external packages"
// When a matching dependency is found
// Then the output includes the configured violation message
// ----------------------------------------------------------------------------
#[test]
fn custom_rule_violation_message_from_config() {
    let output = boundary_cmd()
        .args(["analyze", &fixture("fr7-custom-rules")])
        .output()
        .expect("failed to run boundary analyze");

    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(
        output.status.success(),
        "exit code should be 0: stdout={stdout}"
    );
    assert!(
        stdout.contains("Domain must not import external packages"),
        "custom violation should include the configured message: {stdout}"
    );
}

// ----------------------------------------------------------------------------
// Scenario: Rule that doesn't match produces no CustomRule violation
// Given a project with no dependencies matching the custom rule pattern
// When I run "boundary analyze ."
// Then the output does not contain a custom rule violation
// ----------------------------------------------------------------------------
#[test]
fn non_matching_rule_produces_no_custom_violation() {
    // full-ddd-module has no domain->external dependency, so the custom rule
    // defined in fr7-custom-rules/.boundary.toml should not fire here.
    // We run against full-ddd-module which has no custom rules defined at all.
    let output = boundary_cmd()
        .args(["analyze", &fixture("full-ddd-module")])
        .output()
        .expect("failed to run boundary analyze");

    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(
        output.status.success(),
        "exit code should be 0: stdout={stdout}"
    );
    assert!(
        !stdout.contains("custom:"),
        "output should not contain custom rule violations when none match: {stdout}"
    );
}

// ----------------------------------------------------------------------------
// Scenario: check command exits 0 for custom-rule-only violations when fail-on is error
// Given a project with a custom rule at severity "warning"
// When I run "boundary check . --fail-on error"
// Then the exit code is 0 (warnings do not trigger failure at error threshold)
// ----------------------------------------------------------------------------
#[test]
fn custom_rule_warning_does_not_trigger_check_at_error_threshold() {
    let output = boundary_cmd()
        .args(["check", &fixture("fr7-custom-rules"), "--fail-on", "error"])
        .output()
        .expect("failed to run boundary check");

    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(
        output.status.success(),
        "check --fail-on error should exit 0 when only warning-level custom violations present: stdout={stdout}"
    );
}
