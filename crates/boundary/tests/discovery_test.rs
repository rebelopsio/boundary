/// Acceptance tests for the Architecture Discovery feature.
///
/// Each test maps directly to a scenario in docs/features/01-discovery.feature.
/// These tests are intentionally RED — they define the desired CLI behavior
/// before the implementation exists. Run `cargo test --test discovery_test`
/// to see the current failure state.
use std::process::Command;

fn boundary_cmd() -> Command {
    Command::new(env!("CARGO_BIN_EXE_boundary"))
}

fn fixture(name: &str) -> String {
    format!("{}/tests/fixtures/{name}", env!("CARGO_MANIFEST_DIR"))
}

// ----------------------------------------------------------------------------
// Scenario: Codebase with no recognizable architectural structure
// Given a Go module with no directories matching known DDD layer patterns
// When I run "boundary analyze ."
// Then the report states that no architectural layers were detected
// And the exit code is 0
// ----------------------------------------------------------------------------
#[test]
fn discovery_no_recognizable_structure() {
    let output = boundary_cmd()
        .args(["analyze", &fixture("flat-go-module")])
        .output()
        .expect("failed to run boundary analyze");

    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(
        output.status.success(),
        "exit code should be 0: stdout={stdout}"
    );
    assert!(
        stdout.contains("No architectural layers detected"),
        "should report no layers detected: {stdout}"
    );
}

// ----------------------------------------------------------------------------
// Scenario: Codebase with complete DDD layer structure reports all layer components
// Given a Go module with "domain", "application", and "infrastructure" directories
// And each directory contains at least one Go type
// When I run "boundary analyze ."
// Then the report lists components found in the "domain" layer
// And the report lists components found in the "application" layer
// And the report lists components found in the "infrastructure" layer
// ----------------------------------------------------------------------------
#[test]
fn discovery_complete_ddd_reports_all_layers() {
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
        stdout.contains("Domain"),
        "should list Domain layer components: {stdout}"
    );
    assert!(
        stdout.contains("Application"),
        "should list Application layer components: {stdout}"
    );
    assert!(
        stdout.contains("Infrastructure"),
        "should list Infrastructure layer components: {stdout}"
    );
}

// ----------------------------------------------------------------------------
// Scenario: Codebase where all components map to DDD layers receives full structural presence
// Given a Go module containing only "domain", "application", and "infrastructure" directories
// And each directory contains at least one Go type
// When I run "boundary analyze ."
// Then the output contains "Structural Presence: 100%"
// ----------------------------------------------------------------------------
#[test]
fn discovery_complete_ddd_structural_presence_100() {
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
        stdout.contains("Structural Presence: 100%"),
        "should report Structural Presence: 100%: {stdout}"
    );
}

// ----------------------------------------------------------------------------
// Scenario: Codebase with partial DDD structure reports classified and unclassified directories
// Given a Go module with a "domain" directory and a "services" directory
// And "domain" matches a known DDD layer pattern
// And "services" does not match any known DDD layer pattern
// When I run "boundary analyze ."
// Then the report lists components found in the "domain" layer
// And the report identifies "services" as an unclassified directory
// ----------------------------------------------------------------------------
#[test]
fn discovery_partial_ddd_reports_classified_and_unclassified() {
    let output = boundary_cmd()
        .args(["analyze", &fixture("partial-ddd-module")])
        .output()
        .expect("failed to run boundary analyze");

    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(
        output.status.success(),
        "exit code should be 0: stdout={stdout}"
    );
    assert!(
        stdout.contains("Domain"),
        "should list Domain layer components: {stdout}"
    );
    assert!(
        stdout.to_lowercase().contains("unclassified") && stdout.contains("services"),
        "should identify 'services' as an unclassified directory: {stdout}"
    );
}

