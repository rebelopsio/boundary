//! Pattern detection engine for FR-27.
//!
//! Computes independent confidence scores in [0.0, 1.0] for five architectural
//! patterns. Confidence values do NOT sum to 1.0 — a codebase in transition
//! may score above zero for multiple patterns simultaneously.
//!
//! Patterns:
//!   ddd-hexagonal  — distinct layers, domain has ports, correct coupling direction
//!   active-record  — domain types carry persistence (insufficient signals without method analysis)
//!   flat-crud      — single package, all concrete, no layer convention
//!   anemic-domain  — domain package has no interfaces, business logic lives elsewhere
//!   service-layer  — some separation, cross-package deps, but no ports/adapters

use std::collections::HashSet;

use serde::{Deserialize, Serialize};

use crate::types::{Component, ComponentKind, Dependency};

/// A single pattern with its confidence score.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PatternScore {
    pub name: String,
    /// Independent confidence in [0.0, 1.0]. Values do not sum to 1.0.
    pub confidence: f64,
}

/// Output of the pattern detection pass.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PatternDetection {
    /// All five patterns with their confidence scores.
    pub patterns: Vec<PatternScore>,
    /// Name of the pattern with the highest confidence.
    pub top_pattern: String,
    /// Confidence of the top pattern.
    pub top_confidence: f64,
}

/// Internal signals derived from components and dependencies.
struct Signals {
    pkg_count: usize,
    has_domain_layer: bool,
    has_app_layer: bool,
    has_infra_layer: bool,
    layer_name_count: usize,
    total_interfaces: usize,
    domain_interfaces: usize,
    domain_structs: usize,
    domain_is_imported: bool,
    domain_imports_nothing: bool,
    has_any_internal_deps: bool,
}

