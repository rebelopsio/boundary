/// Acceptance tests for FR-12: Markdown Reports.
///
/// Each test maps to a scenario in docs/features/markdown_reports.feature.
/// Run `cargo test --test markdown_report_test` to see the current state.
use std::process::Command;

fn boundary_cmd() -> Command {
    Command::new(env!("CARGO_BIN_EXE_boundary"))
}

fn fixture(name: &str) -> String {
    format!("{}/tests/fixtures/{name}", env!("CARGO_MANIFEST_DIR"))
}

fn analyze_markdown(fixture_name: &str) -> String {
    let path = fixture(fixture_name);
    let output = boundary_cmd()
        .args(["analyze", &path, "--format", "markdown"])
        .output()
        .unwrap_or_else(|e| panic!("failed to run boundary analyze on {fixture_name}: {e}"));

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();

    assert!(
        output.status.success(),
        "boundary analyze --format markdown failed on {fixture_name}: stdout={stdout}, stderr={stderr}"
    );

    stdout
}

// ----------------------------------------------------------------------------
// Scenario: Markdown output contains a Package Metrics section
// Given a project with multiple packages (rcm-ddd-project)
// When I run "boundary analyze . --format markdown"
// Then the output contains a "## Package Metrics" heading
// ----------------------------------------------------------------------------
#[test]
fn markdown_contains_package_metrics_section() {
    let output = analyze_markdown("rcm-ddd-project");

    assert!(
        output.contains("## Package Metrics"),
        "markdown output should contain '## Package Metrics' heading: {output}"
    );
}

// ----------------------------------------------------------------------------
// Scenario: Package metrics table includes A, I, D columns
// Given a project with multiple packages
// When I run "boundary analyze . --format markdown"
// Then the package metrics table has columns A, I, D
// ----------------------------------------------------------------------------
#[test]
fn markdown_package_metrics_has_aid_columns() {
    let output = analyze_markdown("rcm-ddd-project");

    assert!(
        output.contains("## Package Metrics"),
        "markdown output should contain Package Metrics section: {output}"
    );
    assert!(
        output.contains("| A |") || output.contains("| A|"),
        "package metrics table should have an A column: {output}"
    );
    assert!(
        output.contains("| I |") || output.contains("| I|"),
        "package metrics table should have an I column: {output}"
    );
    assert!(
        output.contains("| D |") || output.contains("| D|"),
        "package metrics table should have a D column: {output}"
    );
}

// ----------------------------------------------------------------------------
// Scenario: Zone of Pain package appears with zone annotation
// Given a project with a package in the Zone of Pain (rcm-zone-of-pain)
// When I run "boundary analyze . --format markdown"
// Then the package metrics table shows "⚠ Pain" in the Zone column
// ----------------------------------------------------------------------------
#[test]
fn markdown_zone_of_pain_has_annotation() {
    let output = analyze_markdown("rcm-zone-of-pain");

    assert!(
        output.contains("## Package Metrics"),
        "markdown output should contain Package Metrics section: {output}"
    );
    assert!(
        output.contains("Pain"),
        "zone of pain package should show Pain annotation in markdown: {output}"
    );
}

// ----------------------------------------------------------------------------
// Scenario: Markdown output contains a Pattern Detection section
// Given a project with detectable architectural patterns
// When I run "boundary analyze . --format markdown"
// Then the output contains a "## Pattern Detection" heading
// ----------------------------------------------------------------------------
#[test]
fn markdown_contains_pattern_detection_section() {
    let output = analyze_markdown("pattern-ddd-project");

    assert!(
        output.contains("## Pattern Detection"),
        "markdown output should contain '## Pattern Detection' heading: {output}"
    );
}

// ----------------------------------------------------------------------------
// Scenario: Pattern detection shows the top pattern name
// Given a DDD project
// When I run "boundary analyze . --format markdown"
// Then the Pattern Detection section mentions the top pattern (e.g. "ddd-hexagonal")
// ----------------------------------------------------------------------------
#[test]
fn markdown_pattern_detection_shows_top_pattern() {
    let output = analyze_markdown("pattern-ddd-project");

    assert!(
        output.contains("## Pattern Detection"),
        "markdown output should contain Pattern Detection section: {output}"
    );
    assert!(
        output.contains("Top Pattern:"),
        "Pattern Detection section should show 'Top Pattern:' label: {output}"
    );
    // The top pattern for a DDD project should be one of the recognized patterns
    let has_pattern = output.contains("ddd-hexagonal")
        || output.contains("service-layer")
        || output.contains("anemic-domain")
        || output.contains("flat-crud")
        || output.contains("active-record");
    assert!(
        has_pattern,
        "Pattern Detection section should name a recognized architectural pattern: {output}"
    );
}
