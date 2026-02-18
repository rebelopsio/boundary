use std::path::{Path, PathBuf};

use walkdir::WalkDir;

use crate::metrics::ArchitectureScore;
use crate::pipeline::FullAnalysis;
use crate::types::*;

/// Full forensics analysis for a module.
pub struct ForensicsAnalysis {
    pub module_name: String,
    pub module_path: PathBuf,
    pub directory_tree: Vec<DirEntry>,
    pub aggregates: Vec<AggregateAnalysis>,
    pub domain_events: Vec<Component>,
    pub ports: Vec<Component>,
    pub application_services: Vec<Component>,
    pub infrastructure_adapters: Vec<AdapterMapping>,
    pub violations: Vec<Violation>,
    pub score: ArchitectureScore,
    pub classified_imports: Vec<ClassifiedImport>,
    pub improvements: Vec<String>,
}

/// An entry in the directory tree.
pub struct DirEntry {
    pub rel_path: String,
    pub is_dir: bool,
    pub depth: usize,
}

/// Analysis of a single aggregate (entity).
pub struct AggregateAnalysis {
    pub component: Component,
    pub value_objects: Vec<Component>,
    pub dependency_audit: DependencyAudit,
    pub ddd_patterns: Vec<DddPattern>,
}

/// A DDD pattern detection result.
pub struct DddPattern {
    pub name: String,
    pub detected: bool,
}

/// Audit of an aggregate's dependencies.
pub struct DependencyAudit {
    pub stdlib_imports: Vec<String>,
    pub internal_domain_imports: Vec<String>,
    pub external_imports: Vec<String>,
    pub infrastructure_leaks: Vec<String>,
    pub is_clean: bool,
}

/// Mapping from an adapter to the ports it implements.
pub struct AdapterMapping {
    pub adapter: Component,
    pub implements_ports: Vec<String>,
}

/// A classified import.
pub struct ClassifiedImport {
    pub import_path: String,
    pub category: ImportCategory,
    pub source_file: PathBuf,
}

/// Category of an import path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ImportCategory {
    Stdlib,
    InternalDomain,
    InternalApplication,
    InternalInfrastructure,
    External,
}

/// Build a forensics analysis from a full analysis and module path.
pub fn build_forensics(
    full_analysis: &FullAnalysis,
    module_path: &Path,
    _project_root: &Path,
) -> ForensicsAnalysis {
    let module_name = module_path
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| "unknown".to_string());

    // Build directory tree
    let directory_tree = build_directory_tree(module_path);

    // Classify imports
    let classified_imports = classify_all_imports(&full_analysis.dependencies);

    // Group components by kind
    let mut domain_events = Vec::new();
    let mut ports = Vec::new();
    let mut entities = Vec::new();
    let mut value_objects = Vec::new();
    let mut application_services = Vec::new();
    let mut infrastructure_adapters = Vec::new();

    for comp in &full_analysis.components {
        match &comp.kind {
            ComponentKind::DomainEvent(_) => domain_events.push(comp.clone()),
            ComponentKind::Port(_) => ports.push(comp.clone()),
            ComponentKind::Entity(_) => entities.push(comp.clone()),
            ComponentKind::ValueObject => value_objects.push(comp.clone()),
            ComponentKind::Service if comp.layer == Some(ArchLayer::Application) => {
                application_services.push(comp.clone());
            }
            ComponentKind::UseCase => application_services.push(comp.clone()),
            ComponentKind::Adapter(info) => {
                infrastructure_adapters.push(AdapterMapping {
                    adapter: comp.clone(),
                    implements_ports: info.implements.clone(),
                });
            }
            ComponentKind::Repository if comp.layer == Some(ArchLayer::Infrastructure) => {
                infrastructure_adapters.push(AdapterMapping {
                    adapter: comp.clone(),
                    implements_ports: Vec::new(),
                });
            }
            _ => {}
        }
    }

    // Build aggregate analyses
    let aggregates = build_aggregates(&entities, &value_objects, &classified_imports);

    // Generate improvement suggestions
    let improvements = generate_improvements(
        &entities,
        &domain_events,
        &infrastructure_adapters,
        &ports,
        &full_analysis.result.violations,
    );

    ForensicsAnalysis {
        module_name,
        module_path: module_path.to_path_buf(),
        directory_tree,
        aggregates,
        domain_events,
        ports,
        application_services,
        infrastructure_adapters,
        violations: full_analysis.result.violations.clone(),
        score: full_analysis.result.score.clone(),
        classified_imports,
        improvements,
    }
}

