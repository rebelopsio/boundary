/// Smoke tests against real-world DDD+Hexagonal repositories.
///
/// These tests clone public GitHub repositories and run `boundary analyze`
/// against them to catch regressions that synthetic fixtures cannot surface.
/// They are ignored by default (`cargo test --all` skips them) and run
/// explicitly via the CI `smoke` job or locally with:
///
///   cargo test --test smoke_test -- --include-ignored
///
/// A test is allowed to be skipped (not failed) when the clone fails due to
/// network unavailability, so external flakiness never blocks local work.
use std::path::Path;
use std::process::Command;
use tempfile::TempDir;

fn boundary_cmd() -> Command {
    Command::new(env!("CARGO_BIN_EXE_boundary"))
}

/// Clone a public GitHub repo at shallow depth into a temp directory.
/// Returns `None` if the network is unavailable or the clone fails.
fn shallow_clone(url: &str) -> Option<TempDir> {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    let status = Command::new("git")
        .args(["clone", "--depth", "1", "--quiet", url, "."])
        .current_dir(dir.path())
        .status();

    match status {
        Ok(s) if s.success() => Some(dir),
        Ok(s) => {
            println!("git clone exited with {s} — skipping smoke test");
            None
        }
        Err(e) => {
            println!("git clone failed ({e}) — skipping smoke test");
            None
        }
    }
}

fn analyze_json(path: &Path) -> serde_json::Value {
    let output = boundary_cmd()
        .args(["analyze", &path.to_string_lossy(), "--format", "json"])
        .output()
        .expect("failed to run boundary");
    let stdout = String::from_utf8_lossy(&output.stdout);
    serde_json::from_str(stdout.trim())
        .unwrap_or_else(|e| panic!("invalid JSON from boundary: {e}\noutput: {stdout}"))
}

// ---------------------------------------------------------------------------
// RanchoCooper/go-hexagonal
// Full DDD + Hexagonal microservice with MySQL, PostgreSQL, Redis adapters.
// Default boundary layer patterns classify this repo without any .boundary.toml:
//   domain/**     → Domain        (ports in domain/repo/, domain/service/)
//   application/**→ Application   (use cases in application/example/)
//   adapter/**    → Infrastructure (adapters in adapter/repository/*)
//   api/**        → Presentation  (HTTP/gRPC handlers in api/)
//   cmd/**        → Presentation
// ---------------------------------------------------------------------------

/// Infrastructure layer must contain real adapter components (MySQL, PostgreSQL,
/// Redis). A count of zero indicates classification or extraction is broken.
#[test]
#[ignore = "requires network"]
fn go_hexagonal_infrastructure_has_adapters() {
    let Some(dir) = shallow_clone("https://github.com/RanchoCooper/go-hexagonal") else {
        return;
    };
    let result = analyze_json(dir.path());
    let by_layer = &result["metrics"]["components_by_layer"];

    let infra = by_layer["infrastructure"].as_u64().unwrap_or(0);
    assert!(
        infra >= 5,
        "expected >= 5 infrastructure components (MySQL/PostgreSQL/Redis adapters), got {infra}"
    );
}

/// Domain layer must define port interfaces (IExampleRepo, IExampleCacheRepo,
/// Transaction, IExampleService). Zero ports means interface extraction is broken.
#[test]
#[ignore = "requires network"]
fn go_hexagonal_domain_has_ports() {
    let Some(dir) = shallow_clone("https://github.com/RanchoCooper/go-hexagonal") else {
        return;
    };
    let result = analyze_json(dir.path());
    let by_kind = &result["metrics"]["components_by_kind"];

    let ports = by_kind["port"].as_u64().unwrap_or(0);
    assert!(
        ports >= 2,
        "expected >= 2 port interfaces in domain layer (IExampleRepo, IExampleCacheRepo at minimum), got {ports}"
    );
}

/// Infrastructure layer must contain repository implementations (MySQL ExampleRepo,
/// PostgreSQL ExampleRepo). Zero repositories means struct extraction or layer
/// classification is broken.
#[test]
#[ignore = "requires network"]
fn go_hexagonal_infrastructure_has_repositories() {
    let Some(dir) = shallow_clone("https://github.com/RanchoCooper/go-hexagonal") else {
        return;
    };
    let result = analyze_json(dir.path());
    let by_kind = &result["metrics"]["components_by_kind"];

    let repos = by_kind["repository"].as_u64().unwrap_or(0);
    assert!(
        repos >= 2,
        "expected >= 2 repository adapters (mysql.ExampleRepo, postgre.ExampleRepo), got {repos}"
    );
}

/// Interface coverage must be > 0 when both ports and adapters exist.
/// Coverage = 0 with known ports and repositories indicates a scoring bug.
#[test]
#[ignore = "requires network"]
fn go_hexagonal_interface_coverage_nonzero() {
    let Some(dir) = shallow_clone("https://github.com/RanchoCooper/go-hexagonal") else {
        return;
    };
    let result = analyze_json(dir.path());

    // Score may be None if pattern confidence < 0.5; treat that as a failure.
    let score = result["score"]
        .as_object()
        .expect("score should be present");
    let coverage = score
        .get("interface_coverage")
        .and_then(|v| v.as_f64())
        .unwrap_or(0.0);
    assert!(
        coverage > 0.0,
        "interface_coverage must be > 0 when ports and repositories exist; got {coverage}"
    );
}

/// DDD + Hexagonal pattern must be detected with >= 50% confidence.
/// Lower confidence means the structural signals boundary relies on are broken.
#[test]
#[ignore = "requires network"]
fn go_hexagonal_pattern_detected() {
    let Some(dir) = shallow_clone("https://github.com/RanchoCooper/go-hexagonal") else {
        return;
    };
    let result = analyze_json(dir.path());

    let top_confidence = result["pattern_detection"]["top_confidence"]
        .as_f64()
        .unwrap_or(0.0);
    let top_pattern = result["pattern_detection"]["top_pattern"]
        .as_str()
        .unwrap_or("unknown");
    assert!(
        top_confidence >= 0.5,
        "expected ddd-hexagonal pattern with >= 50% confidence, got '{top_pattern}' at {top_confidence:.2}"
    );
}

/// Domain layer must not import from infrastructure. Any violation of this rule
/// is a boundary-violation detection regression.
#[test]
#[ignore = "requires network"]
fn go_hexagonal_no_domain_to_infra_violations() {
    let Some(dir) = shallow_clone("https://github.com/RanchoCooper/go-hexagonal") else {
        return;
    };
    let result = analyze_json(dir.path());

    let violations = result["violations"].as_array().cloned().unwrap_or_default();
    let domain_to_infra: Vec<_> = violations
        .iter()
        .filter(|v| {
            let kind = v["kind"].as_object();
            kind.map(|k| k.contains_key("LayerBoundary"))
                .unwrap_or(false)
                && v.to_string().contains("domain")
                && v.to_string().contains("infrastructure")
        })
        .collect();

    assert!(
        domain_to_infra.is_empty(),
        "domain layer must not import from infrastructure; found {} violation(s): {:?}",
        domain_to_infra.len(),
        domain_to_infra
    );
}
