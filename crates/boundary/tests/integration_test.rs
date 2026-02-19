use std::process::Command;

fn fixture_path() -> String {
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    format!("{manifest_dir}/tests/fixtures/sample-go-project/")
}

fn ts_fixture_path() -> String {
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    format!("{manifest_dir}/tests/fixtures/sample-ts-project/")
}

fn java_fixture_path() -> String {
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    format!("{manifest_dir}/tests/fixtures/sample-java-project/")
}

fn rust_fixture_path() -> String {
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    format!("{manifest_dir}/tests/fixtures/sample-rust-project/")
}

fn boundary_cmd() -> Command {
    Command::new(env!("CARGO_BIN_EXE_boundary"))
}

// ==================== Go analyzer tests (existing) ====================

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

// ==================== TypeScript analyzer tests ====================

#[test]
fn test_analyze_typescript_project() {
    let output = boundary_cmd()
        .args(["analyze", &ts_fixture_path()])
        .output()
        .expect("failed to run boundary analyze on TS fixture");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(
        output.status.success(),
        "TS analyze failed: stdout={stdout}, stderr={stderr}"
    );
    assert!(
        stdout.contains("Overall Score"),
        "should contain score: {stdout}"
    );
}

#[test]
fn test_check_typescript_violations() {
    let output = boundary_cmd()
        .args(["check", &ts_fixture_path(), "--fail-on", "error"])
        .output()
        .expect("failed to run boundary check on TS fixture");

    let stdout = String::from_utf8_lossy(&output.stdout);

    assert_eq!(
        output.status.code(),
        Some(1),
        "expected exit code 1 for TS violations, got {:?}: {stdout}",
        output.status.code()
    );
    assert!(
        stdout.contains("CHECK FAILED"),
        "should say CHECK FAILED: {stdout}"
    );
}

#[test]
fn test_analyze_typescript_json() {
    let output = boundary_cmd()
        .args(["analyze", &ts_fixture_path(), "--format", "json"])
        .output()
        .expect("failed to run boundary analyze --format json on TS fixture");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        output.status.success(),
        "TS JSON analyze should succeed: {stdout}"
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
        "should find TS components"
    );
}

// ==================== Java analyzer tests ====================

#[test]
fn test_analyze_java_project() {
    let output = boundary_cmd()
        .args(["analyze", &java_fixture_path()])
        .output()
        .expect("failed to run boundary analyze on Java fixture");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(
        output.status.success(),
        "Java analyze failed: stdout={stdout}, stderr={stderr}"
    );
    assert!(
        stdout.contains("Overall Score"),
        "should contain score: {stdout}"
    );
}

#[test]
fn test_check_java_violations() {
    let output = boundary_cmd()
        .args(["check", &java_fixture_path(), "--fail-on", "error"])
        .output()
        .expect("failed to run boundary check on Java fixture");

    let stdout = String::from_utf8_lossy(&output.stdout);

    assert_eq!(
        output.status.code(),
        Some(1),
        "expected exit code 1 for Java violations, got {:?}: {stdout}",
        output.status.code()
    );
    assert!(
        stdout.contains("CHECK FAILED"),
        "should say CHECK FAILED: {stdout}"
    );
}

#[test]
fn test_analyze_java_json() {
    let output = boundary_cmd()
        .args(["analyze", &java_fixture_path(), "--format", "json"])
        .output()
        .expect("failed to run boundary analyze --format json on Java fixture");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        output.status.success(),
        "Java JSON analyze should succeed: {stdout}"
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
        "should find Java components"
    );
}

// ==================== Rust analyzer tests ====================

#[test]
fn test_analyze_rust_project() {
    let output = boundary_cmd()
        .args(["analyze", &rust_fixture_path()])
        .output()
        .expect("failed to run boundary analyze on Rust fixture");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(
        output.status.success(),
        "Rust analyze failed: stdout={stdout}, stderr={stderr}"
    );
    assert!(
        stdout.contains("Overall Score"),
        "should contain score: {stdout}"
    );
}

