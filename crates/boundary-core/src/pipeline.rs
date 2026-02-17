use std::path::{Path, PathBuf};

use anyhow::Result;
use rayon::prelude::*;
use walkdir::WalkDir;

use std::collections::HashMap;

use crate::analyzer::LanguageAnalyzer;
use crate::cache::{AnalysisCache, CachedFileResult};
use crate::config::Config;
use crate::graph::DependencyGraph;
use crate::layer::LayerClassifier;
use crate::metrics;
use crate::types::{ArchLayer, ArchitectureMode, Component, Dependency};

/// Full analysis output including the graph for diagram generation.
pub struct FullAnalysis {
    pub result: metrics::AnalysisResult,
    pub graph: DependencyGraph,
    pub components: Vec<Component>,
    pub dependencies: Vec<Dependency>,
}

/// A dependency with its resolved layer info and architecture context.
type ClassifiedDependency = (
    Dependency,
    Option<ArchLayer>,
    Option<ArchLayer>,
    bool,
    ArchitectureMode,
);

/// Extracted per-file data before merging into the graph.
struct FileResult {
    components: Vec<(Component, Option<ArchLayer>)>,
    dependencies: Vec<ClassifiedDependency>,
}

/// Reusable analysis pipeline that can be shared between CLI and LSP.
pub struct AnalysisPipeline {
    analyzers: Vec<Box<dyn LanguageAnalyzer>>,
    config: Config,
    classifier: LayerClassifier,
}

impl AnalysisPipeline {
    pub fn new(analyzers: Vec<Box<dyn LanguageAnalyzer>>, config: Config) -> Self {
        let classifier = LayerClassifier::new(&config.layers);
        Self {
            analyzers,
            config,
            classifier,
        }
    }

    /// Run a full analysis on the given project path.
    pub fn analyze(&self, project_path: &Path) -> Result<FullAnalysis> {
        self.analyze_inner(project_path, false)
    }

    /// Run an incremental analysis, using cached results for unchanged files.
    pub fn analyze_incremental(&self, project_path: &Path) -> Result<FullAnalysis> {
        self.analyze_inner(project_path, true)
    }

    /// Run a module-scoped analysis for forensics reporting.
    /// `module_path` is the directory to analyze.
    /// `project_root` is the project root for layer classification patterns.
    pub fn analyze_module(&self, module_path: &Path, project_root: &Path) -> Result<FullAnalysis> {
        let mut graph = DependencyGraph::new();
        let mut total_deps = 0usize;
        let mut all_components = Vec::new();
        let mut all_dependencies = Vec::new();

        for analyzer in &self.analyzers {
            let extensions: Vec<&str> = analyzer.file_extensions().to_vec();

            let source_files: Vec<PathBuf> = WalkDir::new(module_path)
                .into_iter()
                .filter_map(|e| e.ok())
                .filter(|e| {
                    let p = e.path();
                    let matches_ext = p
                        .extension()
                        .is_some_and(|ext| extensions.iter().any(|e| ext == *e));
                    if !matches_ext {
                        return false;
                    }
                    let path_str = p.to_string_lossy();
                    !path_str.contains("vendor/")
                        && !path_str.contains("/target/")
                        && !path_str.ends_with("_test.go")
                        && !path_str.ends_with(".d.ts")
                })
                .map(|e| e.into_path())
                .collect();

            if source_files.is_empty() {
                continue;
            }

            let classifier = &self.classifier;

            let file_results: Vec<FileResult> = source_files
                .par_iter()
                .filter_map(|file_path| {
                    let content = match std::fs::read_to_string(file_path) {
                        Ok(c) => c,
                        Err(e) => {
                            eprintln!("Warning: failed to read {}: {e}", file_path.display());
                            return None;
                        }
                    };

                    // Use project_root for relative path computation so layer patterns match
                    let rel_path = file_path
                        .strip_prefix(project_root)
                        .unwrap_or(file_path)
                        .to_string_lossy()
                        .to_string();

                    let parsed = match analyzer.parse_file(file_path, &content) {
                        Ok(p) => p,
                        Err(e) => {
                            eprintln!("Warning: failed to parse {}: {e}", file_path.display());
                            return None;
                        }
                    };

                    let mut components_raw = analyzer.extract_components(&parsed);
                    let file_layer = classifier.classify(&rel_path);
                    let is_cross_cutting = classifier.is_cross_cutting(&rel_path);
                    let arch_mode = classifier.architecture_mode(&rel_path);

                    let components: Vec<_> = components_raw
                        .drain(..)
                        .map(|mut comp| {
                            if comp.layer.is_none() {
                                comp.layer = file_layer;
                            }
                            comp.is_cross_cutting = is_cross_cutting;
                            comp.architecture_mode = arch_mode;
                            let layer = comp.layer;
                            (comp, layer)
                        })
                        .collect();

                    let deps = analyzer.extract_dependencies(&parsed);
                    let dependencies: Vec<_> = deps
                        .into_iter()
                        .map(|dep| {
                            let to_layer = dep
                                .import_path
                                .as_deref()
                                .and_then(|p| classifier.classify_import(p));
                            let from_layer = classifier.classify(&rel_path);
                            (dep, from_layer, to_layer, is_cross_cutting, arch_mode)
                        })
                        .collect();

                    Some(FileResult {
                        components,
                        dependencies,
                    })
                })
                .collect();

            for fr in file_results {
                for (comp, _) in &fr.components {
                    graph.add_component(comp);
                    all_components.push(comp.clone());
                }
                for (dep, from_layer, to_layer, is_cc, arch_mode) in &fr.dependencies {
                    graph.ensure_node_with_mode(&dep.from, *from_layer, *is_cc, *arch_mode);
                    graph.ensure_node(&dep.to, *to_layer, false);
                    graph.add_dependency(dep);
                    all_dependencies.push(dep.clone());
                }
                total_deps += fr.dependencies.len();
            }
        }

        let result = metrics::build_result(&graph, &self.config, total_deps, &all_components);
        Ok(FullAnalysis {
            result,
            graph,
            components: all_components,
            dependencies: all_dependencies,
        })
    }

