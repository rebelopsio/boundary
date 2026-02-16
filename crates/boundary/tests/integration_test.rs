use std::process::Command;

fn fixture_path() -> String {
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    format!("{manifest_dir}/tests/fixtures/sample-go-project/")
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
