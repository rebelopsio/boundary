/// Acceptance tests for FR-27: Pattern Detection with Confidence Distribution.
///
/// Each test maps to a scenario in docs/features/05-pattern-detection.feature.
/// Run `cargo test --test pattern_detection_test` to check the current state.
///
/// All tests are RED until FR-27 is implemented in boundary-core.
use std::process::Command;

fn boundary_cmd() -> Command {
    Command::new(env!("CARGO_BIN_EXE_boundary"))
}

fn fixture(name: &str) -> String {
    format!("{}/tests/fixtures/{name}", env!("CARGO_MANIFEST_DIR"))
}

/// Run `boundary analyze <fixture> --format json` and return the parsed JSON.
/// Panics with a helpful message if the command fails or output is not valid JSON.
fn analyze_json(fixture_name: &str) -> serde_json::Value {
    let path = fixture(fixture_name);
    let output = boundary_cmd()
        .args(["analyze", &path, "--format", "json"])
        .output()
        .unwrap_or_else(|e| panic!("failed to run boundary analyze on {fixture_name}: {e}"));

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(
        output.status.success(),
        "boundary analyze failed on {fixture_name}: stdout={stdout}, stderr={stderr}"
    );

    serde_json::from_str(stdout.trim())
        .unwrap_or_else(|e| panic!("invalid JSON from {fixture_name}: {e}\noutput: {stdout}"))
}

/// Run `boundary analyze <fixture>` (text output) and return the stdout string.
/// Panics with a helpful message if the command fails.
fn analyze_text(fixture_name: &str) -> String {
    let path = fixture(fixture_name);
    let output = boundary_cmd()
        .args(["analyze", &path])
        .output()
        .unwrap_or_else(|e| panic!("failed to run boundary analyze (text) on {fixture_name}: {e}"));

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(
        output.status.success(),
        "boundary analyze (text) failed on {fixture_name}: stdout={stdout}, stderr={stderr}"
    );

    stdout.to_string()
}

/// Find a pattern entry in the `pattern_detection.patterns` array by name.
fn find_pattern<'a>(json: &'a serde_json::Value, name: &str) -> Option<&'a serde_json::Value> {
    json["pattern_detection"]["patterns"]
        .as_array()?
        .iter()
        .find(|entry| entry["name"].as_str() == Some(name))
}

/// Return the confidence value for a named pattern, panicking if not found or not a number.
fn pattern_confidence(json: &serde_json::Value, name: &str) -> f64 {
    let entry = find_pattern(json, name).unwrap_or_else(|| {
        panic!("pattern '{name}' not found in pattern_detection.patterns: {json}")
    });
    entry["confidence"]
        .as_f64()
        .unwrap_or_else(|| panic!("confidence for '{name}' is not a number: {entry}"))
}

// ============================================================================
// @contract — JSON output shape
// Background fixture: pattern-ddd-project
//   domain: 1 interface + 1 struct, no imports
//   application: 1 struct, imports domain
//   infrastructure: 1 struct, imports domain
// ============================================================================

/// @contract Scenario: Pattern detection appears in JSON output
///
/// Asserts:
///   - `pattern_detection` object is present
///   - `patterns` array contains entries for all 5 named patterns
///   - each entry has `name` and `confidence` fields
///   - every `confidence` value is in [0.0, 1.0]
///   - `top_pattern` is a string field
///   - `top_confidence` is a number field
#[test]
fn pattern_detection_json_shape_contract() {
    let json = analyze_json("pattern-ddd-project");

    let pd = json
        .get("pattern_detection")
        .unwrap_or_else(|| panic!("'pattern_detection' object missing from JSON: {json}"));

    let patterns = pd["patterns"]
        .as_array()
        .unwrap_or_else(|| panic!("'pattern_detection.patterns' is not an array: {pd}"));

    let expected_names = [
        "ddd-hexagonal",
        "active-record",
        "flat-crud",
        "anemic-domain",
        "service-layer",
    ];
    for name in &expected_names {
        let entry = patterns
            .iter()
            .find(|e| e["name"].as_str() == Some(name))
            .unwrap_or_else(|| {
                panic!("pattern '{name}' missing from patterns array: {patterns:?}")
            });
        assert!(
            entry.get("confidence").is_some(),
            "pattern '{name}' is missing 'confidence' field: {entry}"
        );
        let conf = entry["confidence"]
            .as_f64()
            .unwrap_or_else(|| panic!("confidence for '{name}' is not a number: {entry}"));
        assert!(
            (0.0..=1.0).contains(&conf),
            "confidence for '{name}' is out of [0.0, 1.0]: {conf}"
        );
    }

    assert!(
        pd.get("top_pattern").and_then(|v| v.as_str()).is_some(),
        "'top_pattern' should be a string field in pattern_detection: {pd}"
    );
    assert!(
        pd.get("top_confidence").and_then(|v| v.as_f64()).is_some(),
        "'top_confidence' should be a number field in pattern_detection: {pd}"
    );
}

// ============================================================================
// DDD + Hexagonal detection
// Background fixture: pattern-ddd-project
// ============================================================================

/// Scenario: A well-structured DDD project is detected as DDD+Hexagonal
#[test]
fn pattern_ddd_hexagonal_is_top_pattern() {
    let json = analyze_json("pattern-ddd-project");
    let top = json["pattern_detection"]["top_pattern"]
        .as_str()
        .unwrap_or_else(|| panic!("'top_pattern' missing or not a string: {json}"));
    assert_eq!(
        top, "ddd-hexagonal",
        "expected top_pattern to be 'ddd-hexagonal', got '{top}'"
    );
}