    fn analyze_inner(&self, project_path: &Path, incremental: bool) -> Result<FullAnalysis> {
        let mut graph = DependencyGraph::new();
        let mut total_deps = 0usize;
        let mut all_components = Vec::new();
        let mut all_dependencies = Vec::new();

        let mut cache = if incremental {
            AnalysisCache::load(project_path).unwrap_or_default()
        } else {
            AnalysisCache::new()
        };

        for analyzer in &self.analyzers {
            let extensions: Vec<&str> = analyzer.file_extensions().to_vec();

            let source_files: Vec<PathBuf> = WalkDir::new(project_path)
                .into_iter()
                .filter_map(|e| e.ok())
                .filter(|e| {
                    let p = e.path();
                    let matches_ext = p
                        .extension()
                        .is_some_and(|ext| extensions.iter().any(|e| ext == *e));
                    if !matches_ext {
                        return false;
                    }
                    let path_str = p.to_string_lossy();
                    !path_str.contains("vendor/")
                        && !path_str.contains("/target/")
                        && !path_str.ends_with("_test.go")
                        && !path_str.ends_with(".d.ts")
                })
                .map(|e| e.into_path())
                .collect();

            if source_files.is_empty() {
                continue;
            }

            let classifier = &self.classifier;

            let file_results: Vec<(String, FileResult, String)> = source_files
                .par_iter()
                .filter_map(|file_path| {
                    let content = match std::fs::read_to_string(file_path) {
                        Ok(c) => c,
                        Err(e) => {
                            eprintln!("Warning: failed to read {}: {e}", file_path.display());
                            return None;
                        }
                    };

                    let rel_path = file_path
                        .strip_prefix(project_path)
                        .unwrap_or(file_path)
                        .to_string_lossy()
                        .to_string();

                    let is_cross_cutting = classifier.is_cross_cutting(&rel_path);
                    let arch_mode = classifier.architecture_mode(&rel_path);

                    if incremental {
                        if let Some(cached) = cache.get(&rel_path, &content) {
                            let file_layer = classifier.classify(&rel_path);
                            let components: Vec<_> = cached
                                .components
                                .iter()
                                .map(|comp| {
                                    let mut comp = comp.clone();
                                    if comp.layer.is_none() {
                                        comp.layer = file_layer;
                                    }
                                    comp.is_cross_cutting = is_cross_cutting;
                                    comp.architecture_mode = arch_mode;
                                    let layer = comp.layer;
                                    (comp, layer)
                                })
                                .collect();

                            let dependencies: Vec<_> = cached
                                .dependencies
                                .iter()
                                .map(|dep| {
                                    let to_layer = dep
                                        .import_path
                                        .as_deref()
                                        .and_then(|p| classifier.classify_import(p));
                                    let from_layer = classifier.classify(&rel_path);
                                    (
                                        dep.clone(),
                                        from_layer,
                                        to_layer,
                                        is_cross_cutting,
                                        arch_mode,
                                    )
                                })
                                .collect();

                            return Some((
                                rel_path,
                                FileResult {
                                    components,
                                    dependencies,
                                },
                                content,
                            ));
                        }
                    }

                    let parsed = match analyzer.parse_file(file_path, &content) {
                        Ok(p) => p,
                        Err(e) => {
                            eprintln!("Warning: failed to parse {}: {e}", file_path.display());
                            return None;
                        }
                    };

                    let mut components_raw = analyzer.extract_components(&parsed);
                    let file_layer = classifier.classify(&rel_path);

                    let components: Vec<_> = components_raw
                        .drain(..)
                        .map(|mut comp| {
                            if comp.layer.is_none() {
                                comp.layer = file_layer;
                            }
                            comp.is_cross_cutting = is_cross_cutting;
                            comp.architecture_mode = arch_mode;
                            let layer = comp.layer;
                            (comp, layer)
                        })
                        .collect();

                    let deps = analyzer.extract_dependencies(&parsed);
                    let dependencies: Vec<_> = deps
                        .into_iter()
                        .map(|dep| {
                            let to_layer = dep
                                .import_path
                                .as_deref()
                                .and_then(|p| classifier.classify_import(p));
                            let from_layer = classifier.classify(&rel_path);
                            (dep, from_layer, to_layer, is_cross_cutting, arch_mode)
                        })
                        .collect();

                    Some((
                        rel_path,
                        FileResult {
                            components,
                            dependencies,
                        },
                        content,
                    ))
                })
                .collect();

            let current_files: Vec<String> =
                file_results.iter().map(|(p, _, _)| p.clone()).collect();

            for (rel_path, fr, content) in file_results {
                if incremental {
                    let cached_components: Vec<_> =
                        fr.components.iter().map(|(comp, _)| comp.clone()).collect();
                    let cached_deps: Vec<_> = fr
                        .dependencies
                        .iter()
                        .map(|(dep, _, _, _, _)| dep.clone())
                        .collect();
                    cache.insert(
                        rel_path,
                        &content,
                        CachedFileResult {
                            hash: String::new(),
                            components: cached_components,
                            dependencies: cached_deps,
                        },
                    );
                }

                for (comp, _) in &fr.components {
                    graph.add_component(comp);
                    all_components.push(comp.clone());
                }
                for (dep, from_layer, to_layer, is_cc, arch_mode) in &fr.dependencies {
                    graph.ensure_node_with_mode(&dep.from, *from_layer, *is_cc, *arch_mode);
                    graph.ensure_node(&dep.to, *to_layer, false);
                    graph.add_dependency(dep);
                    all_dependencies.push(dep.clone());
                }
                total_deps += fr.dependencies.len();
            }

            if incremental {
                cache.prune(&current_files);
            }
        }

        if incremental {
            if let Err(e) = cache.save(project_path) {
                eprintln!("Warning: failed to save analysis cache: {e}");
            }
        }

        let result = metrics::build_result(&graph, &self.config, total_deps, &all_components);
        Ok(FullAnalysis {
            result,
            graph,
            components: all_components,
            dependencies: all_dependencies,
        })
    }