#[test]
fn test_check_rust_violations() {
    let output = boundary_cmd()
        .args(["check", &rust_fixture_path(), "--fail-on", "error"])
        .output()
        .expect("failed to run boundary check on Rust fixture");

    let stdout = String::from_utf8_lossy(&output.stdout);

    // Rust fixture has domain->infrastructure violation (domain/user/mod.rs imports infrastructure::postgres)
    assert_eq!(
        output.status.code(),
        Some(1),
        "expected exit code 1 for Rust violations, got {:?}: {stdout}",
        output.status.code()
    );
    assert!(
        stdout.contains("CHECK FAILED"),
        "should say CHECK FAILED: {stdout}"
    );
}

// ==================== Score regression tests ====================

/// Parse --score-only --format=json output into (overall, presence, layer, deps, interfaces).
fn parse_score_json(stdout: &str) -> (f64, f64, f64, f64, f64) {
    let parsed: serde_json::Value =
        serde_json::from_str(stdout.trim()).expect("score-only JSON should be valid");
    (
        parsed["overall"].as_f64().unwrap(),
        parsed["structural_presence"].as_f64().unwrap(),
        parsed["layer_isolation"].as_f64().unwrap(),
        parsed["dependency_direction"].as_f64().unwrap(),
        parsed["interface_coverage"].as_f64().unwrap(),
    )
}

fn assert_score_near(actual: f64, expected: f64, tolerance: f64, label: &str) {
    assert!(
        (actual - expected).abs() <= tolerance,
        "{label}: expected ~{expected} (±{tolerance}), got {actual}"
    );
}

#[test]
fn test_score_go_fixture() {
    let output = boundary_cmd()
        .args([
            "analyze",
            &fixture_path(),
            "--score-only",
            "--format",
            "json",
        ])
        .output()
        .expect("failed to run boundary");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success(), "analyze failed: {stdout}");

    let (overall, _presence, layer, deps, iface) = parse_score_json(&stdout);

    // Go fixture has a domain->infra violation, so scores should be imperfect
    assert_score_near(overall, 73.3, 5.0, "go overall");
    assert_score_near(layer, 66.7, 5.0, "go layer_isolation");
    assert_score_near(deps, 66.7, 5.0, "go dependency_direction");
    assert_score_near(iface, 100.0, 1.0, "go interface_coverage");
}

#[test]
fn test_score_ts_fixture() {
    let output = boundary_cmd()
        .args([
            "analyze",
            &ts_fixture_path(),
            "--score-only",
            "--format",
            "json",
        ])
        .output()
        .expect("failed to run boundary");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success(), "analyze failed: {stdout}");

    let (overall, _presence, layer, deps, iface) = parse_score_json(&stdout);

    assert_score_near(overall, 84.0, 5.0, "ts overall");
    assert_score_near(layer, 80.0, 5.0, "ts layer_isolation");
    assert_score_near(deps, 80.0, 5.0, "ts dependency_direction");
    assert_score_near(iface, 100.0, 1.0, "ts interface_coverage");
}

#[test]
fn test_score_java_fixture() {
    let output = boundary_cmd()
        .args([
            "analyze",
            &java_fixture_path(),
            "--score-only",
            "--format",
            "json",
        ])
        .output()
        .expect("failed to run boundary");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success(), "analyze failed: {stdout}");

    let (overall, _presence, layer, deps, iface) = parse_score_json(&stdout);

    // Java fixture has violations and unclassified components;
    // structural presence gate reduces the overall score further
    assert_score_near(overall, 14.5, 5.0, "java overall");
    assert!(
        layer <= 10.0,
        "java layer_isolation should be low, got {layer}"
    );
    assert!(
        deps <= 10.0,
        "java dependency_direction should be low, got {deps}"
    );
    assert_score_near(iface, 100.0, 1.0, "java interface_coverage");
}