/// Detect architectural patterns and return confidence scores for all five patterns.
pub fn detect_patterns(components: &[Component], dependencies: &[Dependency]) -> PatternDetection {
    let signals = extract_signals(components, dependencies);

    let mut patterns = vec![
        PatternScore {
            name: "ddd-hexagonal".to_string(),
            confidence: ddd_hexagonal(&signals),
        },
        PatternScore {
            name: "active-record".to_string(),
            confidence: active_record(&signals),
        },
        PatternScore {
            name: "flat-crud".to_string(),
            confidence: flat_crud(&signals),
        },
        PatternScore {
            name: "anemic-domain".to_string(),
            confidence: anemic_domain(&signals),
        },
        PatternScore {
            name: "service-layer".to_string(),
            confidence: service_layer(&signals),
        },
    ];

    // Sort descending by confidence so the first entry is the top pattern.
    // Stable sort preserves the fixed declaration order for ties.
    patterns.sort_by(|a, b| {
        b.confidence
            .partial_cmp(&a.confidence)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    let top_pattern = patterns
        .first()
        .map(|p| p.name.clone())
        .unwrap_or_else(|| "unknown".to_string());
    let top_confidence = patterns.first().map(|p| p.confidence).unwrap_or(0.0);

    // Re-sort alphabetically so the JSON output is stable and predictable.
    patterns.sort_by(|a, b| a.name.cmp(&b.name));

    PatternDetection {
        patterns,
        top_pattern,
        top_confidence,
    }
}

// ─── Signal extraction ────────────────────────────────────────────────────────

fn pkg_from_id(id: &str) -> &str {
    id.split("::").next().unwrap_or("")
}

/// Check whether any path segment (split on `/`, `.`, or `:`) equals `layer`.
/// Handles Go (`project/domain`), Java (`com.example.domain.user`),
/// and Rust (`crate::domain::user`) package naming conventions.
fn path_contains_layer(path: &str, layer: &str) -> bool {
    path.split(['/', '.', ':']).any(|seg| seg == layer)
}

/// Return the canonical layer name found anywhere in `path`, or `None`.
fn import_layer(path: &str) -> Option<&'static str> {
    // Order matters: check more-specific names before aliases.
    ["domain", "application", "infrastructure", "app", "infra"]
        .iter()
        .find(|&&layer| path_contains_layer(path, layer))
        .copied()
}

/// Normalise layer aliases: "app" → "application", "infra" → "infrastructure".
fn canonical_layer(layer: &str) -> &str {
    match layer {
        "app" => "application",
        "infra" => "infrastructure",
        other => other,
    }
}

fn extract_signals(components: &[Component], dependencies: &[Dependency]) -> Signals {
    // ── Package set ─────────────────────────────────────────────────────────
    let mut pkg_paths: HashSet<String> = HashSet::new();
    for comp in components {
        let pkg = pkg_from_id(&comp.id.0);
        if !pkg.is_empty() {
            pkg_paths.insert(pkg.to_string());
        }
    }
    let pkg_count = pkg_paths.len();

    // ── Layer names ──────────────────────────────────────────────────────────
    // Check whether any layer keyword appears as a segment anywhere in the pkg path.
    // This handles sub-packages (Java `com.example.domain.user`) and
    // sub-directories (Rust `src/domain/user`).
    let has_domain_layer = pkg_paths.iter().any(|p| path_contains_layer(p, "domain"));
    let has_app_layer = pkg_paths
        .iter()
        .any(|p| path_contains_layer(p, "application") || path_contains_layer(p, "app"));
    let has_infra_layer = pkg_paths
        .iter()
        .any(|p| path_contains_layer(p, "infrastructure") || path_contains_layer(p, "infra"));
    let layer_name_count = [has_domain_layer, has_app_layer, has_infra_layer]
        .iter()
        .filter(|&&x| x)
        .count();

    // ── Interface / struct counts ────────────────────────────────────────────
    let mut total_interfaces = 0usize;
    let mut domain_interfaces = 0usize;
    let mut domain_structs = 0usize;

    for comp in components {
        let full_pkg = pkg_from_id(&comp.id.0);
        let in_domain = path_contains_layer(full_pkg, "domain");
        let is_interface = matches!(comp.kind, ComponentKind::Port(_));
        if is_interface {
            total_interfaces += 1;
        }
        if in_domain {
            if is_interface {
                domain_interfaces += 1;
            } else {
                domain_structs += 1;
            }
        }
    }

    // ── Internal coupling ─────────────────────────────────────────────────────
    // Detect cross-layer deps by matching layer keywords in the from-package
    // path and the import_path string.
    let mut has_any_internal_deps = false;
    let mut domain_has_afferent = false; // something outside domain imports domain
    let mut domain_has_efferent = false; // domain imports something outside itself

    for dep in dependencies {
        let from_pkg = pkg_from_id(&dep.from.0);
        let Some(from_layer) = import_layer(from_pkg) else {
            continue;
        };
        let Some(to_layer) = dep.import_path.as_deref().and_then(import_layer) else {
            continue;
        };

        let from_c = canonical_layer(from_layer);
        let to_c = canonical_layer(to_layer);
        if from_c == to_c {
            continue; // intra-layer
        }

        has_any_internal_deps = true;
        if to_c == "domain" {
            domain_has_afferent = true;
        }
        if from_c == "domain" {
            domain_has_efferent = true;
        }
    }

    let domain_is_imported = domain_has_afferent;
    let domain_imports_nothing = !domain_has_efferent;

    Signals {
        pkg_count,
        has_domain_layer,
        has_app_layer,
        has_infra_layer,
        layer_name_count,
        total_interfaces,
        domain_interfaces,
        domain_structs,
        domain_is_imported,
        domain_imports_nothing,
        has_any_internal_deps,
    }
}

// ─── Pattern confidence functions ─────────────────────────────────────────────

/// DDD + Hexagonal: distinct layers, domain has ports, stable core.
fn ddd_hexagonal(s: &Signals) -> f64 {
    let mut score = 0.0_f64;
    if s.has_domain_layer {
        score += 0.20;
    }
    if s.has_app_layer {
        score += 0.15;
    }
    if s.has_infra_layer {
        score += 0.15;
    }
    if s.domain_interfaces > 0 {
        score += 0.20;
    }
    if s.domain_is_imported && s.domain_imports_nothing {
        score += 0.20;
    }
    let total_domain = s.domain_interfaces + s.domain_structs;
    if total_domain > 0 {
        let ratio = s.domain_interfaces as f64 / total_domain as f64;
        if ratio >= 0.25 {
            score += 0.10;
        }
    }
    score.clamp(0.0, 1.0)
}

/// Active Record: domain types carry persistence — requires method-level signals
/// not available from structural analysis alone.
fn active_record(_s: &Signals) -> f64 {
    0.0
}

/// Flat CRUD: single package (or nearly so), all concrete, no layer convention.
fn flat_crud(s: &Signals) -> f64 {
    let mut score = 0.0_f64;
    if s.pkg_count == 1 {
        score += 0.55;
    } else if s.pkg_count == 2 && !s.has_any_internal_deps {
        // Two isolated packages with no coupling — weak flat signal
        score += 0.05;
    }
    if s.total_interfaces == 0 {
        score += 0.20;
    }
    if s.layer_name_count == 0 {
        score += 0.15;
    }
    score.clamp(0.0, 1.0)
}

/// Anemic Domain: domain package is a data container (no interfaces), business
/// logic lives in a separate service/application package.
fn anemic_domain(s: &Signals) -> f64 {
    if !s.has_domain_layer {
        return 0.0;
    }
    let mut score = 0.0_f64;
    if s.domain_interfaces == 0 && s.domain_structs > 0 {
        score += 0.40; // key signal: domain has no abstract types
    }
    if s.domain_is_imported {
        score += 0.30; // other packages depend on domain as a data layer
    }
    if s.total_interfaces == 0 {
        score += 0.20; // no ports anywhere
    }
    if !s.has_app_layer {
        score += 0.10; // logic layer is not named "application"
    }
    score.clamp(0.0, 1.0)
}

/// Service Layer: some separation and cross-package coupling, but no ports/adapters.
fn service_layer(s: &Signals) -> f64 {
    let mut score = 0.0_f64;
    if s.pkg_count >= 2 {
        score += 0.20;
    }
    if s.has_any_internal_deps {
        score += 0.30;
    }
    if s.total_interfaces == 0 && s.has_any_internal_deps {
        score += 0.20; // services call data types directly — no abstraction layer
    }
    if !s.has_domain_layer && !s.has_infra_layer && s.pkg_count >= 2 {
        score += 0.10; // no DDD naming convention
    }
    score.clamp(0.0, 1.0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{
        ArchLayer, ArchitectureMode, ComponentId, ComponentKind, DependencyKind, EntityInfo,
        PortInfo, SourceLocation,
    };
    use std::path::PathBuf;

    fn make_interface(id: &str) -> Component {
        Component {
            id: ComponentId(id.to_string()),
            name: id.split("::").last().unwrap_or(id).to_string(),
            kind: ComponentKind::Port(PortInfo {
                name: id.to_string(),
                methods: vec![],
            }),
            layer: None,
            location: SourceLocation {
                file: PathBuf::from("test.go"),
                line: 1,
                column: 1,
            },
            is_cross_cutting: false,
            architecture_mode: ArchitectureMode::Ddd,
        }
    }

    fn make_struct(id: &str) -> Component {
        Component {
            id: ComponentId(id.to_string()),
            name: id.split("::").last().unwrap_or(id).to_string(),
            kind: ComponentKind::Entity(EntityInfo {
                name: id.to_string(),
                fields: vec![],
                methods: vec![],
                is_active_record: false,
                is_anemic_domain_model: false,
            }),
            layer: Some(ArchLayer::Domain),
            location: SourceLocation {
                file: PathBuf::from("test.go"),
                line: 1,
                column: 1,
            },
            is_cross_cutting: false,
            architecture_mode: ArchitectureMode::Ddd,
        }
    }

    fn make_dep(from: &str, to: &str, import_path: &str) -> Dependency {
        Dependency {
            from: ComponentId(from.to_string()),
            to: ComponentId(to.to_string()),
            kind: DependencyKind::Import,
            location: SourceLocation {
                file: PathBuf::from("test.go"),
                line: 1,
                column: 1,
            },
            import_path: Some(import_path.to_string()),
        }
    }

    #[test]
    fn ddd_hexagonal_confidence_high_for_layered_project_with_ports() {
        // domain: 1 interface + 1 struct; application + infrastructure import domain
        let components = vec![
            make_interface("/project/domain::UserRepository"),
            make_struct("/project/domain::User"),
            make_struct("/project/application::UserService"),
            make_struct("/project/infrastructure::UserRepo"),
        ];
        let deps = vec![
            make_dep(
                "/project/application::UserService",
                "/project/domain::UserRepository",
                "example/domain",
            ),
            make_dep(
                "/project/infrastructure::UserRepo",
                "/project/domain::UserRepository",
                "example/domain",
            ),
        ];
        let pd = detect_patterns(&components, &deps);
        let conf = pd
            .patterns
            .iter()
            .find(|p| p.name == "ddd-hexagonal")
            .map(|p| p.confidence)
            .unwrap();
        assert!(
            conf >= 0.5,
            "ddd-hexagonal should be >= 0.5 for a layered project, got {conf}"
        );
        assert_eq!(pd.top_pattern, "ddd-hexagonal");
    }

    #[test]
    fn flat_crud_confidence_high_for_single_package() {
        let components = vec![
            make_struct("/project/flat::Product"),
            make_struct("/project/flat::Order"),
            make_struct("/project/flat::Customer"),
        ];
        let pd = detect_patterns(&components, &[]);
        let conf = pd
            .patterns
            .iter()
            .find(|p| p.name == "flat-crud")
            .map(|p| p.confidence)
            .unwrap();
        assert!(
            conf >= 0.5,
            "flat-crud should be >= 0.5 for a single-package all-concrete project, got {conf}"
        );
    }

    #[test]
    fn anemic_domain_confidence_high_when_domain_has_no_interfaces() {
        // domain: 2 structs; services: 1 struct; services imports domain
        let components = vec![
            make_struct("/project/domain::Order"),
            make_struct("/project/domain::Customer"),
            make_struct("/project/services::OrderService"),
        ];
        let deps = vec![make_dep(
            "/project/services::OrderService",
            "/project/domain::Order",
            "example/domain",
        )];
        let pd = detect_patterns(&components, &deps);
        let conf = pd
            .patterns
            .iter()
            .find(|p| p.name == "anemic-domain")
            .map(|p| p.confidence)
            .unwrap();
        assert!(
            conf >= 0.5,
            "anemic-domain should be >= 0.5 for a domain with no interfaces, got {conf}"
        );
    }

    #[test]
    fn all_confidences_below_threshold_for_structurally_neutral_project() {
        // alpha/beta: no layer names, no imports, no interfaces
        let components = vec![
            make_struct("/project/alpha::Foo"),
            make_struct("/project/alpha::Bar"),
            make_struct("/project/beta::Qux"),
            make_struct("/project/beta::Baz"),
        ];
        let pd = detect_patterns(&components, &[]);
        let max_conf = pd
            .patterns
            .iter()
            .map(|p| p.confidence)
            .fold(0.0_f64, f64::max);
        assert!(
            max_conf < 0.5,
            "all confidences should be < 0.5 for a structurally neutral project, got max {max_conf}"
        );
    }

    #[test]
    fn transition_project_has_multiple_nonzero_patterns() {
        // domain: 3 structs (no interfaces); infrastructure: 2 structs, imports domain
        let components = vec![
            make_struct("/project/domain::Order"),
            make_struct("/project/domain::Customer"),
            make_struct("/project/domain::Product"),
            make_struct("/project/infrastructure::OrderRepo"),
            make_struct("/project/infrastructure::CustomerRepo"),
        ];
        let deps = vec![
            make_dep(
                "/project/infrastructure::OrderRepo",
                "/project/domain::Order",
                "example/domain",
            ),
            make_dep(
                "/project/infrastructure::CustomerRepo",
                "/project/domain::Customer",
                "example/domain",
            ),
        ];
        let pd = detect_patterns(&components, &deps);
        let nonzero = pd.patterns.iter().filter(|p| p.confidence > 0.0).count();
        assert!(
            nonzero > 1,
            "a transition project should have more than one pattern above 0.0, got {nonzero}"
        );
    }

    #[test]
    fn output_always_contains_all_five_patterns() {
        let pd = detect_patterns(&[], &[]);
        let names: Vec<&str> = pd.patterns.iter().map(|p| p.name.as_str()).collect();
        for expected in [
            "ddd-hexagonal",
            "active-record",
            "flat-crud",
            "anemic-domain",
            "service-layer",
        ] {
            assert!(names.contains(&expected), "missing pattern '{expected}'");
        }
    }

    #[test]
    fn all_confidence_values_in_range() {
        let pd = detect_patterns(&[], &[]);
        for p in &pd.patterns {
            assert!(
                (0.0..=1.0).contains(&p.confidence),
                "confidence for '{}' out of range: {}",
                p.name,
                p.confidence
            );
        }
    }
}