    /// Get a reference to the config.
    pub fn config(&self) -> &Config {
        &self.config
    }

    /// Run per-service analysis for monorepo support.
    /// Discovers services matching the pattern, analyzes each independently,
    /// and returns aggregate results.
    pub fn analyze_per_service(&self, project_path: &Path) -> Result<metrics::MultiServiceResult> {
        let pattern = self
            .config
            .project
            .services_pattern
            .as_deref()
            .unwrap_or("services/*");

        let service_dirs = discover_services(project_path, pattern);

        if service_dirs.is_empty() {
            anyhow::bail!(
                "no services found matching pattern '{}' in '{}'",
                pattern,
                project_path.display()
            );
        }

        let mut service_results = Vec::new();
        let mut import_paths_by_service: HashMap<String, Vec<String>> = HashMap::new();

        for service_dir in &service_dirs {
            let service_name = service_dir
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_else(|| service_dir.to_string_lossy().to_string());

            match self.analyze_module(service_dir, project_path) {
                Ok(analysis) => {
                    // Collect import paths for shared module detection
                    let imports: Vec<String> = analysis
                        .dependencies
                        .iter()
                        .filter_map(|d| d.import_path.clone())
                        .collect();
                    import_paths_by_service.insert(service_name.clone(), imports);

                    service_results.push(metrics::ServiceAnalysisResult {
                        service_name,
                        result: analysis.result,
                    });
                }
                Err(e) => {
                    eprintln!(
                        "Warning: failed to analyze service '{}': {e}",
                        service_dir.display()
                    );
                }
            }
        }

        // Detect shared modules (import paths used by 2+ services)
        let shared_modules = detect_shared_modules(&import_paths_by_service);

        let aggregate = metrics::aggregate_results(&service_results);

        Ok(metrics::MultiServiceResult {
            services: service_results,
            aggregate,
            shared_modules,
        })
    }
}

