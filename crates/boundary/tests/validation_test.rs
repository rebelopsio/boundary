/// Acceptance tests for the Architecture Validation feature.
///
/// Each test maps directly to a scenario in docs/features/02-validation.feature.
/// Run `cargo test --test validation_test` to see the current state.
use std::process::Command;

fn boundary_cmd() -> Command {
    Command::new(env!("CARGO_BIN_EXE_boundary"))
}

fn fixture(name: &str) -> String {
    format!("{}/tests/fixtures/{name}", env!("CARGO_MANIFEST_DIR"))
}

// ----------------------------------------------------------------------------
// Scenario: Codebase with correct layering reports no violations
// Given a Go module with "domain", "application", and "infrastructure" directories
// And each directory contains at least one Go type
// And no component imports across a forbidden layer boundary
// When I run "boundary analyze ."
// Then the report states that no violations were found
// ----------------------------------------------------------------------------
#[test]
fn validation_clean_layering_no_violations() {
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
        stdout.contains("No violations found"),
        "should state no violations were found: {stdout}"
    );
}

// ----------------------------------------------------------------------------
// Scenario: Domain component importing infrastructure is reported as a layer boundary violation
// Given a Go module where a type in the "domain" directory imports from the "infrastructure" directory
// When I run "boundary analyze ."
// Then the report identifies a "layer boundary violation" between "domain" and "infrastructure"
// And the violation includes a suggestion for how to resolve it
// ----------------------------------------------------------------------------
#[test]
fn validation_domain_imports_infra_layer_boundary_violation() {
    let output = boundary_cmd()
        .args(["analyze", &fixture("domain-imports-infra")])
        .output()
        .expect("failed to run boundary analyze");

    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(
        output.status.success(),
        "exit code should be 0: stdout={stdout}"
    );
    assert!(
        stdout.contains("Domain") && stdout.contains("Infrastructure"),
        "should identify a layer boundary violation between domain and infrastructure: {stdout}"
    );
    assert!(
        stdout.contains("Suggestion"),
        "violation should include a suggestion for how to resolve it: {stdout}"
    );
}

// ----------------------------------------------------------------------------
// Scenario: analyze command always exits zero regardless of violations
// Given a Go module where a type in the "domain" directory imports from the "infrastructure" directory
// When I run "boundary analyze ."
// Then the exit code is 0
// ----------------------------------------------------------------------------
#[test]
fn validation_analyze_always_exits_zero() {
    let output = boundary_cmd()
        .args(["analyze", &fixture("domain-imports-infra")])
        .output()
        .expect("failed to run boundary analyze");

    assert!(
        output.status.success(),
        "analyze should always exit 0, even with violations"
    );
}

// ----------------------------------------------------------------------------
// Scenario: check command exits non-zero when violations meet the default fail-on threshold
// Given a Go module where a type in the "domain" directory imports from the "infrastructure" directory
// When I run "boundary check ."
// Then the exit code is non-zero
// ----------------------------------------------------------------------------
#[test]
fn validation_check_exits_nonzero_on_default_threshold() {
    let output = boundary_cmd()
        .args(["check", &fixture("domain-imports-infra")])
        .output()
        .expect("failed to run boundary check");

    assert!(
        !output.status.success(),
        "check should exit non-zero when layer boundary violations are present"
    );
}

// ----------------------------------------------------------------------------
// Scenario: check command exits zero when only warning-level violations are present and fail-on is error
// Given a Go module with an infrastructure adapter and no matching domain port interface
// And boundary reports this condition as a "missing port" warning
// When I run "boundary check . --fail-on error"
// Then the exit code is 0
// ----------------------------------------------------------------------------
#[test]
fn validation_check_fail_on_error_passes_for_warnings() {
    let output = boundary_cmd()
        .args(["check", &fixture("adapters-override"), "--fail-on", "error"])
        .output()
        .expect("failed to run boundary check");

    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(
        output.status.success(),
        "check --fail-on error should exit 0 when only warning-level violations are present: stdout={stdout}"
    );
}

// ----------------------------------------------------------------------------
// Scenario: check command exits non-zero when fail-on threshold is lowered to warning
// Given a Go module with an infrastructure adapter and no matching domain port interface
// And boundary reports this condition as a "missing port" warning
// When I run "boundary check . --fail-on warning"
// Then the exit code is non-zero
// ----------------------------------------------------------------------------
#[test]
fn validation_check_fail_on_warning_exits_nonzero() {
    let output = boundary_cmd()
        .args([
            "check",
            &fixture("adapters-override"),
            "--fail-on",
            "warning",
        ])
        .output()
        .expect("failed to run boundary check");

    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(
        !output.status.success(),
        "check --fail-on warning should exit non-zero when missing port warnings are present: stdout={stdout}"
    );
}

// ----------------------------------------------------------------------------
// Scenario: check command exits zero but does not claim a clean architecture when no layers are detected
// Given a Go module where no directories match any known DDD layer pattern
// When I run "boundary check ."
// Then the exit code is 0
// And the report states that no architectural layers were detected
// And the report does not state that no violations were found
// ----------------------------------------------------------------------------
#[test]
fn validation_check_no_layers_exits_zero_without_clean_claim() {
    let output = boundary_cmd()
        .args(["check", &fixture("flat-go-module")])
        .output()
        .expect("failed to run boundary check");

    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(
        output.status.success(),
        "check should exit 0 when no layers are detected: stdout={stdout}"
    );
    assert!(
        stdout.contains("No architectural layers detected"),
        "should state that no architectural layers were detected: {stdout}"
    );
    assert!(
        !stdout.contains("No violations found"),
        "should not claim a clean architecture when no layers are detected: {stdout}"
    );
}
