use std::path::Path;

use anyhow::{Context, Result};
use tree_sitter::{Language, Parser, Query, QueryCursor, StreamingIterator};

use boundary_core::analyzer::{LanguageAnalyzer, ParsedFile};
use boundary_core::types::*;

/// Holds queries compiled for a specific TypeScript dialect.
struct QuerySet {
    interface_query: Query,
    type_alias_query: Query,
    class_query: Query,
    import_query: Query,
}

const INTERFACE_QUERY_SRC: &str = r#"
(interface_declaration
  name: (type_identifier) @name
  body: (interface_body) @body)
"#;

const TYPE_ALIAS_QUERY_SRC: &str = r#"
(type_alias_declaration
  name: (type_identifier) @name
  value: (object_type))
"#;

const CLASS_QUERY_SRC: &str = r#"
(class_declaration
  name: (type_identifier) @name
  (class_heritage
    (implements_clause
      (type_identifier) @implements))?
  body: (class_body))
"#;

const IMPORT_QUERY_SRC: &str = r#"
(import_statement
  source: (string) @path)
"#;

fn compile_queries(language: &Language) -> Result<QuerySet> {
    Ok(QuerySet {
        interface_query: Query::new(language, INTERFACE_QUERY_SRC)
            .context("failed to compile interface query")?,
        type_alias_query: Query::new(language, TYPE_ALIAS_QUERY_SRC)
            .context("failed to compile type alias query")?,
        class_query: Query::new(language, CLASS_QUERY_SRC)
            .context("failed to compile class query")?,
        import_query: Query::new(language, IMPORT_QUERY_SRC)
            .context("failed to compile import query")?,
    })
}

/// TypeScript/TSX language analyzer using tree-sitter.
pub struct TypeScriptAnalyzer {
    ts_language: Language,
    tsx_language: Language,
    ts_queries: QuerySet,
    tsx_queries: QuerySet,
}

impl TypeScriptAnalyzer {
    pub fn new() -> Result<Self> {
        let ts_language: Language = tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into();
        let tsx_language: Language = tree_sitter_typescript::LANGUAGE_TSX.into();

        let ts_queries = compile_queries(&ts_language)?;
        let tsx_queries = compile_queries(&tsx_language)?;

        Ok(Self {
            ts_language,
            tsx_language,
            ts_queries,
            tsx_queries,
        })
    }

    fn language_for_file(&self, path: &Path) -> &Language {
        match path.extension().and_then(|e| e.to_str()) {
            Some("tsx") => &self.tsx_language,
            _ => &self.ts_language,
        }
    }

    fn queries_for_file(&self, path: &Path) -> &QuerySet {
        match path.extension().and_then(|e| e.to_str()) {
            Some("tsx") => &self.tsx_queries,
            _ => &self.ts_queries,
        }
    }
}

impl LanguageAnalyzer for TypeScriptAnalyzer {
    fn language(&self) -> &'static str {
        "typescript"
    }

    fn file_extensions(&self) -> &[&str] {
        &["ts", "tsx"]
    }

    fn parse_file(&self, path: &Path, content: &str) -> Result<ParsedFile> {
        let language = self.language_for_file(path);
        let mut parser = Parser::new();
        parser
            .set_language(language)
            .context("failed to set TypeScript language")?;
        let tree = parser
            .parse(content, None)
            .context("failed to parse TypeScript file")?;
        Ok(ParsedFile {
            path: path.to_path_buf(),
            tree,
            content: content.to_string(),
        })
    }

    fn extract_components(&self, parsed: &ParsedFile) -> Vec<Component> {
        let mut components = Vec::new();
        let module_path = derive_module_path(&parsed.path);

        // Skip .d.ts declaration files
        if parsed.path.to_string_lossy().ends_with(".d.ts") {
            return components;
        }

        let queries = self.queries_for_file(&parsed.path);
        extract_interfaces(
            &queries.interface_query,
            parsed,
            &module_path,
            &mut components,
        );
        extract_type_aliases(
            &queries.type_alias_query,
            parsed,
            &module_path,
            &mut components,
        );
        extract_classes(&queries.class_query, parsed, &module_path, &mut components);

        components
    }

    fn extract_dependencies(&self, parsed: &ParsedFile) -> Vec<Dependency> {
        let mut deps = Vec::new();
        let module_path = derive_module_path(&parsed.path);
        let from_id = ComponentId::new(&module_path, "<file>");

        let queries = self.queries_for_file(&parsed.path);
        let mut cursor = QueryCursor::new();
        let path_idx = queries
            .import_query
            .capture_names()
            .iter()
            .position(|n| *n == "path")
            .unwrap_or(0);

        let mut matches = cursor.matches(
            &queries.import_query,
            parsed.tree.root_node(),
            parsed.content.as_bytes(),
        );

        while let Some(m) = matches.next() {
            for capture in m.captures {
                if capture.index as usize == path_idx {
                    let node = capture.node;
                    let raw = node_text(node, &parsed.content);
                    // Strip quotes (single or double)
                    let import_path = raw.trim_matches('"').trim_matches('\'').to_string();
                    let to_id = ComponentId::new(&import_path, "<module>");

                    deps.push(Dependency {
                        from: from_id.clone(),
                        to: to_id,
                        kind: DependencyKind::Import,
                        location: SourceLocation {
                            file: parsed.path.clone(),
                            line: node.start_position().row + 1,
                            column: node.start_position().column + 1,
                        },
                        import_path: Some(import_path),
                    });
                }
            }
        }

        deps
    }
}

