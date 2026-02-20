use std::path::{Path, PathBuf};
use std::process;

use anyhow::{Context, Result};
use clap::{Parser, Subcommand, ValueEnum};
use rayon::prelude::*;
use walkdir::WalkDir;

use boundary_core::analyzer::LanguageAnalyzer;
use boundary_core::config::Config;
use boundary_core::graph::DependencyGraph;
use boundary_core::layer::LayerClassifier;
use boundary_core::metrics;
use boundary_core::pipeline::{self, AnalysisPipeline};
use boundary_core::types::Severity;

use boundary_go::GoAnalyzer;
use boundary_java::JavaAnalyzer;
use boundary_report::{json, text};
use boundary_rust::RustAnalyzer;
use boundary_typescript::TypeScriptAnalyzer;

#[derive(Debug, Clone, Copy, ValueEnum)]
enum OutputFormat {
    Text,
    Json,
    Markdown,
}

#[derive(Parser)]
#[command(name = "boundary")]
#[command(about = "Analyze and score DDD/Hexagonal architecture boundaries")]
#[command(version)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Analyze a codebase and print a full architecture report
    Analyze {
        /// Path to the project root
        path: PathBuf,
        /// Config file path (defaults to .boundary.toml in project root)
        #[arg(short, long)]
        config: Option<PathBuf>,
        /// Output format
        #[arg(long, value_enum, default_value_t = OutputFormat::Text)]
        format: OutputFormat,
        /// Compact output (single-line JSON, no colors for text)
        #[arg(long)]
        compact: bool,
        /// Languages to analyze (auto-detect if not specified)
        #[arg(long, value_delimiter = ',')]
        languages: Option<Vec<String>>,
        /// Use incremental analysis (cache unchanged files)
        #[arg(long)]
        incremental: bool,
        /// Analyze each service independently (monorepo support)
        #[arg(long)]
        per_service: bool,
        /// Output only the architecture score (one line)
        #[arg(long)]
        score_only: bool,
    },
    /// Analyze and exit with code 0 (pass) or 1 (fail)
    Check {
        /// Path to the project root
        path: PathBuf,
        /// Minimum severity to cause failure
        #[arg(long, default_value = "error")]
        fail_on: String,
        /// Config file path
        #[arg(short, long)]
        config: Option<PathBuf>,
        /// Output format
        #[arg(long, value_enum, default_value_t = OutputFormat::Text)]
        format: OutputFormat,
        /// Compact output (single-line JSON, no colors for text)
        #[arg(long)]
        compact: bool,
        /// Languages to analyze (auto-detect if not specified)
        #[arg(long, value_delimiter = ',')]
        languages: Option<Vec<String>>,
        /// Save analysis snapshot for evolution tracking
        #[arg(long)]
        track: bool,
        /// Fail if architecture score regresses from last snapshot
        #[arg(long)]
        no_regression: bool,
        /// Use incremental analysis (cache unchanged files)
        #[arg(long)]
        incremental: bool,
        /// Analyze each service independently (monorepo support)
        #[arg(long)]
        per_service: bool,
    },
    /// Create a default .boundary.toml configuration file
    Init {
        /// Overwrite existing config
        #[arg(long)]
        force: bool,
    },
    /// Generate an architecture diagram (Mermaid or DOT format)
    Diagram {
        /// Path to the project root
        path: PathBuf,
        /// Config file path
        #[arg(short, long)]
        config: Option<PathBuf>,
        /// Diagram type
        #[arg(long, value_enum, default_value_t = DiagramType::Layers)]
        diagram_type: DiagramType,
        /// Languages to analyze (auto-detect if not specified)
        #[arg(long, value_delimiter = ',')]
        languages: Option<Vec<String>>,
    },
    /// Generate a detailed forensics report for a module
    Forensics {
        /// Path to the module directory
        path: PathBuf,
        /// Project root (auto-detected if not specified)
        #[arg(long)]
        project_root: Option<PathBuf>,
        /// Config file path
        #[arg(short, long)]
        config: Option<PathBuf>,
        /// Languages to analyze (auto-detect if not specified)
        #[arg(long, value_delimiter = ',')]
        languages: Option<Vec<String>>,
        /// Write output to file instead of stdout
        #[arg(short, long)]
        output: Option<PathBuf>,
    },
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum DiagramType {
    Layers,
    Dependencies,
    Dot,
    DotDependencies,
}

