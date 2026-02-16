use std::path::Path;

use anyhow::{Context, Result};
use tree_sitter::{Language, Parser, Query, QueryCursor, StreamingIterator};

use boundary_core::analyzer::{LanguageAnalyzer, ParsedFile};
use boundary_core::types::*;

/// Go language analyzer using tree-sitter.
pub struct GoAnalyzer {
    language: Language,
    interface_query: Query,
    struct_query: Query,
    import_query: Query,
    _func_query: Query,
}

impl GoAnalyzer {
    pub fn new() -> Result<Self> {
        let language: Language = tree_sitter_go::LANGUAGE.into();

        let interface_query = Query::new(
            &language,
            r#"
            (type_declaration
              (type_spec
                name: (type_identifier) @name
                type: (interface_type
                  (method_elem
                    name: (field_identifier) @method)*)))
            "#,
        )
        .context("failed to compile interface query")?;

        let struct_query = Query::new(
            &language,
            r#"
            (type_declaration
              (type_spec
                name: (type_identifier) @name
                type: (struct_type
                  (field_declaration_list
                    (field_declaration
                      name: (field_identifier) @field)*))))
            "#,
        )
        .context("failed to compile struct query")?;

        let import_query = Query::new(
            &language,
            r#"
            (import_spec
              path: (interpreted_string_literal) @path)
            "#,
        )
        .context("failed to compile import query")?;

        let func_query = Query::new(
            &language,
            r#"
            (function_declaration
              name: (identifier) @name)
            "#,
        )
        .context("failed to compile function query")?;

        Ok(Self {
            language,
            interface_query,
            struct_query,
            import_query,
            _func_query: func_query,
        })
    }
}

impl LanguageAnalyzer for GoAnalyzer {
    fn language(&self) -> &'static str {
        "go"
    }

    fn file_extensions(&self) -> &[&str] {
        &["go"]
    }

    fn parse_file(&self, path: &Path, content: &str) -> Result<ParsedFile> {
        let mut parser = Parser::new();
        parser
            .set_language(&self.language)
            .context("failed to set Go language")?;
        let tree = parser
            .parse(content, None)
            .context("failed to parse Go file")?;
        Ok(ParsedFile {
            path: path.to_path_buf(),
            tree,
            content: content.to_string(),
        })
    }

    fn extract_components(&self, parsed: &ParsedFile) -> Vec<Component> {
        let mut components = Vec::new();
        let pkg = derive_package_path(&parsed.path);

        // Extract interfaces (ports)
        extract_interfaces(&self.interface_query, parsed, &pkg, &mut components);

        // Extract structs
        extract_structs(&self.struct_query, parsed, &pkg, &mut components);

        components
    }

    fn extract_dependencies(&self, parsed: &ParsedFile) -> Vec<Dependency> {
        let mut deps = Vec::new();
        let pkg = derive_package_path(&parsed.path);
        let from_id = ComponentId::new(&pkg, "<file>");

        let mut cursor = QueryCursor::new();
        let path_idx = self
            .import_query
            .capture_names()
            .iter()
            .position(|n| *n == "path")
            .unwrap_or(0);

        let mut matches = cursor.matches(
            &self.import_query,
            parsed.tree.root_node(),
            parsed.content.as_bytes(),
        );

        while let Some(m) = matches.next() {
            for capture in m.captures {
                if capture.index as usize == path_idx {
                    let node = capture.node;
                    let raw = node_text(node, &parsed.content);
                    // Strip quotes from import path
                    let import_path = raw.trim_matches('"').to_string();
                    let to_id = ComponentId::new(&import_path, "<package>");

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
    pkg: &str,
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
            id: ComponentId::new(pkg, &name),
            name: name.clone(),
            kind: ComponentKind::Port(PortInfo { name, methods }),
            layer: None, // Will be classified later by LayerClassifier
            location: SourceLocation {
                file: parsed.path.clone(),
                line: start_row + 1,
                column: start_col + 1,
            },
        });
    }
}

fn extract_structs(query: &Query, parsed: &ParsedFile, pkg: &str, components: &mut Vec<Component>) {
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
            id: ComponentId::new(pkg, &name),
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

/// Classify a struct by its name suffix heuristic.
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

/// Derive a package path from a file path.
/// e.g., "internal/domain/user/entity.go" → "internal/domain/user"
fn derive_package_path(path: &Path) -> String {
    path.parent()
        .map(|p| p.to_string_lossy().replace('\\', "/"))
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_parse_simple_go_file() {
        let analyzer = GoAnalyzer::new().unwrap();
        let content = r#"
package user

type UserRepository interface {
    Save(user *User) error
    FindByID(id string) (*User, error)
}

type User struct {
    ID   string
    Name string
}
"#;
        let path = PathBuf::from("internal/domain/user/entity.go");
        let parsed = analyzer.parse_file(&path, content).unwrap();
        let components = analyzer.extract_components(&parsed);

        assert!(
            components.len() >= 2,
            "expected at least 2 components, got {}",
            components.len()
        );

        let interface = components.iter().find(|c| c.name == "UserRepository");
        assert!(interface.is_some(), "should find UserRepository interface");
        assert!(matches!(interface.unwrap().kind, ComponentKind::Port(_)));

        let entity = components.iter().find(|c| c.name == "User");
        assert!(entity.is_some(), "should find User struct");
    }

    #[test]
    fn test_extract_imports() {
        let analyzer = GoAnalyzer::new().unwrap();
        let content = r#"
package user

import (
    "fmt"
    "github.com/example/app/internal/infrastructure/postgres"
)

func main() {
    fmt.Println("hello")
}
"#;
        let path = PathBuf::from("internal/domain/user/service.go");
        let parsed = analyzer.parse_file(&path, content).unwrap();
        let deps = analyzer.extract_dependencies(&parsed);

        assert_eq!(deps.len(), 2, "expected 2 imports");
        let paths: Vec<&str> = deps
            .iter()
            .filter_map(|d| d.import_path.as_deref())
            .collect();
        assert!(paths.contains(&"fmt"));
        assert!(paths.contains(&"github.com/example/app/internal/infrastructure/postgres"));
    }
}