/// Discover service directories matching a glob pattern.
pub fn discover_services(project_path: &Path, pattern: &str) -> Vec<PathBuf> {
    let full_pattern = project_path.join(pattern).to_string_lossy().to_string();
    let mut dirs: Vec<PathBuf> = glob::glob(&full_pattern)
        .unwrap_or_else(|_| glob::glob("").unwrap())
        .filter_map(|entry| entry.ok())
        .filter(|p| p.is_dir())
        .collect();
    dirs.sort();
    dirs
}

/// Detect shared modules from import paths used by multiple services.
fn detect_shared_modules(
    import_paths_by_service: &HashMap<String, Vec<String>>,
) -> Vec<metrics::SharedModule> {
    let mut path_to_services: HashMap<String, Vec<String>> = HashMap::new();

    for (service, imports) in import_paths_by_service {
        for import in imports {
            path_to_services
                .entry(import.clone())
                .or_default()
                .push(service.clone());
        }
    }

    let mut shared: Vec<_> = path_to_services
        .into_iter()
        .filter(|(_, services)| services.len() > 1)
        .map(|(path, mut used_by)| {
            used_by.sort();
            used_by.dedup();
            metrics::SharedModule { path, used_by }
        })
        .collect();

    shared.sort_by(|a, b| a.path.cmp(&b.path));
    shared
}

/// Walk up from `start` looking for `.boundary.toml` or `.git` to find the project root.
pub fn find_project_root(start: &Path) -> Option<PathBuf> {
    let mut current = if start.is_file() {
        start.parent()?.to_path_buf()
    } else {
        start.to_path_buf()
    };

    loop {
        if current.join(".boundary.toml").exists() || current.join(".git").exists() {
            return Some(current);
        }
        if !current.pop() {
            return None;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_discover_services_finds_matching_dirs() {
        let tmp = tempfile::tempdir().unwrap();
        let base = tmp.path();
        std::fs::create_dir_all(base.join("services/auth")).unwrap();
        std::fs::create_dir_all(base.join("services/billing")).unwrap();
        std::fs::create_dir_all(base.join("other/stuff")).unwrap();

        let dirs = discover_services(base, "services/*");
        assert_eq!(dirs.len(), 2);
        let names: Vec<_> = dirs
            .iter()
            .map(|d| d.file_name().unwrap().to_str().unwrap())
            .collect();
        assert!(names.contains(&"auth"));
        assert!(names.contains(&"billing"));
    }

    #[test]
    fn test_discover_services_no_matches() {
        let tmp = tempfile::tempdir().unwrap();
        let dirs = discover_services(tmp.path(), "services/*");
        assert!(dirs.is_empty());
    }

    #[test]
    fn test_detect_shared_modules() {
        let mut import_map = HashMap::new();
        import_map.insert(
            "auth".to_string(),
            vec!["pkg/logger".to_string(), "pkg/db".to_string()],
        );
        import_map.insert(
            "billing".to_string(),
            vec!["pkg/logger".to_string(), "pkg/events".to_string()],
        );

        let shared = detect_shared_modules(&import_map);
        assert_eq!(shared.len(), 1);
        assert_eq!(shared[0].path, "pkg/logger");
        assert!(shared[0].used_by.contains(&"auth".to_string()));
        assert!(shared[0].used_by.contains(&"billing".to_string()));
    }
}
