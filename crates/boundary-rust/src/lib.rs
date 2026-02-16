use std::path::Path;

use anyhow::{Context, Result};
use tree_sitter::{Language, Parser, Query, QueryCursor, StreamingIterator};

use boundary_core::analyzer::{LanguageAnalyzer, ParsedFile};
use boundary_core::types::*;

/// Rust language analyzer using tree-sitter.
pub struct RustAnalyzer {
    language: Language,
    trait_query: Query,
    struct_query: Query,
    impl_query: Query,
    use_query: Query,
}

impl RustAnalyzer {
    pub fn new() -> Result<Self> {
        let language: Language = tree_sitter_rust::LANGUAGE.into();

        let trait_query = Query::new(
            &language,
            r#"
            (trait_item
              name: (type_identifier) @name
              body: (declaration_list
                (function_signature_item
                  name: (identifier) @method)*))
            "#,
        )
        .context("failed to compile trait query")?;

        let struct_query = Query::new(
            &language,
            r#"
            (struct_item
              name: (type_identifier) @name
              body: (field_declaration_list
                (field_declaration
                  name: (field_identifier) @field)*)?)
            "#,
        )
        .context("failed to compile struct query")?;

        let impl_query = Query::new(
            &language,
            r#"
            (impl_item
              trait: (type_identifier)? @trait_name
              type: (type_identifier) @type_name)
            "#,
        )
        .context("failed to compile impl query")?;

        let use_query = Query::new(
            &language,
            r#"
            (use_declaration
              argument: (_) @path)
            "#,
        )
        .context("failed to compile use query")?;

        Ok(Self {
            language,
            trait_query,
            struct_query,
            impl_query,
            use_query,
        })
    }
}

impl LanguageAnalyzer for RustAnalyzer {
    fn language(&self) -> &'static str {
        "rust"
    }

    fn file_extensions(&self) -> &[&str] {
        &["rs"]
    }

    fn parse_file(&self, path: &Path, content: &str) -> Result<ParsedFile> {
        let mut parser = Parser::new();
        parser
            .set_language(&self.language)
            .context("failed to set Rust language")?;
        let tree = parser
            .parse(content, None)
            .context("failed to parse Rust file")?;
        Ok(ParsedFile {
            path: path.to_path_buf(),
            tree,
            content: content.to_string(),
        })
    }

    fn extract_components(&self, parsed: &ParsedFile) -> Vec<Component> {
        let mut components = Vec::new();
        let module_path = derive_module_path(&parsed.path);

        // Extract traits (ports)
        extract_traits(&self.trait_query, parsed, &module_path, &mut components);

        // Extract structs
        extract_structs(&self.struct_query, parsed, &module_path, &mut components);

        // Enrich structs with impl info (adapter classification)
        enrich_with_impls(&self.impl_query, parsed, &module_path, &mut components);

        components
    }

    fn extract_dependencies(&self, parsed: &ParsedFile) -> Vec<Dependency> {
        let mut deps = Vec::new();
        let module_path = derive_module_path(&parsed.path);
        let from_id = ComponentId::new(&module_path, "<file>");

        let mut cursor = QueryCursor::new();
        let path_idx = self
            .use_query
            .capture_names()
            .iter()
            .position(|n| *n == "path")
            .unwrap_or(0);

        let mut matches = cursor.matches(
            &self.use_query,
            parsed.tree.root_node(),
            parsed.content.as_bytes(),
        );

        while let Some(m) = matches.next() {
            for capture in m.captures {
                if capture.index as usize == path_idx {
                    let node = capture.node;
                    let use_path = node_text(node, &parsed.content);

                    // Skip std library imports
                    if use_path.starts_with("std::") || use_path.starts_with("core::") {
                        continue;
                    }

                    let to_id = ComponentId::new(&use_path, "<module>");

                    deps.push(Dependency {
                        from: from_id.clone(),
                        to: to_id,
                        kind: DependencyKind::Import,
                        location: SourceLocation {
                            file: parsed.path.clone(),
                            line: node.start_position().row + 1,
                            column: node.start_position().column + 1,
                        },
                        import_path: Some(use_path),
                    });
                }
            }
        }

        deps
    }
}