fn extract_interfaces(
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
    let body_idx = query.capture_names().iter().position(|n| *n == "body");

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
            } else if Some(capture.index as usize) == body_idx {
                // Walk child nodes of the interface body to find method signatures
                let body_node = capture.node;
                let mut child_cursor = body_node.walk();
                if child_cursor.goto_first_child() {
                    loop {
                        let child = child_cursor.node();
                        if child.kind() == "method_signature" {
                            if let Some(name_node) = child.child_by_field_name("name") {
                                methods.push(MethodInfo {
                                    name: node_text(name_node, &parsed.content),
                                    parameters: String::new(),
                                    return_type: String::new(),
                                });
                            }
                        }
                        if !child_cursor.goto_next_sibling() {
                            break;
                        }
                    }
                }
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
            is_cross_cutting: false,
        });
    }
}

fn extract_type_aliases(
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

    let mut matches = cursor.matches(query, parsed.tree.root_node(), parsed.content.as_bytes());

    while let Some(m) = matches.next() {
        for capture in m.captures {
            if capture.index as usize == name_idx {
                let name = node_text(capture.node, &parsed.content);
                if name.is_empty() {
                    continue;
                }

                components.push(Component {
                    id: ComponentId::new(module_path, &name),
                    name: name.clone(),
                    kind: ComponentKind::Port(PortInfo {
                        name,
                        methods: vec![],
                    }),
                    layer: None,
                    location: SourceLocation {
                        file: parsed.path.clone(),
                        line: capture.node.start_position().row + 1,
                        column: capture.node.start_position().column + 1,
                    },
                    is_cross_cutting: false,
                });
            }
        }
    }
}

fn extract_classes(
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
    let implements_idx = query
        .capture_names()
        .iter()
        .position(|n| *n == "implements");

    let mut matches = cursor.matches(query, parsed.tree.root_node(), parsed.content.as_bytes());

    while let Some(m) = matches.next() {
        let mut name = String::new();
        let mut implements = Vec::new();
        let mut start_row = 0;
        let mut start_col = 0;

        for capture in m.captures {
            if capture.index as usize == name_idx {
                name = node_text(capture.node, &parsed.content);
                start_row = capture.node.start_position().row;
                start_col = capture.node.start_position().column;
            } else if Some(capture.index as usize) == implements_idx {
                implements.push(node_text(capture.node, &parsed.content));
            }
        }

        if name.is_empty() {
            continue;
        }

        let kind = classify_class_kind(&name, &implements);

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
            is_cross_cutting: false,
        });
    }
}

/// Classify a class by its name suffix heuristic and implements clause.
fn classify_class_kind(name: &str, implements: &[String]) -> ComponentKind {
    let lower = name.to_lowercase();
    if lower.ends_with("repository") || lower.ends_with("repo") {
        ComponentKind::Repository
    } else if lower.ends_with("service") || lower.ends_with("svc") {
        ComponentKind::Service
    } else if lower.ends_with("handler") || lower.ends_with("controller") {
        ComponentKind::Adapter(AdapterInfo {
            name: name.to_string(),
            implements: implements.to_vec(),
        })
    } else if lower.ends_with("usecase") || lower.ends_with("interactor") {
        ComponentKind::UseCase
    } else if !implements.is_empty() {
        ComponentKind::Adapter(AdapterInfo {
            name: name.to_string(),
            implements: implements.to_vec(),
        })
    } else {
        ComponentKind::Entity(EntityInfo {
            name: name.to_string(),
            fields: vec![],
            methods: Vec::new(),
        })
    }
}

/// Extract text from a tree-sitter node.
fn node_text(node: tree_sitter::Node, source: &str) -> String {
    source[node.byte_range()].to_string()
}