// ----------------------------------------------------------------------------
// Scenario: Codebase with unclassified directories prompts the user to add configuration
// Given a Go module with a "domain" directory and a "services" directory
// And "domain" matches a known DDD layer pattern
// And "services" does not match any known DDD layer pattern
// When I run "boundary analyze ."
// Then the report suggests adding a .boundary.toml to classify unrecognized directories
// ----------------------------------------------------------------------------
#[test]
fn discovery_unclassified_directories_suggests_config() {
    let output = boundary_cmd()
        .args(["analyze", &fixture("partial-ddd-module")])
        .output()
        .expect("failed to run boundary analyze");

    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(
        output.status.success(),
        "exit code should be 0: stdout={stdout}"
    );
    assert!(
        stdout.contains(".boundary.toml"),
        "should suggest adding a .boundary.toml: {stdout}"
    );
}

// ----------------------------------------------------------------------------
// Scenario: Configuration override assigns an unrecognized directory to a specified layer
// Given a Go module with an "adapters" directory containing Go types
// And "adapters" does not match any default layer pattern
// And a .boundary.toml that classifies "adapters/**" as the Infrastructure layer
// When I run "boundary analyze ."
// Then components in "adapters" are reported as belonging to the Infrastructure layer
// ----------------------------------------------------------------------------
#[test]
fn discovery_override_assigns_directory_to_layer() {
    let output = boundary_cmd()
        .args(["analyze", &fixture("adapters-override")])
        .output()
        .expect("failed to run boundary analyze");

    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(
        output.status.success(),
        "exit code should be 0: stdout={stdout}"
    );
    assert!(
        stdout.contains("Infrastructure"),
        "should report adapters components as Infrastructure layer: {stdout}"
    );
}

// ----------------------------------------------------------------------------
// Scenario: Target path does not exist
// Given no directory exists at the specified path
// When I run "boundary analyze /tmp/nonexistent"
// Then the report states the target path could not be found
// And the exit code is non-zero
// ----------------------------------------------------------------------------
#[test]
fn discovery_nonexistent_path_errors() {
    // Create and immediately drop a temp dir to get a guaranteed nonexistent path
    let path = {
        let dir = tempfile::tempdir().expect("failed to create temp dir");
        dir.path().to_path_buf()
    };

    let output = boundary_cmd()
        .args(["analyze", path.to_str().unwrap()])
        .output()
        .expect("failed to run boundary analyze");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let combined = format!("{stdout}{stderr}");

    assert!(
        !output.status.success(),
        "exit code should be non-zero for nonexistent path"
    );
    assert!(
        combined.to_lowercase().contains("not found")
            || combined.to_lowercase().contains("does not exist")
            || combined.to_lowercase().contains("no such file"),
        "should report the path could not be found: {combined}"
    );
}

// ----------------------------------------------------------------------------
// Scenario: Target directory contains no Go files
// Given a directory containing only non-Go files
// When I run "boundary analyze ."
// Then the report states that no supported source files were found
// And the exit code is 0
// ----------------------------------------------------------------------------
#[test]
fn discovery_no_go_files_reports_no_source_files() {
    let output = boundary_cmd()
        .args(["analyze", &fixture("no-go-files")])
        .output()
        .expect("failed to run boundary analyze");

    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(
        output.status.success(),
        "exit code should be 0: stdout={stdout}"
    );
    assert!(
        stdout.to_lowercase().contains("no supported source files"),
        "should report no supported source files were found: {stdout}"
    );
}

// ----------------------------------------------------------------------------
// Scenario: Target directory contains Go files but no extractable components
// Given a directory containing Go files with no exported types
// When I run "boundary analyze ."
// Then the report states that no components were detected in the analyzed files
// And the exit code is 0
// ----------------------------------------------------------------------------
#[test]
fn discovery_go_files_no_exported_types_reports_no_components() {
    let output = boundary_cmd()
        .args(["analyze", &fixture("no-exported-types")])
        .output()
        .expect("failed to run boundary analyze");

    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(
        output.status.success(),
        "exit code should be 0: stdout={stdout}"
    );
    assert!(
        stdout
            .to_lowercase()
            .contains("no components were detected"),
        "should report no components were detected in the analyzed files: {stdout}"
    );
}
