mod server;

use std::path::Path;

use anyhow::{Context, Result};
use tokio::io::{AsyncRead, AsyncWrite};
use tower_lsp::{LspService, Server};
use walkdir::WalkDir;

use boundary_core::analyzer::LanguageAnalyzer;
use boundary_core::config::Config;

use server::BoundaryLanguageServer;

/// Create analyzers based on auto-detection (used by LSP server).
pub fn create_analyzers(
    project_path: &Path,
    config: &Config,
) -> Result<Vec<Box<dyn LanguageAnalyzer>>> {
    let languages = if config.project.languages.is_empty() {
        auto_detect_languages(project_path)
    } else {
        config.project.languages.clone()
    };

    let mut analyzers: Vec<Box<dyn LanguageAnalyzer>> = Vec::new();

    for lang in &languages {
        match lang.as_str() {
            "go" => {
                analyzers.push(Box::new(
                    boundary_go::GoAnalyzer::new().context("failed to init Go analyzer")?,
                ));
            }
            "rust" => {
                analyzers.push(Box::new(
                    boundary_rust::RustAnalyzer::new().context("failed to init Rust analyzer")?,
                ));
            }
            "typescript" | "ts" => {
                analyzers.push(Box::new(
                    boundary_typescript::TypeScriptAnalyzer::new()
                        .context("failed to init TypeScript analyzer")?,
                ));
            }
            "java" => {
                analyzers.push(Box::new(
                    boundary_java::JavaAnalyzer::new().context("failed to init Java analyzer")?,
                ));
            }
            _ => {}
        }
    }

    if analyzers.is_empty() {
        anyhow::bail!("no supported language analyzers could be initialized");
    }

    Ok(analyzers)
}

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
                Some("ts" | "tsx") => has_ts = true,
                Some("java") => has_java = true,
                _ => {}
            }
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
        languages.push("go".to_string());
    }
    languages
}

async fn run_server<I, O>(input: I, output: O)
where
    I: AsyncRead + Unpin,
    O: AsyncWrite + Unpin,
{
    let (service, socket) = LspService::new(BoundaryLanguageServer::new);
    Server::new(input, output, socket).serve(service).await;
}

#[tokio::main]
async fn main() {
    let stdin = tokio::io::stdin();
    let stdout = tokio::io::stdout();
    run_server(stdin, stdout).await;
}