fn extract_traits(
    query: &Query,
    parsed: &ParsedFile,
    module_path: &str,
    components: &mut Vec<Component>,
) {
    let mut cursor = QueryCursor::new();
    let name_idx = query
        .capture_names()
        .iter()
        .position(|n| *n == "name")
        .unwrap_or(0);
    let method_idx = query.capture_names().iter().position(|n| *n == "method");

    let mut matches = cursor.matches(query, parsed.tree.root_node(), parsed.content.as_bytes());

    while let Some(m) = matches.next() {
        let mut name = String::new();
        let mut methods = Vec::new();
        let mut start_row = 0;
        let mut start_col = 0;

        for capture in m.captures {
            if capture.index as usize == name_idx {
                name = node_text(capture.node, &parsed.content);
                start_row = capture.node.start_position().row;
                start_col = capture.node.start_position().column;
            } else if Some(capture.index as usize) == method_idx {
                methods.push(node_text(capture.node, &parsed.content));
            }
        }

        if name.is_empty() {
            continue;
        }

        components.push(Component {
            id: ComponentId::new(module_path, &name),
            name: name.clone(),
            kind: ComponentKind::Port(PortInfo { name, methods }),
            layer: None,
            location: SourceLocation {
                file: parsed.path.clone(),
                line: start_row + 1,
                column: start_col + 1,
            },
        });
    }
}

fn extract_structs(
    query: &Query,
    parsed: &ParsedFile,
    module_path: &str,
    components: &mut Vec<Component>,
) {
    let mut cursor = QueryCursor::new();
    let name_idx = query
        .capture_names()
        .iter()
        .position(|n| *n == "name")
        .unwrap_or(0);
    let field_idx = query.capture_names().iter().position(|n| *n == "field");

    let mut matches = cursor.matches(query, parsed.tree.root_node(), parsed.content.as_bytes());

    while let Some(m) = matches.next() {
        let mut name = String::new();
        let mut fields = Vec::new();
        let mut start_row = 0;
        let mut start_col = 0;

        for capture in m.captures {
            if capture.index as usize == name_idx {
                name = node_text(capture.node, &parsed.content);
                start_row = capture.node.start_position().row;
                start_col = capture.node.start_position().column;
            } else if Some(capture.index as usize) == field_idx {
                fields.push(node_text(capture.node, &parsed.content));
            }
        }

        if name.is_empty() {
            continue;
        }

        let kind = classify_struct_kind(&name, &fields);

        components.push(Component {
            id: ComponentId::new(module_path, &name),
            name: name.clone(),
            kind,
            layer: None,
            location: SourceLocation {
                file: parsed.path.clone(),
                line: start_row + 1,
                column: start_col + 1,
            },
        });
    }
}

/// Scan impl blocks and upgrade matching structs to Adapter when they implement a trait.
fn enrich_with_impls(
    query: &Query,
    parsed: &ParsedFile,
    module_path: &str,
    components: &mut [Component],
) {
    let mut cursor = QueryCursor::new();
    let trait_name_idx = query
        .capture_names()
        .iter()
        .position(|n| *n == "trait_name");
    let type_name_idx = query
        .capture_names()
        .iter()
        .position(|n| *n == "type_name")
        .unwrap_or(0);

    let mut matches = cursor.matches(query, parsed.tree.root_node(), parsed.content.as_bytes());

    while let Some(m) = matches.next() {
        let mut trait_name: Option<String> = None;
        let mut type_name = String::new();

        for capture in m.captures {
            if Some(capture.index as usize) == trait_name_idx {
                trait_name = Some(node_text(capture.node, &parsed.content));
            }
            if capture.index as usize == type_name_idx {
                type_name = node_text(capture.node, &parsed.content);
            }
        }

        if type_name.is_empty() {
            continue;
        }

        // If this impl has a trait, mark the struct as an Adapter
        if let Some(ref trait_name) = trait_name {
            let id = ComponentId::new(module_path, &type_name);
            if let Some(comp) = components.iter_mut().find(|c| c.id == id) {
                match &mut comp.kind {
                    ComponentKind::Adapter(info) => {
                        if !info.implements.contains(trait_name) {
                            info.implements.push(trait_name.clone());
                        }
                    }
                    _ => {
                        comp.kind = ComponentKind::Adapter(AdapterInfo {
                            name: type_name.clone(),
                            implements: vec![trait_name.clone()],
                        });
                    }
                }
            }
        }
    }
}

/// Classify a struct by its name suffix heuristic (same as Go analyzer).
fn classify_struct_kind(name: &str, fields: &[String]) -> ComponentKind {
    let lower = name.to_lowercase();
    if lower.ends_with("repository") || lower.ends_with("repo") {
        ComponentKind::Repository
    } else if lower.ends_with("service") || lower.ends_with("svc") {
        ComponentKind::Service
    } else if lower.ends_with("handler") || lower.ends_with("controller") {
        ComponentKind::Adapter(AdapterInfo {
            name: name.to_string(),
            implements: Vec::new(),
        })
    } else if lower.ends_with("usecase") || lower.ends_with("interactor") {
        ComponentKind::UseCase
    } else {
        ComponentKind::Entity(EntityInfo {
            name: name.to_string(),
            fields: fields.to_vec(),
        })
    }
}

/// Extract text from a tree-sitter node.
fn node_text(node: tree_sitter::Node, source: &str) -> String {
    source[node.byte_range()].to_string()
}

