use std::path::PathBuf;
use std::sync::Arc;

use tokio::sync::Mutex;
use tower_lsp::jsonrpc::Result;
use tower_lsp::lsp_types::*;
use tower_lsp::{Client, LanguageServer};

use boundary_core::config::Config;
use boundary_core::pipeline::{AnalysisPipeline, FullAnalysis};
use boundary_core::types::{Severity, ViolationKind};

use crate::create_analyzers;

pub struct BoundaryLanguageServer {
    client: Client,
    pipeline: Arc<Mutex<Option<AnalysisPipeline>>>,
    project_root: Arc<Mutex<Option<PathBuf>>>,
    last_analysis: Arc<Mutex<Option<FullAnalysis>>>,
}

impl BoundaryLanguageServer {
    pub fn new(client: Client) -> Self {
        Self {
            client,
            pipeline: Arc::new(Mutex::new(None)),
            project_root: Arc::new(Mutex::new(None)),
            last_analysis: Arc::new(Mutex::new(None)),
        }
    }

    async fn initialize_pipeline(&self, root: PathBuf) {
        let config = Config::load_or_default(&root);
        match create_analyzers(&root, &config) {
            Ok(analyzers) => {
                let pipeline = AnalysisPipeline::new(analyzers, config);
                *self.pipeline.lock().await = Some(pipeline);
                *self.project_root.lock().await = Some(root);
                self.client
                    .log_message(MessageType::INFO, "Boundary LSP: pipeline initialized")
                    .await;
            }
            Err(e) => {
                self.client
                    .log_message(
                        MessageType::ERROR,
                        format!("Boundary LSP: failed to initialize analyzers: {e}"),
                    )
                    .await;
            }
        }
    }

    async fn run_analysis_and_publish(&self) {
        let analysis = {
            let pipeline = self.pipeline.lock().await;
            let root = self.project_root.lock().await;

            let (Some(pipeline), Some(root)) = (pipeline.as_ref(), root.as_ref()) else {
                return;
            };

            match pipeline.analyze_incremental(root) {
                Ok(analysis) => analysis,
                Err(e) => {
                    self.client
                        .log_message(
                            MessageType::ERROR,
                            format!("Boundary LSP: analysis failed: {e}"),
                        )
                        .await;
                    return;
                }
            }
        };
        // Locks are dropped here before publish_diagnostics acquires project_root lock
        self.publish_diagnostics(&analysis).await;
        *self.last_analysis.lock().await = Some(analysis);
    }

    async fn publish_diagnostics(&self, analysis: &FullAnalysis) {
        // Group violations by file
        let mut diagnostics_by_file: std::collections::HashMap<Url, Vec<Diagnostic>> =
            std::collections::HashMap::new();

        for violation in &analysis.result.violations {
            let file_path = &violation.location.file;
            let line = violation.location.line.saturating_sub(1) as u32;
            let col = violation.location.column.saturating_sub(1) as u32;

            let severity = match violation.severity {
                Severity::Error => DiagnosticSeverity::ERROR,
                Severity::Warning => DiagnosticSeverity::WARNING,
                Severity::Info => DiagnosticSeverity::INFORMATION,
            };

            let kind_label = match &violation.kind {
                ViolationKind::LayerBoundary {
                    from_layer,
                    to_layer,
                } => format!("layer-boundary: {from_layer} -> {to_layer}"),
                ViolationKind::CircularDependency { .. } => "circular-dependency".to_string(),
                ViolationKind::MissingPort { adapter_name } => {
                    format!("missing-port: {adapter_name}")
                }
                ViolationKind::CustomRule { rule_name } => format!("custom-rule: {rule_name}"),
                ViolationKind::DomainInfrastructureLeak { detail } => {
                    format!("domain-infra-leak: {detail}")
                }
            };

            let diagnostic = Diagnostic {
                range: Range {
                    start: Position::new(line, col),
                    end: Position::new(line, col + 1),
                },
                severity: Some(severity),
                code: Some(NumberOrString::String(kind_label)),
                source: Some("boundary".to_string()),
                message: violation.message.clone(),
                related_information: None,
                tags: None,
                code_description: None,
                data: None,
            };

            // Try to construct a URI from the file path
            let root = self.project_root.lock().await;
            let abs_path = if file_path.is_absolute() {
                file_path.clone()
            } else if let Some(ref root) = *root {
                root.join(file_path)
            } else {
                file_path.clone()
            };

            if let Ok(uri) = Url::from_file_path(&abs_path) {
                diagnostics_by_file.entry(uri).or_default().push(diagnostic);
            }
        }

        // Publish diagnostics for files with violations
        for (uri, diagnostics) in diagnostics_by_file {
            self.client
                .publish_diagnostics(uri, diagnostics, None)
                .await;
        }
    }
}

