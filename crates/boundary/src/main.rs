use std::path::{Path, PathBuf};
use std::process;

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use walkdir::WalkDir;

use boundary_core::analyzer::LanguageAnalyzer;
use boundary_core::config::Config;
use boundary_core::graph::DependencyGraph;
use boundary_core::layer::LayerClassifier;
use boundary_core::metrics;
use boundary_core::types::Severity;

use boundary_go::GoAnalyzer;
use boundary_report::text;

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
        Commands::Analyze { path, config } => cmd_analyze(&path, config.as_deref()),
        Commands::Check {
            path,
            fail_on,
            config,
        } => cmd_check(&path, &fail_on, config.as_deref()),
        Commands::Init { force } => cmd_init(force),
    };

    if let Err(e) = result {
        eprintln!("Error: {e:#}");
        process::exit(2);
    }
}

fn cmd_analyze(path: &Path, config_path: Option<&Path>) -> Result<()> {
    let config = load_config(path, config_path)?;
    let result = run_analysis(path, &config)?;
    let report = text::format_report(&result);
    print!("{report}");
    Ok(())
}

fn cmd_check(path: &Path, fail_on_str: &str, config_path: Option<&Path>) -> Result<()> {
    let config = load_config(path, config_path)?;
    let fail_on: Severity = fail_on_str.parse()?;
    let result = run_analysis(path, &config)?;
    let (report, passed) = text::format_check(&result, fail_on);
    print!("{report}");
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

    for file_path in &go_files {
        let content = std::fs::read_to_string(file_path)
            .with_context(|| format!("failed to read {}", file_path.display()))?;

        let parsed = match analyzer.parse_file(file_path, &content) {
            Ok(p) => p,
            Err(e) => {
                eprintln!("Warning: failed to parse {}: {e}", file_path.display());
                continue;
            }
        };

        // Extract and classify components
        let mut components = analyzer.extract_components(&parsed);
        let rel_path = file_path
            .strip_prefix(project_path)
            .unwrap_or(file_path)
            .to_string_lossy();

        for comp in &mut components {
            if comp.layer.is_none() {
                comp.layer = classifier.classify(&rel_path);
            }
        }

        for comp in &components {
            graph.add_component(comp);
        }

        // Extract and add dependencies
        let deps = analyzer.extract_dependencies(&parsed);
        for dep in &deps {
            // Classify the target by import path
            let to_layer = dep
                .import_path
                .as_deref()
                .and_then(|p| classifier.classify_import(p));

            // Ensure source node has a layer
            let from_layer = classifier.classify(&rel_path);
            graph.ensure_node(&dep.from, from_layer);
            graph.ensure_node(&dep.to, to_layer);

            graph.add_dependency(dep);
        }
        total_deps += deps.len();
    }

    Ok(metrics::build_result(&graph, config, total_deps))
}
