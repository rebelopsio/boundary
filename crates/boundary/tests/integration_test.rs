use std::process::Command;

fn fixture_path() -> String {
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    format!("{manifest_dir}/tests/fixtures/sample-go-project/")
}

fn boundary_cmd() -> Command {
    Command::new(env!("CARGO_BIN_EXE_boundary"))
}

#[test]
fn test_analyze_sample_project() {
    let output = Command::new(env!("CARGO_BIN_EXE_boundary"))
        .args(["analyze", &fixture_path()])
        .output()
        .expect("failed to run boundary analyze");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(
        output.status.success(),
        "boundary analyze failed: stdout={stdout}, stderr={stderr}"
    );
    assert!(
        stdout.contains("Overall Score"),
        "should contain score: {stdout}"
    );
    assert!(
        stdout.contains("Violations") || stdout.contains("No violations"),
        "should mention violations: {stdout}"
    );
}

#[test]
fn test_check_sample_project_fails_on_error() {
    let output = Command::new(env!("CARGO_BIN_EXE_boundary"))
        .args(["check", &fixture_path(), "--fail-on", "error"])
        .output()
        .expect("failed to run boundary check");

    let stdout = String::from_utf8_lossy(&output.stdout);

    // The sample project has a domain->infrastructure violation, so check should fail
    assert!(
        output.status.code() == Some(1),
        "expected exit code 1 for violations, got {:?}: {stdout}",
        output.status.code()
    );
    assert!(
        stdout.contains("CHECK FAILED"),
        "should say CHECK FAILED: {stdout}"
    );
}

#[test]
fn test_init_creates_config() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    let output = Command::new(env!("CARGO_BIN_EXE_boundary"))
        .args(["init"])
        .current_dir(dir.path())
        .output()
        .expect("failed to run boundary init");

    assert!(output.status.success(), "init should succeed");

    let config_path = dir.path().join(".boundary.toml");
    assert!(config_path.exists(), ".boundary.toml should be created");

    let content = std::fs::read_to_string(&config_path).unwrap();
    assert!(
        content.contains("[project]"),
        "should contain [project] section"
    );
    assert!(
        content.contains("[layers]"),
        "should contain [layers] section"
    );
}

#[test]
fn test_init_refuses_overwrite() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    std::fs::write(dir.path().join(".boundary.toml"), "existing").unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_boundary"))
        .args(["init"])
        .current_dir(dir.path())
        .output()
        .expect("failed to run boundary init");

    assert!(
        !output.status.success(),
        "init should fail when file exists"
    );
}

#[test]
fn test_analyze_json_output() {
    let output = boundary_cmd()
        .args(["analyze", &fixture_path(), "--format", "json"])
        .output()
        .expect("failed to run boundary analyze --format json");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        output.status.success(),
        "analyze --format json should succeed: {stdout}"
    );

    let parsed: serde_json::Value =
        serde_json::from_str(stdout.trim()).expect("output should be valid JSON");
    assert!(parsed.get("score").is_some(), "should have score field");
    assert!(
        parsed.get("violations").is_some(),
        "should have violations field"
    );
    assert!(
        parsed.get("component_count").is_some(),
        "should have component_count field"
    );
    assert!(
        parsed["component_count"].as_u64().unwrap() > 0,
        "should find components"
    );
}

#[test]
fn test_analyze_json_compact() {
    let output = boundary_cmd()
        .args(["analyze", &fixture_path(), "--format", "json", "--compact"])
        .output()
        .expect("failed to run boundary analyze --format json --compact");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());

    // Compact JSON: the actual JSON content (first line) should be single-line
    let json_line = stdout.trim();
    assert!(!json_line.is_empty(), "should produce output");
    // Verify it parses as valid JSON
    let _: serde_json::Value =
        serde_json::from_str(json_line).expect("compact output should be valid JSON");
    // Compact output should not have indentation
    assert!(
        !json_line.contains("  \""),
        "compact JSON should not be indented"
    );
}

#[test]
fn test_check_json_with_violations() {
    let output = boundary_cmd()
        .args([
            "check",
            &fixture_path(),
            "--fail-on",
            "error",
            "--format",
            "json",
        ])
        .output()
        .expect("failed to run boundary check --format json");

    let stdout = String::from_utf8_lossy(&output.stdout);

    assert_eq!(
        output.status.code(),
        Some(1),
        "should exit 1 due to violations: {stdout}"
    );

    let parsed: serde_json::Value =
        serde_json::from_str(stdout.trim()).expect("output should be valid JSON");
    assert_eq!(
        parsed["check"]["passed"], false,
        "check.passed should be false"
    );
    assert!(
        parsed["check"]["failing_violation_count"].as_u64().unwrap() > 0,
        "should have failing violations"
    );
}

#[test]
fn test_analyze_nonexistent_path() {
    let output = boundary_cmd()
        .args(["analyze", "/nonexistent/path/that/does/not/exist"])
        .output()
        .expect("failed to run boundary");

    assert_eq!(output.status.code(), Some(2), "should exit 2 for error");

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("does not exist"),
        "should show helpful error message: {stderr}"
    );
}
