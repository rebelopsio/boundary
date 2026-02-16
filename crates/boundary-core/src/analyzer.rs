use std::path::{Path, PathBuf};

use anyhow::Result;
use tree_sitter::Tree;

use crate::types::{Component, Dependency};

/// A parsed source file with its tree-sitter AST and original content.
pub struct ParsedFile {
    pub path: PathBuf,
    pub tree: Tree,
    pub content: String,
}

/// Trait that each language analyzer must implement.
pub trait LanguageAnalyzer: Send + Sync {
    /// Language name (e.g., "go", "rust")
    fn language(&self) -> &'static str;

    /// File extensions this analyzer handles (e.g., &["go"])
    fn file_extensions(&self) -> &[&str];

    /// Parse a source file into a ParsedFile.
    fn parse_file(&self, path: &Path, content: &str) -> Result<ParsedFile>;

    /// Extract architectural components from a parsed file.
    fn extract_components(&self, parsed: &ParsedFile) -> Vec<Component>;

    /// Extract dependencies (imports, type references, etc.) from a parsed file.
    fn extract_dependencies(&self, parsed: &ParsedFile) -> Vec<Dependency>;
}