/// Derive a module path from a file path.
fn derive_module_path(path: &Path) -> String {
    path.parent()
        .map(|p| p.to_string_lossy().replace('\\', "/"))
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_parse_typescript_interface() {
        let analyzer = TypeScriptAnalyzer::new().unwrap();
        let content = r#"
export interface UserRepository {
    save(user: User): Promise<void>;
    findById(id: string): Promise<User | null>;
}

export interface User {
    id: string;
    name: string;
    email: string;
}
"#;
        let path = PathBuf::from("src/domain/user/user.ts");
        let parsed = analyzer.parse_file(&path, content).unwrap();
        let components = analyzer.extract_components(&parsed);

        assert!(
            components.len() >= 2,
            "expected at least 2 components, got {}",
            components.len()
        );

        let repo = components.iter().find(|c| c.name == "UserRepository");
        assert!(repo.is_some(), "should find UserRepository interface");
        assert!(matches!(repo.unwrap().kind, ComponentKind::Port(_)));

        if let ComponentKind::Port(ref info) = repo.unwrap().kind {
            assert!(info.methods.iter().any(|m| m.name == "save"));
            assert!(info.methods.iter().any(|m| m.name == "findById"));
        }
    }

    #[test]
    fn test_extract_class_with_implements() {
        let analyzer = TypeScriptAnalyzer::new().unwrap();
        let content = r#"
export class PostgresUserRepository implements UserRepository {
    constructor(private pool: Pool) {}

    async save(user: User): Promise<void> {
        // save
    }

    async findById(id: string): Promise<User | null> {
        return null;
    }
}
"#;
        let path = PathBuf::from("src/infrastructure/postgres/user-repo.ts");
        let parsed = analyzer.parse_file(&path, content).unwrap();
        let components = analyzer.extract_components(&parsed);

        let repo = components
            .iter()
            .find(|c| c.name == "PostgresUserRepository");
        assert!(repo.is_some(), "should find PostgresUserRepository");

        match &repo.unwrap().kind {
            ComponentKind::Repository => {} // classified by name
            ComponentKind::Adapter(info) => {
                assert!(info.implements.contains(&"UserRepository".to_string()));
            }
            other => panic!("expected Repository or Adapter, got {:?}", other),
        }
    }

    #[test]
    fn test_extract_imports() {
        let analyzer = TypeScriptAnalyzer::new().unwrap();
        let content = r#"
import { User } from '../domain/user/user';
import { UserRepository } from '../domain/user/user-repository';
import { Pool } from 'pg';
"#;
        let path = PathBuf::from("src/infrastructure/postgres/user-repo.ts");
        let parsed = analyzer.parse_file(&path, content).unwrap();
        let deps = analyzer.extract_dependencies(&parsed);

        assert_eq!(deps.len(), 3, "expected 3 imports");
        let paths: Vec<&str> = deps
            .iter()
            .filter_map(|d| d.import_path.as_deref())
            .collect();
        assert!(paths.contains(&"../domain/user/user"));
        assert!(paths.contains(&"../domain/user/user-repository"));
        assert!(paths.contains(&"pg"));
    }

    #[test]
    fn test_parse_tsx_file() {
        let analyzer = TypeScriptAnalyzer::new().unwrap();
        let content = r#"
import React from 'react';

interface Props {
    name: string;
}

export class UserHandler {
    render() {
        return "Hello";
    }
}
"#;
        let path = PathBuf::from("src/presentation/user.tsx");
        let parsed = analyzer.parse_file(&path, content).unwrap();
        let components = analyzer.extract_components(&parsed);
        assert!(!components.is_empty(), "should extract components from TSX");

        // Should find the interface
        let props = components.iter().find(|c| c.name == "Props");
        assert!(props.is_some(), "should find Props interface in TSX");
    }

    #[test]
    fn test_struct_classification() {
        let analyzer = TypeScriptAnalyzer::new().unwrap();
        let content = r#"
export class UserService {
    constructor(private repo: UserRepository) {}
}

export class UserHandler {
    constructor(private service: UserService) {}
}

export class CreateUserUseCase {
    constructor(private repo: UserRepository) {}
}
"#;
        let path = PathBuf::from("src/app.ts");
        let parsed = analyzer.parse_file(&path, content).unwrap();
        let components = analyzer.extract_components(&parsed);

        let svc = components.iter().find(|c| c.name == "UserService");
        assert!(matches!(svc.unwrap().kind, ComponentKind::Service));

        let handler = components.iter().find(|c| c.name == "UserHandler");
        assert!(matches!(handler.unwrap().kind, ComponentKind::Adapter(_)));

        let uc = components.iter().find(|c| c.name == "CreateUserUseCase");
        assert!(matches!(uc.unwrap().kind, ComponentKind::UseCase));
    }

    #[test]
    fn test_type_alias_port() {
        let analyzer = TypeScriptAnalyzer::new().unwrap();
        let content = r#"
export type UserPort = {
    save(user: User): Promise<void>;
    findById(id: string): Promise<User>;
};
"#;
        let path = PathBuf::from("src/domain/user/ports.ts");
        let parsed = analyzer.parse_file(&path, content).unwrap();
        let components = analyzer.extract_components(&parsed);

        let port = components.iter().find(|c| c.name == "UserPort");
        assert!(port.is_some(), "should find UserPort type alias");
        assert!(matches!(port.unwrap().kind, ComponentKind::Port(_)));
    }
}