/// Derive a module path from a file path.
/// e.g., "src/domain/user/mod.rs" → "src/domain/user"
fn derive_module_path(path: &Path) -> String {
    let path_str = path.to_string_lossy().replace('\\', "/");
    // Remove filename, keeping just the directory
    if let Some(parent) = path.parent() {
        parent.to_string_lossy().replace('\\', "/")
    } else {
        path_str
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_parse_simple_rust_file() {
        let analyzer = RustAnalyzer::new().unwrap();
        let content = r#"
pub trait UserRepository {
    fn save(&self, user: &User) -> Result<(), Error>;
    fn find_by_id(&self, id: &str) -> Result<User, Error>;
}

pub struct User {
    pub id: String,
    pub name: String,
}
"#;
        let path = PathBuf::from("src/domain/user/mod.rs");
        let parsed = analyzer.parse_file(&path, content).unwrap();
        let components = analyzer.extract_components(&parsed);

        assert!(
            components.len() >= 2,
            "expected at least 2 components, got {}",
            components.len()
        );

        let trait_comp = components.iter().find(|c| c.name == "UserRepository");
        assert!(trait_comp.is_some(), "should find UserRepository trait");
        assert!(matches!(trait_comp.unwrap().kind, ComponentKind::Port(_)));

        if let ComponentKind::Port(ref info) = trait_comp.unwrap().kind {
            assert!(info.methods.contains(&"save".to_string()));
            assert!(info.methods.contains(&"find_by_id".to_string()));
        }

        let entity = components.iter().find(|c| c.name == "User");
        assert!(entity.is_some(), "should find User struct");
    }

    #[test]
    fn test_extract_use_statements() {
        let analyzer = RustAnalyzer::new().unwrap();
        let content = r#"
use std::collections::HashMap;
use crate::domain::user::User;
use crate::infrastructure::postgres::PostgresRepo;
"#;
        let path = PathBuf::from("src/application/user_service.rs");
        let parsed = analyzer.parse_file(&path, content).unwrap();
        let deps = analyzer.extract_dependencies(&parsed);

        // Should skip std imports
        let paths: Vec<&str> = deps
            .iter()
            .filter_map(|d| d.import_path.as_deref())
            .collect();
        assert!(!paths.iter().any(|p| p.starts_with("std::")));
        assert!(paths.iter().any(|p| p.contains("domain::user::User")));
        assert!(paths
            .iter()
            .any(|p| p.contains("infrastructure::postgres::PostgresRepo")));
    }

    #[test]
    fn test_struct_classification() {
        let analyzer = RustAnalyzer::new().unwrap();
        let content = r#"
pub struct PostgresUserRepository {
    pool: Pool,
}

pub struct UserService {
    repo: Box<dyn UserRepository>,
}

pub struct HttpHandler {
    service: UserService,
}

pub struct CreateUserUseCase {
    repo: Box<dyn UserRepository>,
}
"#;
        let path = PathBuf::from("src/lib.rs");
        let parsed = analyzer.parse_file(&path, content).unwrap();
        let components = analyzer.extract_components(&parsed);

        let repo = components
            .iter()
            .find(|c| c.name == "PostgresUserRepository");
        assert!(matches!(repo.unwrap().kind, ComponentKind::Repository));

        let svc = components.iter().find(|c| c.name == "UserService");
        assert!(matches!(svc.unwrap().kind, ComponentKind::Service));

        let handler = components.iter().find(|c| c.name == "HttpHandler");
        assert!(matches!(handler.unwrap().kind, ComponentKind::Adapter(_)));

        let uc = components.iter().find(|c| c.name == "CreateUserUseCase");
        assert!(matches!(uc.unwrap().kind, ComponentKind::UseCase));
    }

    #[test]
    fn test_impl_trait_enrichment() {
        let analyzer = RustAnalyzer::new().unwrap();
        let content = r#"
pub trait UserRepository {
    fn save(&self, user: &User);
}

pub struct PostgresRepo {
    pool: Pool,
}

impl UserRepository for PostgresRepo {
    fn save(&self, user: &User) {}
}
"#;
        let path = PathBuf::from("src/infrastructure/postgres.rs");
        let parsed = analyzer.parse_file(&path, content).unwrap();
        let components = analyzer.extract_components(&parsed);

        let repo = components.iter().find(|c| c.name == "PostgresRepo");
        assert!(repo.is_some(), "should find PostgresRepo");
        match &repo.unwrap().kind {
            ComponentKind::Adapter(info) => {
                assert!(
                    info.implements.contains(&"UserRepository".to_string()),
                    "should track implemented trait"
                );
            }
            other => panic!("expected Adapter, got {:?}", other),
        }
    }
}
