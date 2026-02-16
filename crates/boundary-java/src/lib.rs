use std::path::Path;

use anyhow::{Context, Result};
use tree_sitter::{Language, Parser, Query, QueryCursor, StreamingIterator};

use boundary_core::analyzer::{LanguageAnalyzer, ParsedFile};
use boundary_core::types::*;

/// Java language analyzer using tree-sitter.
pub struct JavaAnalyzer {
    language: Language,
    interface_query: Query,
    class_query: Query,
    import_query: Query,
    annotation_query: Query,
}

impl JavaAnalyzer {
    pub fn new() -> Result<Self> {
        let language: Language = tree_sitter_java::LANGUAGE.into();

        let interface_query = Query::new(
            &language,
            r#"
            (interface_declaration
              name: (identifier) @name
              body: (interface_body
                (method_declaration
                  name: (identifier) @method)*))
            "#,
        )
        .context("failed to compile interface query")?;

        let class_query = Query::new(
            &language,
            r#"
            (class_declaration
              name: (identifier) @name
              interfaces: (super_interfaces
                (type_list
                  (type_identifier) @implements))?
              body: (class_body))
            "#,
        )
        .context("failed to compile class query")?;

        let import_query = Query::new(
            &language,
            r#"
            (import_declaration
              (scoped_identifier) @path)
            "#,
        )
        .context("failed to compile import query")?;

        // Annotation on class declarations for classification hints
        let annotation_query = Query::new(
            &language,
            r#"
            (class_declaration
              (modifiers
                (marker_annotation
                  name: (identifier) @annotation))
              name: (identifier) @class_name)
            "#,
        )
        .context("failed to compile annotation query")?;

        Ok(Self {
            language,
            interface_query,
            class_query,
            import_query,
            annotation_query,
        })
    }
}

impl LanguageAnalyzer for JavaAnalyzer {
    fn language(&self) -> &'static str {
        "java"
    }

    fn file_extensions(&self) -> &[&str] {
        &["java"]
    }

    fn parse_file(&self, path: &Path, content: &str) -> Result<ParsedFile> {
        let mut parser = Parser::new();
        parser
            .set_language(&self.language)
            .context("failed to set Java language")?;
        let tree = parser
            .parse(content, None)
            .context("failed to parse Java file")?;
        Ok(ParsedFile {
            path: path.to_path_buf(),
            tree,
            content: content.to_string(),
        })
    }

    fn extract_components(&self, parsed: &ParsedFile) -> Vec<Component> {
        let mut components = Vec::new();
        let package_path = derive_package_path(&parsed.path);

        // Extract interfaces (ports)
        extract_interfaces(
            &self.interface_query,
            parsed,
            &package_path,
            &mut components,
        );

        // Extract classes
        extract_classes(&self.class_query, parsed, &package_path, &mut components);

        // Enrich with annotation info
        enrich_with_annotations(
            &self.annotation_query,
            parsed,
            &package_path,
            &mut components,
        );

        components
    }

    fn extract_dependencies(&self, parsed: &ParsedFile) -> Vec<Dependency> {
        let mut deps = Vec::new();
        let package_path = derive_package_path(&parsed.path);
        let from_id = ComponentId::new(&package_path, "<file>");

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
                    let import_path = node_text(node, &parsed.content);

                    // Skip java.lang.* and standard library
                    if import_path.starts_with("java.") || import_path.starts_with("javax.") {
                        continue;
                    }

                    let to_id = ComponentId::new(&import_path, "<class>");

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
    package_path: &str,
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
            id: ComponentId::new(package_path, &name),
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

fn extract_classes(
    query: &Query,
    parsed: &ParsedFile,
    package_path: &str,
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
            id: ComponentId::new(package_path, &name),
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

/// Enrich class components with annotation-based classification.
fn enrich_with_annotations(
    query: &Query,
    parsed: &ParsedFile,
    package_path: &str,
    components: &mut [Component],
) {
    let mut cursor = QueryCursor::new();
    let annotation_idx = query
        .capture_names()
        .iter()
        .position(|n| *n == "annotation");
    let class_name_idx = query
        .capture_names()
        .iter()
        .position(|n| *n == "class_name");

    let mut matches = cursor.matches(query, parsed.tree.root_node(), parsed.content.as_bytes());

    while let Some(m) = matches.next() {
        let mut annotation = String::new();
        let mut class_name = String::new();

        for capture in m.captures {
            if Some(capture.index as usize) == annotation_idx {
                annotation = node_text(capture.node, &parsed.content);
            }
            if Some(capture.index as usize) == class_name_idx {
                class_name = node_text(capture.node, &parsed.content);
            }
        }

        if class_name.is_empty() || annotation.is_empty() {
            continue;
        }

        let id = ComponentId::new(package_path, &class_name);
        if let Some(comp) = components.iter_mut().find(|c| c.id == id) {
            match annotation.as_str() {
                "Repository" => {
                    comp.kind = ComponentKind::Repository;
                }
                "Service" => {
                    comp.kind = ComponentKind::Service;
                }
                "Controller" | "RestController" => {
                    comp.kind = ComponentKind::Adapter(AdapterInfo {
                        name: class_name,
                        implements: vec![],
                    });
                }
                _ => {}
            }
        }
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
        })
    }
}