fn build_directory_tree(module_path: &Path) -> Vec<DirEntry> {
    let mut entries = Vec::new();

    for entry in WalkDir::new(module_path)
        .sort_by_file_name()
        .into_iter()
        .filter_map(|e| e.ok())
    {
        let rel_path = entry
            .path()
            .strip_prefix(module_path)
            .unwrap_or(entry.path())
            .to_string_lossy()
            .to_string();

        if rel_path.is_empty() {
            continue;
        }

        entries.push(DirEntry {
            rel_path,
            is_dir: entry.path().is_dir(),
            depth: entry.depth(),
        });
    }

    entries
}

fn classify_import(import_path: &str) -> ImportCategory {
    // Go: no dots in path segment -> stdlib
    if !import_path.contains('.') && !import_path.starts_with("./") {
        return ImportCategory::Stdlib;
    }

    // Rust stdlib
    if import_path.starts_with("std::") || import_path.starts_with("core::") {
        return ImportCategory::Stdlib;
    }

    // Java stdlib
    if import_path.starts_with("java.") || import_path.starts_with("javax.") {
        return ImportCategory::Stdlib;
    }

    // Internal detection by path patterns
    let lower = import_path.to_lowercase();
    if lower.contains("/domain/")
        || lower.contains("::domain::")
        || lower.contains(".domain.")
        || lower.contains("/domain")
    {
        return ImportCategory::InternalDomain;
    }
    if lower.contains("/application/")
        || lower.contains("::application::")
        || lower.contains(".application.")
        || lower.contains("/usecase/")
    {
        return ImportCategory::InternalApplication;
    }
    if lower.contains("/infrastructure/")
        || lower.contains("::infrastructure::")
        || lower.contains(".infrastructure.")
        || lower.contains("/adapter/")
    {
        return ImportCategory::InternalInfrastructure;
    }

    // Relative imports (TS) or crate-local (Rust)
    if import_path.starts_with("./")
        || import_path.starts_with("../")
        || import_path.starts_with("crate::")
        || import_path.starts_with("super::")
    {
        return ImportCategory::InternalDomain; // conservative default for relative imports
    }

    ImportCategory::External
}

fn classify_all_imports(dependencies: &[Dependency]) -> Vec<ClassifiedImport> {
    dependencies
        .iter()
        .filter_map(|dep| {
            dep.import_path.as_ref().map(|path| ClassifiedImport {
                import_path: path.clone(),
                category: classify_import(path),
                source_file: dep.location.file.clone(),
            })
        })
        .collect()
}

fn build_aggregates(
    entities: &[Component],
    value_objects: &[Component],
    classified_imports: &[ClassifiedImport],
) -> Vec<AggregateAnalysis> {
    entities
        .iter()
        .map(|entity| {
            let entity_file = &entity.location.file;

            // Find value objects that might be used by this entity
            let associated_vos: Vec<Component> =
                if let ComponentKind::Entity(ref info) = entity.kind {
                    value_objects
                        .iter()
                        .filter(|vo| {
                            // Check if any field type matches a value object name
                            info.fields.iter().any(|f| f.type_name.contains(&vo.name))
                        })
                        .cloned()
                        .collect()
                } else {
                    Vec::new()
                };

            // Build dependency audit from the entity's file imports
            let file_imports: Vec<&ClassifiedImport> = classified_imports
                .iter()
                .filter(|ci| ci.source_file == *entity_file)
                .collect();

            let stdlib_imports: Vec<String> = file_imports
                .iter()
                .filter(|ci| ci.category == ImportCategory::Stdlib)
                .map(|ci| ci.import_path.clone())
                .collect();

            let internal_domain_imports: Vec<String> = file_imports
                .iter()
                .filter(|ci| ci.category == ImportCategory::InternalDomain)
                .map(|ci| ci.import_path.clone())
                .collect();

            let external_imports: Vec<String> = file_imports
                .iter()
                .filter(|ci| ci.category == ImportCategory::External)
                .map(|ci| ci.import_path.clone())
                .collect();

            let infrastructure_leaks: Vec<String> = file_imports
                .iter()
                .filter(|ci| ci.category == ImportCategory::InternalInfrastructure)
                .map(|ci| ci.import_path.clone())
                .collect();

            let is_clean = infrastructure_leaks.is_empty();

            let dependency_audit = DependencyAudit {
                stdlib_imports,
                internal_domain_imports,
                external_imports,
                infrastructure_leaks,
                is_clean,
            };

            // Detect DDD patterns
            let ddd_patterns = detect_ddd_patterns(entity);

            AggregateAnalysis {
                component: entity.clone(),
                value_objects: associated_vos,
                dependency_audit,
                ddd_patterns,
            }
        })
        .collect()
}

