/// Acceptance tests for FR-18: Configurable Layer Classification Patterns.
///
/// Each test maps to a scenario in features/configurable_layer_patterns.feature.
/// Run `cargo test --test configurable_layer_patterns_test` to check the current state.
use std::process::Command;

fn boundary_cmd() -> Command {
    Command::new(env!("CARGO_BIN_EXE_boundary"))
}

fn fixture(name: &str) -> String {
    format!("{}/tests/fixtures/{name}", env!("CARGO_MANIFEST_DIR"))
}

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

// ============================================================================
// Fixture: fr18-custom-layers
//   .boundary.toml defines [[layers.overrides]] for services/auth/**:
//     domain         = ["services/auth/core/**"]
//     application    = ["services/auth/app/**"]
//     infrastructure = ["services/auth/server/**"]
//
//   Files:
//     services/auth/core/user.go   — User (entity) + UserRepository (port)
//     services/auth/app/handler.go — AuthService (application service)
//     services/auth/server/http.go — HTTPAdapter (infrastructure adapter)
// ============================================================================

/// @contract Scenario: Custom domain pattern classifies a file as domain
///
/// The services/auth/core/ directory is not covered by any default pattern
/// ("**/domain/**", etc.), but the override maps it to the domain layer.
#[test]
fn fr18_custom_domain_pattern_classifies_core_as_domain() {
    let json = analyze_json("fr18-custom-layers");

    let domain_count = json["metrics"]["components_by_layer"]["domain"]
        .as_u64()
        .unwrap_or_else(|| panic!("'components_by_layer.domain' missing or not a number: {json}"));

    assert!(
        domain_count > 0,
        "expected domain layer to have at least one component from services/auth/core/: {json}"
    );
}

/// @contract Scenario: Custom infrastructure pattern classifies a file as infrastructure
///
/// The services/auth/server/ directory maps to infrastructure via the override.
#[test]
fn fr18_custom_infrastructure_pattern_classifies_server_as_infrastructure() {
    let json = analyze_json("fr18-custom-layers");

    let infra_count = json["metrics"]["components_by_layer"]["infrastructure"]
        .as_u64()
        .unwrap_or_else(|| {
            panic!("'components_by_layer.infrastructure' missing or not a number: {json}")
        });

    assert!(
        infra_count > 0,
        "expected infrastructure layer to have at least one component from services/auth/server/: {json}"
    );
}

/// Scenario: Classification coverage is 100% when all paths are covered by override patterns
#[test]
fn fr18_all_components_classified_via_custom_patterns() {
    let json = analyze_json("fr18-custom-layers");

    let coverage = json["metrics"]["classification_coverage"]["coverage_percentage"]
        .as_f64()
        .unwrap_or_else(|| {
            panic!("'classification_coverage.coverage_percentage' missing or not a number: {json}")
        });

    assert!(
        coverage >= 100.0,
        "expected 100% classification coverage when all paths are covered by override patterns, got {coverage:.1}%: {json}"
    );
}

/// Scenario: Custom application pattern classifies app/ as application layer
#[test]
fn fr18_custom_application_pattern_classifies_app_as_application() {
    let json = analyze_json("fr18-custom-layers");

    let app_count = json["metrics"]["components_by_layer"]["application"]
        .as_u64()
        .unwrap_or_else(|| {
            panic!("'components_by_layer.application' missing or not a number: {json}")
        });

    assert!(
        app_count > 0,
        "expected application layer to have at least one component from services/auth/app/: {json}"
    );
}

/// Scenario: No layer boundary violations when architecture is clean
///
/// server/ (infrastructure) imports core/ (domain) — valid direction.
/// app/ (application) imports core/ (domain) — valid direction.
/// No reverse dependencies.
#[test]
fn fr18_clean_architecture_has_no_layer_boundary_violations() {
    let json = analyze_json("fr18-custom-layers");

    let violations = json["violations"]
        .as_array()
        .unwrap_or_else(|| panic!("'violations' missing or not an array: {json}"));

    let layer_violations: Vec<_> = violations
        .iter()
        .filter(|v| v["kind"].get("LayerBoundary").is_some())
        .collect();

    assert!(
        layer_violations.is_empty(),
        "expected no layer boundary violations for clean architecture with custom patterns, got: {layer_violations:?}"
    );
}
