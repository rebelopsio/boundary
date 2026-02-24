/// Acceptance tests for FR-21: Init Function Dependency Detection.
///
/// Each test maps to a scenario in features/init_function_coupling.feature.
/// Run `cargo test --test init_function_coupling_test` to check the current state.
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

fn analyze_text(fixture_name: &str) -> String {
    let path = fixture(fixture_name);
    let output = boundary_cmd()
        .args(["analyze", &path])
        .output()
        .unwrap_or_else(|e| panic!("failed to run boundary on {fixture_name}: {e}"));
    String::from_utf8_lossy(&output.stdout).into_owned()
}

/// FR-21 @contract: init() cross-layer call produces an InitFunctionCoupling violation.
#[test]
fn init_cross_layer_call_produces_init_function_coupling_violation() {
    let result = analyze_json("fr21-init-coupling");
    let empty = vec![];
    let kinds: Vec<String> = result["violations"]
        .as_array()
        .unwrap_or(&empty)
        .iter()
        .filter_map(|v| v["kind"].as_object())
        .flat_map(|o| o.keys().cloned())
        .collect();
    assert!(
        kinds.contains(&"InitFunctionCoupling".to_string()),
        "should report InitFunctionCoupling violation; got: {kinds:?}"
    );
}

/// FR-21 @contract: InitFunctionCoupling violation has warning severity by default.
#[test]
fn init_function_coupling_has_warning_severity() {
    let result = analyze_json("fr21-init-coupling");
    let empty = vec![];
    let init_violation = result["violations"]
        .as_array()
        .unwrap_or(&empty)
        .iter()
        .find(|v| {
            v["kind"]
                .as_object()
                .is_some_and(|o| o.contains_key("InitFunctionCoupling"))
        })
        .expect("should have an InitFunctionCoupling violation");
    let severity = init_violation["severity"].as_str().unwrap_or("");
    assert_eq!(
        severity, "warning",
        "InitFunctionCoupling should have 'warning' severity; got: {severity}"
    );
}

/// FR-21: detect_init_functions = false suppresses init violations.
#[test]
fn detect_init_functions_false_suppresses_init_violations() {
    let result = analyze_json("fr21-init-coupling-disabled");
    let empty = vec![];
    let kinds: Vec<String> = result["violations"]
        .as_array()
        .unwrap_or(&empty)
        .iter()
        .filter_map(|v| v["kind"].as_object())
        .flat_map(|o| o.keys().cloned())
        .collect();
    assert!(
        !kinds.contains(&"InitFunctionCoupling".to_string()),
        "detect_init_functions=false should suppress InitFunctionCoupling; got: {kinds:?}"
    );
}

/// FR-21: Text output describes the init coupling.
#[test]
fn init_function_coupling_text_output_mentions_init() {
    let output = analyze_text("fr21-init-coupling");
    assert!(
        output.to_lowercase().contains("init"),
        "text output should mention 'init'; got:\n{output}"
    );
}
