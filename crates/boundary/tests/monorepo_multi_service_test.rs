/// Acceptance tests for FR-24: Monorepo / Multi-Service Support.
///
/// Each test maps to a scenario in features/monorepo_multi_service.feature.
/// Run `cargo test --test monorepo_multi_service_test` to check the current state.
use std::process::Command;

fn boundary_cmd() -> Command {
    Command::new(env!("CARGO_BIN_EXE_boundary"))
}

fn fixture(name: &str) -> String {
    format!("{}/tests/fixtures/{name}", env!("CARGO_MANIFEST_DIR"))
}

fn analyze_per_service_json(fixture_name: &str) -> serde_json::Value {
    let path = fixture(fixture_name);
    let output = boundary_cmd()
        .args(["analyze", &path, "--per-service", "--format", "json"])
        .output()
        .unwrap_or_else(|e| panic!("failed to run boundary on {fixture_name}: {e}"));
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "boundary failed on {fixture_name}: stderr={stderr}"
    );
    serde_json::from_str(stdout.trim())
        .unwrap_or_else(|e| panic!("invalid JSON from {fixture_name}: {e}\noutput: {stdout}"))
}

/// FR-24 @contract: Per-service JSON output contains a "services" array with 2 entries.
#[test]
fn per_service_output_has_services_array_with_two_entries() {
    let result = analyze_per_service_json("fr24-monorepo");
    let services = result["services"]
        .as_array()
        .expect("output should have a top-level 'services' array");
    assert_eq!(
        services.len(),
        2,
        "monorepo with 2 services should produce 2 service entries; got: {}",
        services.len()
    );
}

/// FR-24 @contract: Each service entry has a service_name and a result object.
#[test]
fn each_service_entry_has_service_name_and_result() {
    let result = analyze_per_service_json("fr24-monorepo");
    let services = result["services"]
        .as_array()
        .expect("output should have a 'services' array");
    for svc in services {
        assert!(
            svc["service_name"].is_string(),
            "each service entry should have a 'service_name' string; got: {svc}"
        );
        assert!(
            svc["result"].is_object(),
            "each service entry should have a 'result' object; got: {svc}"
        );
    }
}

/// FR-24: The two services are "auth" and "order".
#[test]
fn per_service_discovers_auth_and_order_services() {
    let result = analyze_per_service_json("fr24-monorepo");
    let services = result["services"]
        .as_array()
        .expect("output should have a 'services' array");
    let names: Vec<&str> = services
        .iter()
        .filter_map(|s| s["service_name"].as_str())
        .collect();
    assert!(
        names.contains(&"auth"),
        "services should include 'auth'; got: {names:?}"
    );
    assert!(
        names.contains(&"order"),
        "services should include 'order'; got: {names:?}"
    );
}

/// FR-24: Each service result contains a score object.
#[test]
fn per_service_each_result_has_score() {
    let result = analyze_per_service_json("fr24-monorepo");
    let services = result["services"]
        .as_array()
        .expect("output should have a 'services' array");
    for svc in services {
        let name = svc["service_name"].as_str().unwrap_or("unknown");
        let score = &svc["result"]["score"];
        assert!(
            score.is_object(),
            "service '{name}' result should have a 'score' object; got: {score}"
        );
    }
}