fn main() {
    let cli = Cli::parse();

    let result = match cli.command {
        Commands::Analyze {
            path,
            config,
            format,
            compact,
            languages,
            incremental,
            per_service,
            score_only,
        } => cmd_analyze(
            &path,
            config.as_deref(),
            format,
            compact,
            languages.as_deref(),
            incremental,
            per_service,
            score_only,
        ),
        Commands::Check {
            path,
            fail_on,
            config,
            format,
            compact,
            languages,
            track,
            no_regression,
            incremental,
            per_service,
        } => cmd_check(
            &path,
            &fail_on,
            config.as_deref(),
            format,
            compact,
            languages.as_deref(),
            track,
            no_regression,
            incremental,
            per_service,
        ),
        Commands::Init { force } => cmd_init(force),
        Commands::Diagram {
            path,
            config,
            diagram_type,
            languages,
        } => cmd_diagram(&path, config.as_deref(), diagram_type, languages.as_deref()),
        Commands::Forensics {
            path,
            project_root,
            config,
            languages,
            output,
        } => cmd_forensics(
            &path,
            project_root.as_deref(),
            config.as_deref(),
            languages.as_deref(),
            output.as_deref(),
        ),
    };

    if let Err(e) = result {
        eprintln!("Error: {e:#}");
        process::exit(2);
    }
}