/// Extract text from a tree-sitter node.
fn node_text(node: tree_sitter::Node, source: &str) -> String {
    source[node.byte_range()].to_string()
}

/// Derive a package path from a file path.
/// e.g., "src/main/java/com/example/domain/user/User.java" → "src/main/java/com/example/domain/user"
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
    fn test_parse_java_interface() {
        let analyzer = JavaAnalyzer::new().unwrap();
        let content = r#"
package com.example.domain.user;

public interface UserRepository {
    void save(User user);
    User findById(String id);
}
"#;
        let path = PathBuf::from("src/main/java/com/example/domain/user/UserRepository.java");
        let parsed = analyzer.parse_file(&path, content).unwrap();
        let components = analyzer.extract_components(&parsed);

        let repo = components.iter().find(|c| c.name == "UserRepository");
        assert!(repo.is_some(), "should find UserRepository interface");
        assert!(matches!(repo.unwrap().kind, ComponentKind::Port(_)));

        if let ComponentKind::Port(ref info) = repo.unwrap().kind {
            assert!(info.methods.contains(&"save".to_string()));
            assert!(info.methods.contains(&"findById".to_string()));
        }
    }

    #[test]
    fn test_parse_java_class_with_implements() {
        let analyzer = JavaAnalyzer::new().unwrap();
        let content = r#"
package com.example.infrastructure.postgres;

public class PostgresUserRepository implements UserRepository {
    private final DataSource dataSource;

    public PostgresUserRepository(DataSource dataSource) {
        this.dataSource = dataSource;
    }

    public void save(User user) {
        // save implementation
    }

    public User findById(String id) {
        return null;
    }
}
"#;
        let path = PathBuf::from(
            "src/main/java/com/example/infrastructure/postgres/PostgresUserRepository.java",
        );
        let parsed = analyzer.parse_file(&path, content).unwrap();
        let components = analyzer.extract_components(&parsed);

        let repo = components
            .iter()
            .find(|c| c.name == "PostgresUserRepository");
        assert!(repo.is_some(), "should find PostgresUserRepository");
        // Name-based classification should match "Repository"
        assert!(matches!(repo.unwrap().kind, ComponentKind::Repository));
    }

    #[test]
    fn test_extract_imports() {
        let analyzer = JavaAnalyzer::new().unwrap();
        let content = r#"
package com.example.application;

import java.util.List;
import com.example.domain.user.User;
import com.example.domain.user.UserRepository;
"#;
        let path = PathBuf::from("src/main/java/com/example/application/UserService.java");
        let parsed = analyzer.parse_file(&path, content).unwrap();
        let deps = analyzer.extract_dependencies(&parsed);

        // Should skip java.* imports
        let paths: Vec<&str> = deps
            .iter()
            .filter_map(|d| d.import_path.as_deref())
            .collect();
        assert!(!paths.iter().any(|p| p.starts_with("java.")));
        assert!(paths.iter().any(|p| p.contains("domain.user.User")));
        assert!(paths
            .iter()
            .any(|p| p.contains("domain.user.UserRepository")));
    }

    #[test]
    fn test_annotation_classification() {
        let analyzer = JavaAnalyzer::new().unwrap();
        let content = r#"
package com.example.application;

@Service
public class UserService {
    private final UserRepository repo;

    public UserService(UserRepository repo) {
        this.repo = repo;
    }
}
"#;
        let path = PathBuf::from("src/main/java/com/example/application/UserService.java");
        let parsed = analyzer.parse_file(&path, content).unwrap();
        let components = analyzer.extract_components(&parsed);

        let svc = components.iter().find(|c| c.name == "UserService");
        assert!(svc.is_some(), "should find UserService");
        assert!(
            matches!(svc.unwrap().kind, ComponentKind::Service),
            "should be classified as Service by annotation"
        );
    }

    #[test]
    fn test_controller_annotation() {
        let analyzer = JavaAnalyzer::new().unwrap();
        let content = r#"
package com.example.presentation;

@Controller
public class UserController {
    public void getUser() {}
}
"#;
        let path = PathBuf::from("src/main/java/com/example/presentation/UserController.java");
        let parsed = analyzer.parse_file(&path, content).unwrap();
        let components = analyzer.extract_components(&parsed);

        let ctrl = components.iter().find(|c| c.name == "UserController");
        assert!(ctrl.is_some(), "should find UserController");
        assert!(
            matches!(ctrl.unwrap().kind, ComponentKind::Adapter(_)),
            "should be classified as Adapter by @Controller annotation"
        );
    }

    #[test]
    fn test_entity_class() {
        let analyzer = JavaAnalyzer::new().unwrap();
        let content = r#"
package com.example.domain.user;

public class User {
    private String id;
    private String name;
    private String email;
}
"#;
        let path = PathBuf::from("src/main/java/com/example/domain/user/User.java");
        let parsed = analyzer.parse_file(&path, content).unwrap();
        let components = analyzer.extract_components(&parsed);

        let user = components.iter().find(|c| c.name == "User");
        assert!(user.is_some(), "should find User");
        assert!(matches!(user.unwrap().kind, ComponentKind::Entity(_)));
    }
}