#[test]
fn test_score_rust_fixture() {
    let output = boundary_cmd()
        .args([
            "analyze",
            &rust_fixture_path(),
            "--score-only",
            "--format",
            "json",
        ])
        .output()
        .expect("failed to run boundary");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success(), "analyze failed: {stdout}");

    let (overall, _presence, layer, deps, iface) = parse_score_json(&stdout);

    // Rust fixture has domain->infra violation
    assert_score_near(overall, 20.0, 5.0, "rust overall");
    assert!(
        layer <= 10.0,
        "rust layer_isolation should be low, got {layer}"
    );
    assert!(
        deps <= 10.0,
        "rust dependency_direction should be low, got {deps}"
    );
    assert_score_near(iface, 100.0, 1.0, "rust interface_coverage");
}

#[test]
fn test_score_not_all_100() {
    // Regression test: ensure fixtures with violations don't score a perfect 100.
    // This catches the PR#39 bug where external deps were auto-marked cross-cutting.
    for (name, path) in [
        ("go", fixture_path()),
        ("ts", ts_fixture_path()),
        ("java", java_fixture_path()),
        ("rust", rust_fixture_path()),
    ] {
        let output = boundary_cmd()
            .args(["analyze", &path, "--score-only", "--format", "json"])
            .output()
            .unwrap();
        let stdout = String::from_utf8_lossy(&output.stdout);
        let (overall, _, _, _, _) = parse_score_json(&stdout);
        assert!(
            overall < 100.0,
            "{name} fixture should NOT score 100.0 (has known violations), got {overall}"
        );
    }
}

// ==================== Cross-language and output format tests ====================

#[test]
fn test_analyze_all_languages_detected() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    let dest = dir.path();

    // Copy all fixtures into a single directory
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    let fixtures_dir = format!("{manifest_dir}/tests/fixtures");

    // Copy files from each fixture into the temp dir with language-appropriate structure
    fn copy_dir_recursive(src: &std::path::Path, dst: &std::path::Path) {
        std::fs::create_dir_all(dst).unwrap();
        for entry in std::fs::read_dir(src).unwrap() {
            let entry = entry.unwrap();
            let target = dst.join(entry.file_name());
            if entry.file_type().unwrap().is_dir() {
                copy_dir_recursive(&entry.path(), &target);
            } else {
                std::fs::copy(entry.path(), &target).unwrap();
            }
        }
    }

    // Copy each language fixture into a subdirectory
    copy_dir_recursive(
        std::path::Path::new(&format!("{fixtures_dir}/sample-go-project")),
        &dest.join("go-code"),
    );
    copy_dir_recursive(
        std::path::Path::new(&format!("{fixtures_dir}/sample-ts-project")),
        &dest.join("ts-code"),
    );
    copy_dir_recursive(
        std::path::Path::new(&format!("{fixtures_dir}/sample-java-project")),
        &dest.join("java-code"),
    );
    copy_dir_recursive(
        std::path::Path::new(&format!("{fixtures_dir}/sample-rust-project")),
        &dest.join("rust-code"),
    );

    let output = boundary_cmd()
        .args(["analyze", dest.to_str().unwrap(), "--format", "json"])
        .output()
        .expect("failed to run boundary analyze on multi-language dir");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        output.status.success(),
        "multi-language analyze should succeed: {stdout}"
    );

    let parsed: serde_json::Value =
        serde_json::from_str(stdout.trim()).expect("output should be valid JSON");

    // Should find components from multiple languages
    let count = parsed["component_count"].as_u64().unwrap();
    assert!(
        count >= 4,
        "should find components from multiple languages, got {count}"
    );
}

#[test]
fn test_markdown_output() {
    let output = boundary_cmd()
        .args(["analyze", &fixture_path(), "--format", "markdown"])
        .output()
        .expect("failed to run boundary analyze --format markdown");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        output.status.success(),
        "markdown analyze should succeed: {stdout}"
    );

    // Markdown output should contain expected sections
    assert!(
        stdout.contains('#'),
        "markdown should contain headings: {stdout}"
    );
    assert!(
        stdout.contains("Score") || stdout.contains("score"),
        "markdown should contain score section: {stdout}"
    );
    assert!(
        stdout.contains("Violation") || stdout.contains("violation"),
        "markdown should mention violations: {stdout}"
    );
}