/// Scenario: A well-structured DDD project scores at least 0.5 confidence for DDD+Hexagonal
#[test]
fn pattern_ddd_hexagonal_confidence_at_least_half() {
    let json = analyze_json("pattern-ddd-project");
    let conf = pattern_confidence(&json, "ddd-hexagonal");
    assert!(
        conf >= 0.5,
        "expected ddd-hexagonal confidence >= 0.5, got {conf}"
    );
}

// ============================================================================
// Gate: DDD scores shown when top confidence >= 0.5
// Background fixture: pattern-ddd-project
// ============================================================================

/// Scenario: Score dimensions are included when top pattern confidence is at least 0.5
#[test]
fn pattern_score_included_when_high_confidence() {
    let json = analyze_json("pattern-ddd-project");

    let score = json
        .get("score")
        .unwrap_or_else(|| panic!("'score' object missing when top confidence >= 0.5: {json}"));

    assert!(
        score.get("overall").and_then(|v| v.as_f64()).is_some(),
        "'score.overall' missing or not a number: {score}"
    );
    assert!(
        score
            .get("layer_conformance")
            .and_then(|v| v.as_f64())
            .is_some(),
        "'score.layer_conformance' missing or not a number: {score}"
    );
    assert!(
        score
            .get("dependency_compliance")
            .and_then(|v| v.as_f64())
            .is_some(),
        "'score.dependency_compliance' missing or not a number: {score}"
    );
}

// ============================================================================
// Text output
// Background fixture: pattern-ddd-project
// ============================================================================

/// Scenario: Text output shows the detected pattern name and its confidence
#[test]
fn pattern_text_output_shows_name_and_confidence() {
    let text = analyze_text("pattern-ddd-project");

    // The detected pattern name must appear in the output.
    assert!(
        text.contains("ddd-hexagonal") || text.to_lowercase().contains("hexagonal"),
        "text output should include the detected pattern name: {text}"
    );

    // The top confidence must appear as a decimal (e.g. "0.75") or percentage (e.g. "75%").
    let has_decimal = text.chars().any(|c| c == '.')
        && text
            .split_whitespace()
            .any(|w| w.trim_end_matches('%').parse::<f64>().is_ok());
    assert!(
        has_decimal,
        "text output should include the top confidence as a decimal or percentage: {text}"
    );
}

// ============================================================================
// Standalone scenarios — independent fixtures
// ============================================================================

// ---- Flat CRUD detection ----

/// Scenario: A flat project with one package and no abstract types is detected as Flat CRUD
///
/// Fixture: pattern-flat-crud/flat — 3 structs, 0 interfaces, single package
#[test]
fn pattern_flat_crud_confidence_at_least_half() {
    let json = analyze_json("pattern-flat-crud");
    let conf = pattern_confidence(&json, "flat-crud");
    assert!(
        conf >= 0.5,
        "expected flat-crud confidence >= 0.5 for single-package all-concrete project, got {conf}"
    );
}

// ---- Anemic Domain detection ----

/// Scenario: A project with a domain package containing only structs is detected as Anemic Domain
///
/// Fixture: pattern-anemic-domain
///   domain: 0 interfaces, 2 structs
///   services: 0 interfaces, 1 struct, imports domain
#[test]
fn pattern_anemic_domain_confidence_at_least_half() {
    let json = analyze_json("pattern-anemic-domain");
    let conf = pattern_confidence(&json, "anemic-domain");
    assert!(
        conf >= 0.5,
        "expected anemic-domain confidence >= 0.5 for all-concrete domain with logic in services, got {conf}"
    );
}

// ---- Gate: DDD scores omitted when top confidence < 0.5 ----

/// Scenario: Score dimensions are omitted when no pattern reaches the confidence threshold
///
/// Fixture: pattern-low-confidence
///   alpha: 0 interfaces, 2 structs — no layer names, no imports
///   beta:  0 interfaces, 2 structs — no layer names, no imports
#[test]
fn pattern_score_omitted_when_low_confidence() {
    let json = analyze_json("pattern-low-confidence");

    assert!(
        json.get("pattern_detection").is_some(),
        "'pattern_detection' object should still be present: {json}"
    );

    let top_conf = json["pattern_detection"]["top_confidence"]
        .as_f64()
        .unwrap_or_else(|| panic!("'top_confidence' missing or not a number: {json}"));
    assert!(
        top_conf < 0.5,
        "expected top_confidence < 0.5 for structurally neutral fixture, got {top_conf}"
    );

    assert!(
        json.get("score").is_none(),
        "'score' object should be absent when top confidence < 0.5: {json}"
    );
}

/// Scenario: Text output describes the low-confidence state when no pattern is dominant
///
/// Fixture: pattern-low-confidence (same as above)
#[test]
fn pattern_text_no_overall_score_when_low_confidence() {
    let text = analyze_text("pattern-low-confidence");

    // The overall architectural score section must NOT be shown.
    assert!(
        !text.contains("Overall Score"),
        "text output should not include 'Overall Score' when no pattern is dominant: {text}"
    );
}

// ---- Confidence value independence ----

/// Scenario: A project in transition scores above zero for more than one pattern
///
/// Fixture: pattern-transition
///   domain: 0 interfaces, 3 structs
///   infrastructure: 0 interfaces, 2 structs, imports domain
#[test]
fn pattern_transition_multiple_nonzero_confidences() {
    let json = analyze_json("pattern-transition");

    let patterns = json["pattern_detection"]["patterns"]
        .as_array()
        .unwrap_or_else(|| panic!("'pattern_detection.patterns' missing or not an array: {json}"));

    let nonzero_count = patterns
        .iter()
        .filter(|entry| entry["confidence"].as_f64().unwrap_or(0.0) > 0.0)
        .count();

    assert!(
        nonzero_count > 1,
        "expected more than one pattern with confidence > 0.0 for a transition project, got {nonzero_count}: {patterns:?}"
    );
}
