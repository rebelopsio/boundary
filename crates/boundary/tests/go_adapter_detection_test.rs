/// Acceptance tests for Go adapter/port detection (two-bug fix).
///
/// These tests cover:
///   Bug 1 — application-layer *Handler structs must NOT be counted as adapters.
///   Bug 2 — unexported structs (e.g. mongoUserRepository) must be included as real components.
///
/// Each test maps to a scenario in docs/features/go_adapter_detection.feature.
/// Run `cargo test --test go_adapter_detection_test` to check the current state.
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
        .unwrap_or_else(|e| panic!("failed to run boundary on {fixture_name}: {e}"));
    let stdout = String::from_utf8_lossy(&output.stdout);
    serde_json::from_str(stdout.trim())
        .unwrap_or_else(|e| panic!("invalid JSON from {fixture_name}: {e}\noutput: {stdout}"))
}

fn score_json(fixture_name: &str) -> serde_json::Value {
    let path = fixture(fixture_name);
    let output = boundary_cmd()
        .args(["analyze", &path, "--score-only", "--format", "json"])
        .output()
        .unwrap_or_else(|e| panic!("failed to run boundary on {fixture_name}: {e}"));
    let stdout = String::from_utf8_lossy(&output.stdout);
    serde_json::from_str(stdout.trim())
        .unwrap_or_else(|e| panic!("invalid JSON from {fixture_name}: {e}\noutput: {stdout}"))
}

/// Scenario 1: Application-layer *Handler struct is NOT counted as an adapter.
///
/// UserHandler lives in application/handler.go.  It should be classified as
/// an Entity or ValueObject — not an Adapter — so it does not appear under
/// the "adapter" key in components_by_kind.
#[test]
fn application_handler_not_counted_as_adapter() {
    let result = analyze_json("fr-go-adapters");
    // components_by_kind is nested under metrics
    let by_kind = &result["metrics"]["components_by_kind"];

    // If there is an "adapter" count, it should come only from WebhookHandler (1 adapter).
    // UserHandler must NOT inflate it.
    let adapter_count = by_kind["adapter"].as_u64().unwrap_or(0);
    assert!(
        adapter_count <= 1,
        "at most 1 adapter (WebhookHandler) expected, got {adapter_count}; \
        UserHandler in application layer must NOT be classified as adapter"
    );
}

/// Scenario 2: Infrastructure-layer *Handler (driving adapter) IS classified as Adapter kind.
///
/// WebhookHandler lives in infrastructure/webhook.go and ends with "Handler".
/// classify_struct_kind has no layer context, so it would otherwise classify
/// WebhookHandler as ValueObject. After layer assignment in the pipeline, the
/// post-processing reclassification step must upgrade infrastructure-layer
/// handler/controller structs to ComponentKind::Adapter.
#[test]
fn infrastructure_webhook_handler_is_classified_as_adapter() {
    let result = analyze_json("fr-go-adapters");
    let by_kind = &result["metrics"]["components_by_kind"];

    let adapter_count = by_kind["adapter"].as_u64().unwrap_or(0);
    assert!(
        adapter_count >= 1,
        "WebhookHandler in infrastructure layer must be classified as Adapter kind, got adapter_count={adapter_count}"
    );
}

/// Scenario 3: Unexported repository struct in infrastructure is a real component.
///
/// mongoUserRepository (lowercase first letter) lives in infrastructure/mongo_repo.go.
/// After the fix it must be extracted and classified as Repository.
#[test]
fn unexported_repository_struct_is_real_component() {
    let result = analyze_json("fr-go-adapters");
    let by_kind = &result["metrics"]["components_by_kind"];

    let repo_count = by_kind["repository"].as_u64().unwrap_or(0);
    assert!(
        repo_count >= 1,
        "mongoUserRepository (unexported) should appear as a Repository component, got {repo_count}"
    );
}

/// Scenario 4: interface_coverage reflects unexported adapters.
///
/// With mongoUserRepository counted as an infrastructure adapter and UserRepository
/// as a domain port, coverage must be > 0.
#[test]
fn interface_coverage_reflects_unexported_adapter() {
    let score = score_json("fr-go-adapters");

    let coverage = score["interface_coverage"].as_f64().unwrap_or(0.0);
    assert!(
        coverage > 0.0,
        "interface_coverage must be > 0 when unexported adapter exists; got {coverage}"
    );
}

/// Scenario 5: No MissingPort violation for a port/adapter pair where only
/// the adapter is unexported.
///
/// UserRepository (port) + mongoUserRepository (adapter) form a valid pair.
/// No MissingPort violation should be emitted for that pairing.
#[test]
fn no_missing_port_violation_for_unexported_adapter() {
    let result = analyze_json("fr-go-adapters");
    let violations = result["violations"].as_array().cloned().unwrap_or_default();

    let missing_port_for_mongo = violations.iter().any(|v| {
        let kind_obj = v["kind"].as_object();
        let is_missing_port = kind_obj
            .map(|o| o.contains_key("MissingPort"))
            .unwrap_or(false);
        let involves_mongo = v.to_string().contains("mongoUserRepository");
        is_missing_port && involves_mongo
    });

    assert!(
        !missing_port_for_mongo,
        "should not produce a MissingPort violation for mongoUserRepository; \
        it is paired with UserRepository port"
    );
}