fn detect_ddd_patterns(entity: &Component) -> Vec<DddPattern> {
    let mut patterns = Vec::new();

    if let ComponentKind::Entity(ref info) = entity.kind {
        let method_count = info.methods.len();

        // Rich domain model
        patterns.push(DddPattern {
            name: format!("Rich domain model ({method_count} methods)"),
            detected: method_count > 0,
        });

        // Factory method
        let has_factory = info
            .methods
            .iter()
            .any(|m| m.name.starts_with("New") || m.name.starts_with("Create"));
        patterns.push(DddPattern {
            name: "Factory method".to_string(),
            detected: has_factory,
        });

        // Has identity field
        let has_id = info.fields.iter().any(|f| {
            let fl = f.name.to_lowercase();
            fl == "id" || fl == "uuid"
        });
        patterns.push(DddPattern {
            name: "Identity field".to_string(),
            detected: has_id,
        });

        // Encapsulation (methods exist to manipulate state)
        patterns.push(DddPattern {
            name: "Encapsulation (methods)".to_string(),
            detected: method_count >= 2,
        });
    }

    patterns
}

fn generate_improvements(
    entities: &[Component],
    domain_events: &[Component],
    adapters: &[AdapterMapping],
    ports: &[Component],
    violations: &[Violation],
) -> Vec<String> {
    let mut suggestions = Vec::new();

    // Anemic domain models — only flag domain-layer entities, not infrastructure DTOs
    for entity in entities {
        if entity.layer != Some(ArchLayer::Domain) {
            continue;
        }
        if let ComponentKind::Entity(ref info) = entity.kind {
            if info.methods.is_empty() {
                suggestions.push(format!(
                    "Anemic domain model: `{}` has no business methods. Consider adding domain logic.",
                    entity.name
                ));
            }
        }
    }

    // No domain events
    if domain_events.is_empty() && !entities.is_empty() {
        suggestions.push(
            "No domain events found. Consider adding domain events for aggregate state changes."
                .to_string(),
        );
    }

    // Adapters without ports
    for adapter in adapters {
        if adapter.implements_ports.is_empty() {
            suggestions.push(format!(
                "Missing port interface for adapter `{}`.",
                adapter.adapter.name
            ));
        }
    }

    // Infrastructure leaks from violations
    for violation in violations {
        if let ViolationKind::DomainInfrastructureLeak { ref detail } = violation.kind {
            suggestions.push(format!("Infrastructure leak: {detail}"));
        }
    }

    // Large entities
    for entity in entities {
        if let ComponentKind::Entity(ref info) = entity.kind {
            if info.fields.len() > 10 {
                suggestions.push(format!(
                    "`{}` has {} fields. Consider breaking into smaller value objects.",
                    entity.name,
                    info.fields.len()
                ));
            }
        }
    }

    // Check port-to-adapter coverage
    let adapter_port_names: Vec<&str> = adapters
        .iter()
        .flat_map(|a| a.implements_ports.iter().map(|s| s.as_str()))
        .collect();

    for port in ports {
        if !adapter_port_names.iter().any(|name| *name == port.name) {
            suggestions.push(format!(
                "Port `{}` has no known adapter implementation.",
                port.name
            ));
        }
    }

    suggestions
}