fn validate_path(path: &Path) -> Result<()> {
    if !path.exists() {
        anyhow::bail!("path '{}' does not exist", path.display());
    }
    if !path.is_dir() {
        anyhow::bail!("path '{}' is not a directory", path.display());
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn cmd_analyze(
    path: &Path,
    config_path: Option<&Path>,
    format: OutputFormat,
    compact: bool,
    languages: Option<&[String]>,
    incremental: bool,
    per_service: bool,
    score_only: bool,
) -> Result<()> {
    validate_path(path)?;
    let project_root = resolve_project_root(path, config_path);
    let config = load_config(&project_root, config_path)?;

    if per_service {
        let analyzers = create_analyzers(path, &config, languages)?;
        let pipeline = AnalysisPipeline::new(analyzers, config);
        let multi = pipeline.analyze_per_service(path)?;

        if score_only {
            for svc in &multi.services {
                print_score_only(&svc.service_name, &svc.result.score, format);
            }
            return Ok(());
        }

        let report = match format {
            OutputFormat::Text => text::format_multi_service_report(&multi),
            OutputFormat::Json => json::format_multi_service_report(&multi, compact),
            OutputFormat::Markdown => {
                boundary_report::markdown::format_multi_service_report(&multi)
            }
        };
        println!("{report}");
        return Ok(());
    }

    let analysis = run_analysis(path, &project_root, &config, languages, incremental)?;

    if score_only {
        let module_name = path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| path.to_string_lossy().into_owned());
        print_score_only(&module_name, &analysis.result.score, format);
        return Ok(());
    }

    let report = match format {
        OutputFormat::Text => text::format_report(&analysis.result),
        OutputFormat::Json => json::format_report(&analysis.result, compact),
        OutputFormat::Markdown => boundary_report::markdown::format_report(&analysis.result),
    };
    println!("{report}");
    Ok(())
}

fn print_score_only(module: &str, score: &metrics::ArchitectureScore, format: OutputFormat) {
    match format {
        OutputFormat::Json => {
            println!(
                "{{\"module\":\"{}\",\"overall\":{:.1},\"structural_presence\":{:.1},\"layer_isolation\":{:.1},\"dependency_direction\":{:.1},\"interface_coverage\":{:.1}}}",
                module, score.overall, score.structural_presence, score.layer_isolation, score.dependency_direction, score.interface_coverage
            );
        }
        OutputFormat::Text | OutputFormat::Markdown => {
            println!(
                "{}: {:.1}/100 (Presence: {:.1}, Layer: {:.1}, Deps: {:.1}, Interfaces: {:.1})",
                module,
                score.overall,
                score.structural_presence,
                score.layer_isolation,
                score.dependency_direction,
                score.interface_coverage
            );
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn cmd_check(
    path: &Path,
    fail_on_str: &str,
    config_path: Option<&Path>,
    format: OutputFormat,
    compact: bool,
    languages: Option<&[String]>,
    track: bool,
    no_regression: bool,
    incremental: bool,
    per_service: bool,
) -> Result<()> {
    validate_path(path)?;
    let project_root = resolve_project_root(path, config_path);
    let config = load_config(&project_root, config_path)?;
    let fail_on: Severity = fail_on_str.parse()?;

    if per_service {
        let analyzers = create_analyzers(path, &config, languages)?;
        let pipeline = AnalysisPipeline::new(analyzers, config);
        let multi = pipeline.analyze_per_service(path)?;

        let report = match format {
            OutputFormat::Text => text::format_multi_service_report(&multi),
            OutputFormat::Json => json::format_multi_service_report(&multi, compact),
            OutputFormat::Markdown => {
                boundary_report::markdown::format_multi_service_report(&multi)
            }
        };
        println!("{report}");

        // Check if any service has failing violations
        let has_failures = multi
            .services
            .iter()
            .any(|s| s.result.violations.iter().any(|v| v.severity >= fail_on));
        if has_failures {
            process::exit(1);
        }
        return Ok(());
    }

    let analysis = run_analysis(path, &project_root, &config, languages, incremental)?;

    // Evolution tracking
    if track {
        boundary_core::evolution::save_snapshot(path, &analysis.result)?;
    }
    if no_regression {
        if let Some(trend) = boundary_core::evolution::check_regression(path, &analysis.result)? {
            let (report, _) = match format {
                OutputFormat::Text => text::format_check(&analysis.result, fail_on),
                OutputFormat::Json => json::format_check(&analysis.result, fail_on, compact),
                OutputFormat::Markdown => {
                    boundary_report::markdown::format_check(&analysis.result, fail_on)
                }
            };
            println!("{report}");
            eprintln!(
                "Score regression detected: {:.1} -> {:.1} (delta: {:.1})",
                trend.previous_score, trend.current_score, trend.score_delta
            );
            process::exit(1);
        }
    }

    let (report, passed) = match format {
        OutputFormat::Text => text::format_check(&analysis.result, fail_on),
        OutputFormat::Json => json::format_check(&analysis.result, fail_on, compact),
        OutputFormat::Markdown => {
            boundary_report::markdown::format_check(&analysis.result, fail_on)
        }
    };
    println!("{report}");
    if !passed {
        process::exit(1);
    }
    Ok(())
}

fn cmd_init(force: bool) -> Result<()> {
    let target = PathBuf::from(".boundary.toml");
    if target.exists() && !force {
        anyhow::bail!(".boundary.toml already exists. Use --force to overwrite.");
    }
    std::fs::write(&target, Config::default_toml())?;
    println!("Created .boundary.toml with default configuration.");
    Ok(())
}

fn cmd_diagram(
    path: &Path,
    config_path: Option<&Path>,
    diagram_type: DiagramType,
    languages: Option<&[String]>,
) -> Result<()> {
    validate_path(path)?;
    let project_root = resolve_project_root(path, config_path);
    let config = load_config(&project_root, config_path)?;
    let analysis = run_analysis(path, &project_root, &config, languages, false)?;

    let diagram = match diagram_type {
        DiagramType::Layers => boundary_report::diagram::generate_layer_diagram(&analysis.graph),
        DiagramType::Dependencies => {
            boundary_report::diagram::generate_dependency_flow(&analysis.graph)
        }
        DiagramType::Dot => boundary_report::dot::generate_layer_diagram(&analysis.graph),
        DiagramType::DotDependencies => {
            boundary_report::dot::generate_dependency_flow(&analysis.graph)
        }
    };
    println!("{diagram}");
    Ok(())
}

fn cmd_forensics(
    module_path: &Path,
    project_root_override: Option<&Path>,
    config_path: Option<&Path>,
    languages: Option<&[String]>,
    output_path: Option<&Path>,
) -> Result<()> {
    validate_path(module_path)?;

    // Canonicalize so find_project_root walks absolute ancestors
    let module_path = module_path
        .canonicalize()
        .with_context(|| format!("failed to resolve path '{}'", module_path.display()))?;

    // Determine project root
    let project_root = if let Some(root) = project_root_override {
        root.to_path_buf()
    } else {
        pipeline::find_project_root(&module_path).unwrap_or_else(|| module_path.to_path_buf())
    };

    validate_path(&project_root)?;

    let config = load_config(&project_root, config_path)?;
    let analyzers = create_analyzers(&project_root, &config, languages)?;
    let pipeline = AnalysisPipeline::new(analyzers, config);

    let full_analysis = pipeline.analyze_module(&module_path, &project_root)?;
    let forensics =
        boundary_core::forensics::build_forensics(&full_analysis, &module_path, &project_root);
    let report = boundary_report::forensics::format_forensics_report(&forensics);

    if let Some(out_path) = output_path {
        std::fs::write(out_path, &report)
            .with_context(|| format!("failed to write output to {}", out_path.display()))?;
        eprintln!("Forensics report written to {}", out_path.display());
    } else {
        println!("{report}");
    }

    Ok(())
}

fn load_config(project_path: &Path, config_path: Option<&Path>) -> Result<Config> {
    match config_path {
        Some(p) => Config::load(p),
        None => Ok(Config::load_or_default(project_path)),
    }
}

/// Resolve the project root directory for path normalization.
///
/// When `--config` is explicit, derives root from the config file's parent.
/// Otherwise walks ancestors looking for `.boundary.toml` or `.git`.
/// Falls back to `analysis_path` if nothing found.
fn resolve_project_root(analysis_path: &Path, config_path: Option<&Path>) -> PathBuf {
    if let Some(cp) = config_path {
        if let Some(parent) = cp.parent() {
            if parent.exists() {
                return parent.to_path_buf();
            }
        }
    }
    pipeline::find_project_root(analysis_path).unwrap_or_else(|| analysis_path.to_path_buf())
}

/// Full analysis output including the graph for diagram generation.
pub struct FullAnalysis {
    pub result: metrics::AnalysisResult,
    pub graph: DependencyGraph,
}

/// A dependency with its resolved layer info and architecture context.
type ClassifiedDependency = (
    boundary_core::types::Dependency,
    Option<boundary_core::types::ArchLayer>,
    Option<boundary_core::types::ArchLayer>,
    bool,
    boundary_core::types::ArchitectureMode,
    bool, // to_is_cross_cutting
);

/// Extracted per-file data before merging into the graph.
struct FileResult {
    components: Vec<(
        boundary_core::types::Component,
        Option<boundary_core::types::ArchLayer>,
    )>,
    dependencies: Vec<ClassifiedDependency>,
}

/// Create analyzers based on languages config or auto-detection.
fn create_analyzers(
    project_path: &Path,
    config: &Config,
    language_override: Option<&[String]>,
) -> Result<Vec<Box<dyn LanguageAnalyzer>>> {
    let languages: Vec<String> = if let Some(langs) = language_override {
        langs.to_vec()
    } else if config.project.languages.is_empty() {
        // Auto-detect based on file extensions present
        auto_detect_languages(project_path)
    } else {
        config.project.languages.clone()
    };

    let mut analyzers: Vec<Box<dyn LanguageAnalyzer>> = Vec::new();

    for lang in &languages {
        match lang.as_str() {
            "go" => {
                analyzers.push(Box::new(
                    GoAnalyzer::new().context("failed to init Go analyzer")?,
                ));
            }
            "rust" => {
                analyzers.push(Box::new(
                    RustAnalyzer::new().context("failed to init Rust analyzer")?,
                ));
            }
            "typescript" | "ts" => {
                analyzers.push(Box::new(
                    TypeScriptAnalyzer::new().context("failed to init TypeScript analyzer")?,
                ));
            }
            "java" => {
                analyzers.push(Box::new(
                    JavaAnalyzer::new().context("failed to init Java analyzer")?,
                ));
            }
            other => {
                eprintln!("Warning: unsupported language '{other}', skipping");
            }
        }
    }

    if analyzers.is_empty() {
        anyhow::bail!("no supported language analyzers could be initialized");
    }

    Ok(analyzers)
}

/// Auto-detect languages by scanning for file extensions.
fn auto_detect_languages(project_path: &Path) -> Vec<String> {
    let mut has_go = false;
    let mut has_rust = false;
    let mut has_ts = false;
    let mut has_java = false;

    for entry in WalkDir::new(project_path)
        .into_iter()
        .filter_map(|e| e.ok())
        .take(1000)
    {
        if let Some(ext) = entry.path().extension() {
            match ext.to_str() {
                Some("go") => has_go = true,
                Some("rs") => has_rust = true,
                Some("ts" | "tsx") => {
                    // Skip .d.ts files
                    if !entry.path().to_string_lossy().ends_with(".d.ts") {
                        has_ts = true;
                    }
                }
                Some("java") => has_java = true,
                _ => {}
            }
        }
        if has_go && has_rust && has_ts && has_java {
            break;
        }
    }

    let mut languages = Vec::new();
    if has_go {
        languages.push("go".to_string());
    }
    if has_rust {
        languages.push("rust".to_string());
    }
    if has_ts {
        languages.push("typescript".to_string());
    }
    if has_java {
        languages.push("java".to_string());
    }
    if languages.is_empty() {
        // Fallback to Go for backward compat
        languages.push("go".to_string());
    }
    languages
}

fn run_analysis(
    project_path: &Path,
    project_root: &Path,
    config: &Config,
    language_override: Option<&[String]>,
    incremental: bool,
) -> Result<FullAnalysis> {
    let analyzers = create_analyzers(project_path, config, language_override)?;
    let classifier = LayerClassifier::new(&config.layers);
    let mut graph = DependencyGraph::new();
    let mut total_deps = 0usize;
    let mut total_files = 0usize;
    let mut all_components = Vec::new();

    // Load cache if incremental
    let mut cache = if incremental {
        boundary_core::cache::AnalysisCache::load(project_path).unwrap_or_default()
    } else {
        boundary_core::cache::AnalysisCache::new()
    };

    for analyzer in &analyzers {
        let extensions: Vec<&str> = analyzer.file_extensions().to_vec();

        // Walk directory and find matching files
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
                // Common exclusions
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
        total_files += source_files.len();

        // Parse and extract in parallel
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
                    .strip_prefix(project_root)
                    .unwrap_or(file_path)
                    .to_string_lossy()
                    .to_string();

                let is_cross_cutting = classifier.is_cross_cutting(&rel_path);
                let arch_mode = classifier.architecture_mode(&rel_path);

                // Check cache for incremental analysis
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
                            .filter(|dep| {
                                !dep.import_path
                                    .as_deref()
                                    .is_some_and(|p| analyzer.is_stdlib_import(p))
                            })
                            .map(|dep| {
                                let to_layer = dep
                                    .import_path
                                    .as_deref()
                                    .and_then(|p| classifier.classify_import(p));
                                let to_is_cross_cutting = dep
                                    .import_path
                                    .as_deref()
                                    .is_some_and(|p| classifier.is_cross_cutting_import(p));
                                let from_layer = classifier.classify(&rel_path);
                                (
                                    dep.clone(),
                                    from_layer,
                                    to_layer,
                                    is_cross_cutting,
                                    arch_mode,
                                    to_is_cross_cutting,
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

                // Extract and classify components
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

                // Extract dependencies with layer info
                let deps = analyzer.extract_dependencies(&parsed);
                let dependencies: Vec<_> = deps
                    .into_iter()
                    .filter(|dep| {
                        !dep.import_path
                            .as_deref()
                            .is_some_and(|p| analyzer.is_stdlib_import(p))
                    })
                    .map(|dep| {
                        let to_layer = dep
                            .import_path
                            .as_deref()
                            .and_then(|p| classifier.classify_import(p));
                        let to_is_cross_cutting = dep
                            .import_path
                            .as_deref()
                            .is_some_and(|p| classifier.is_cross_cutting_import(p));
                        let from_layer = classifier.classify(&rel_path);
                        (
                            dep,
                            from_layer,
                            to_layer,
                            is_cross_cutting,
                            arch_mode,
                            to_is_cross_cutting,
                        )
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

        // Collect rel_paths for pruning
        let current_files: Vec<String> = file_results.iter().map(|(p, _, _)| p.clone()).collect();

        // First pass: add all source file components and update cache
        for (rel_path, fr, content) in &file_results {
            if incremental {
                let cached_components: Vec<_> =
                    fr.components.iter().map(|(comp, _)| comp.clone()).collect();
                let cached_deps: Vec<_> = fr
                    .dependencies
                    .iter()
                    .map(|(dep, _, _, _, _, _)| dep.clone())
                    .collect();
                cache.insert(
                    rel_path.clone(),
                    content,
                    boundary_core::cache::CachedFileResult {
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
        }

        // Second pass: add dependencies
        for (_rel_path, fr, _content) in file_results {
            for (dep, from_layer, to_layer, is_cc, arch_mode, to_is_cc) in &fr.dependencies {
                graph.ensure_node_with_mode(&dep.from, *from_layer, *is_cc, *arch_mode);
                graph.ensure_node(&dep.to, *to_layer, *to_is_cc);
                graph.add_dependency(dep);
            }
            total_deps += fr.dependencies.len();
        }

        // Prune deleted files from cache
        if incremental {
            cache.prune(&current_files);
        }
    }

    // Save cache if incremental
    if incremental {
        if let Err(e) = cache.save(project_path) {
            eprintln!("Warning: failed to save analysis cache: {e}");
        }
    }

    // Mark dependency-only nodes as external if they don't correspond to any
    // analyzed source file. Source components (added via add_component) have
    // kind: Some(...); dependency-target nodes (via ensure_node) have kind: None.
    // Among kind:None nodes, check if the import path matches any source directory.
    let source_ids: std::collections::HashSet<_> = all_components.iter().map(|c| &c.id).collect();
    let source_rel_dirs: std::collections::HashSet<String> = all_components
        .iter()
        .filter_map(|c| {
            let rel = c
                .location
                .file
                .strip_prefix(project_root)
                .unwrap_or(&c.location.file);
            rel.parent().map(|p| p.to_string_lossy().replace('\\', "/"))
        })
        .collect();
    let project_root_str = project_root.to_string_lossy().replace('\\', "/");
    let external_ids: Vec<_> = graph
        .nodes()
        .iter()
        .filter(|n| {
            if source_ids.contains(&n.id) {
                return false;
            }
            // Extract the path portion before "::" (component IDs use path::name format)
            let id = n.id.0.replace('\\', "/");
            let path_part = id.split("::").next().unwrap_or(&id);
            // Relative imports (starting with . or ..) are always internal
            if path_part.starts_with('.') {
                return false;
            }
            // Rust crate-internal imports
            if path_part.starts_with("crate") {
                return false;
            }
            // Absolute paths under the project directory are internal
            if path_part.starts_with(project_root_str.as_str()) {
                return false;
            }
            // Also normalize dots to slashes for Java-style package names
            let path_normalized = path_part.replace('.', "/");
            // Check if this path corresponds to any analyzed source directory
            let is_internal = source_rel_dirs.iter().any(|dir| {
                if dir.is_empty() {
                    return false;
                }
                // Direct suffix match (Go-style fully-qualified imports)
                if path_part.ends_with(dir.as_str()) {
                    return true;
                }
                // Check if import path and source dir share consecutive path segments
                // (catches Java dot-notation imports like com.example.domain.user)
                let dir_segments: Vec<&str> = dir.split('/').collect();
                if dir_segments.len() >= 2 {
                    for window in dir_segments.windows(2) {
                        let pair = format!("{}/{}", window[0], window[1]);
                        if path_normalized.contains(&pair) {
                            return true;
                        }
                    }
                }
                false
            });
            !is_internal
        })
        .map(|n| n.id.clone())
        .collect();
    for id in &external_ids {
        graph.mark_external(id);
    }

    let result = metrics::build_result(&graph, config, total_deps, &all_components, total_files);
    Ok(FullAnalysis { result, graph })
}