#[tower_lsp::async_trait]
impl LanguageServer for BoundaryLanguageServer {
    async fn initialize(&self, params: InitializeParams) -> Result<InitializeResult> {
        // Extract project root
        if let Some(root_uri) = params.root_uri {
            if let Ok(root_path) = root_uri.to_file_path() {
                self.initialize_pipeline(root_path).await;
            }
        } else if let Some(ref folders) = params.workspace_folders {
            if let Some(first) = folders.first() {
                if let Ok(root_path) = first.uri.to_file_path() {
                    self.initialize_pipeline(root_path).await;
                }
            }
        }

        Ok(InitializeResult {
            capabilities: ServerCapabilities {
                text_document_sync: Some(TextDocumentSyncCapability::Kind(
                    TextDocumentSyncKind::FULL,
                )),
                hover_provider: Some(HoverProviderCapability::Simple(true)),
                ..ServerCapabilities::default()
            },
            server_info: Some(ServerInfo {
                name: "boundary-lsp".to_string(),
                version: Some(env!("CARGO_PKG_VERSION").to_string()),
            }),
        })
    }

    async fn initialized(&self, _: InitializedParams) {
        self.client
            .log_message(MessageType::INFO, "Boundary LSP server initialized")
            .await;

        // Run initial analysis
        self.run_analysis_and_publish().await;
    }

    async fn shutdown(&self) -> Result<()> {
        Ok(())
    }

    async fn did_save(&self, _params: DidSaveTextDocumentParams) {
        // Re-analyze on save
        self.run_analysis_and_publish().await;
    }

    async fn did_change_configuration(&self, _params: DidChangeConfigurationParams) {
        // Reload config and re-analyze
        let root = self.project_root.lock().await.clone();
        if let Some(root) = root {
            self.initialize_pipeline(root).await;
            self.run_analysis_and_publish().await;
        }
    }

    async fn hover(&self, params: HoverParams) -> Result<Option<Hover>> {
        let analysis = self.last_analysis.lock().await;
        let Some(ref analysis) = *analysis else {
            return Ok(None);
        };

        let uri = &params.text_document_position_params.text_document.uri;
        let _position = &params.text_document_position_params.position;

        let file_path = uri.to_file_path().ok();
        let root = self.project_root.lock().await;

        // Try to match the hover position to a component
        for node in analysis.graph.nodes() {
            // Match by relative path and line
            if let (Some(ref file), Some(ref root)) = (&file_path, root.as_ref()) {
                let rel_path = file.strip_prefix(root).unwrap_or(file);
                let rel_str = rel_path.to_string_lossy();

                // Check if this node's ID contains the file's directory
                if node.id.0.contains(&*rel_str)
                    || rel_str.contains(node.id.0.split("::").next().unwrap_or_default())
                {
                    let layer_info = match node.layer {
                        Some(layer) => format!("**Layer:** {layer}"),
                        None => "**Layer:** unclassified".to_string(),
                    };

                    let content = format!("**{}** ({})\n\n{}", node.name, node.id.0, layer_info);

                    return Ok(Some(Hover {
                        contents: HoverContents::Markup(MarkupContent {
                            kind: MarkupKind::Markdown,
                            value: content,
                        }),
                        range: None,
                    }));
                }
            }
        }

        Ok(None)
    }
}
