use std::path::{Path, PathBuf};

use anyhow::Result;
use rayon::prelude::*;
use walkdir::WalkDir;

use crate::analyzer::LanguageAnalyzer;
use crate::cache::{AnalysisCache, CachedFileResult};
use crate::config::Config;
use crate::graph::DependencyGraph;
use crate::layer::LayerClassifier;
use crate::metrics;
use crate::types::{ArchLayer, Component, Dependency};

/// Full analysis output including the graph for diagram generation.
pub struct FullAnalysis {
    pub result: metrics::AnalysisResult,
    pub graph: DependencyGraph,
}

/// Extracted per-file data before merging into the graph.
struct FileResult {
    components: Vec<(Component, Option<ArchLayer>)>,
    dependencies: Vec<(Dependency, Option<ArchLayer>, Option<ArchLayer>)>,
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

    fn analyze_inner(&self, project_path: &Path, incremental: bool) -> Result<FullAnalysis> {
        let mut graph = DependencyGraph::new();
        let mut total_deps = 0usize;
        let mut all_components = Vec::new();

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
                                    (dep.clone(), from_layer, to_layer)
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
                            (dep, from_layer, to_layer)
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
                        .map(|(dep, _, _)| dep.clone())
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
                for (dep, from_layer, to_layer) in &fr.dependencies {
                    graph.ensure_node(&dep.from, *from_layer);
                    graph.ensure_node(&dep.to, *to_layer);
                    graph.add_dependency(dep);
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
        Ok(FullAnalysis { result, graph })
    }

    /// Get a reference to the config.
    pub fn config(&self) -> &Config {
        &self.config
    }
}
