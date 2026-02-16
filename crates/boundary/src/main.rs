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
use boundary_core::types::Severity;

use boundary_go::GoAnalyzer;
use boundary_report::{json, text};

#[derive(Debug, Clone, Copy, ValueEnum)]
enum OutputFormat {
    Text,
    Json,
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
    },
    /// Create a default .boundary.toml configuration file
    Init {
        /// Overwrite existing config
        #[arg(long)]
        force: bool,
    },
}

fn main() {
    let cli = Cli::parse();

    let result = match cli.command {
        Commands::Analyze {
            path,
            config,
            format,
            compact,
        } => cmd_analyze(&path, config.as_deref(), format, compact),
        Commands::Check {
            path,
            fail_on,
            config,
            format,
            compact,
        } => cmd_check(&path, &fail_on, config.as_deref(), format, compact),
        Commands::Init { force } => cmd_init(force),
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

fn cmd_analyze(
    path: &Path,
    config_path: Option<&Path>,
    format: OutputFormat,
    compact: bool,
) -> Result<()> {
    validate_path(path)?;
    let config = load_config(path, config_path)?;
    let result = run_analysis(path, &config)?;

    let report = match format {
        OutputFormat::Text => text::format_report(&result),
        OutputFormat::Json => json::format_report(&result, compact),
    };
    println!("{report}");
    Ok(())
}

fn cmd_check(
    path: &Path,
    fail_on_str: &str,
    config_path: Option<&Path>,
    format: OutputFormat,
    compact: bool,
) -> Result<()> {
    validate_path(path)?;
    let config = load_config(path, config_path)?;
    let fail_on: Severity = fail_on_str.parse()?;
    let result = run_analysis(path, &config)?;

    let (report, passed) = match format {
        OutputFormat::Text => text::format_check(&result, fail_on),
        OutputFormat::Json => json::format_check(&result, fail_on, compact),
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

fn load_config(project_path: &Path, config_path: Option<&Path>) -> Result<Config> {
    match config_path {
        Some(p) => Config::load(p),
        None => Ok(Config::load_or_default(project_path)),
    }
}

/// Extracted per-file data before merging into the graph.
struct FileResult {
    components: Vec<(
        boundary_core::types::Component,
        Option<boundary_core::types::ArchLayer>,
    )>,
    dependencies: Vec<(
        boundary_core::types::Dependency,
        Option<boundary_core::types::ArchLayer>,
        Option<boundary_core::types::ArchLayer>,
    )>,
}

fn run_analysis(project_path: &Path, config: &Config) -> Result<metrics::AnalysisResult> {
    let analyzer = GoAnalyzer::new().context("failed to initialize Go analyzer")?;
    let classifier = LayerClassifier::new(&config.layers);
    let mut graph = DependencyGraph::new();
    let mut total_deps = 0usize;

    // Walk directory and find Go files
    let go_files: Vec<PathBuf> = WalkDir::new(project_path)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| {
            let p = e.path();
            p.extension().is_some_and(|ext| ext == "go")
                && !p.to_string_lossy().contains("vendor/")
                && !p.to_string_lossy().ends_with("_test.go")
        })
        .map(|e| e.into_path())
        .collect();

    if go_files.is_empty() {
        eprintln!("Warning: no Go files found in '{}'", project_path.display());
    }

    // Parse and extract in parallel
    let file_results: Vec<FileResult> = go_files
        .par_iter()
        .filter_map(|file_path| {
            let content = match std::fs::read_to_string(file_path) {
                Ok(c) => c,
                Err(e) => {
                    eprintln!("Warning: failed to read {}: {e}", file_path.display());
                    return None;
                }
            };

            let parsed = match analyzer.parse_file(file_path, &content) {
                Ok(p) => p,
                Err(e) => {
                    eprintln!("Warning: failed to parse {}: {e}", file_path.display());
                    return None;
                }
            };

            let rel_path = file_path
                .strip_prefix(project_path)
                .unwrap_or(file_path)
                .to_string_lossy();

            // Extract and classify components
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

            // Extract dependencies with layer info
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

            Some(FileResult {
                components,
                dependencies,
            })
        })
        .collect();

    // Merge results sequentially into graph
    for fr in file_results {
        for (comp, _) in &fr.components {
            graph.add_component(comp);
        }
        for (dep, from_layer, to_layer) in &fr.dependencies {
            graph.ensure_node(&dep.from, *from_layer);
            graph.ensure_node(&dep.to, *to_layer);
            graph.add_dependency(dep);
        }
        total_deps += fr.dependencies.len();
    }

    Ok(metrics::build_result(&graph, config, total_deps))
}
