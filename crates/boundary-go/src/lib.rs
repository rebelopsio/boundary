use std::collections::HashMap;
use std::path::Path;

use anyhow::{Context, Result};
use tree_sitter::{Language, Parser, Query, QueryCursor, StreamingIterator};

use boundary_core::analyzer::{LanguageAnalyzer, ParsedFile};
use boundary_core::types::*;

/// Active Record method name patterns.
/// If a struct has 2+ methods matching these names, it's treated as Active Record.
const ACTIVE_RECORD_METHODS: &[&str] = &[
    "Load", "Save", "Update", "Delete", "Insert", "Create", "FindByID", "FindBy", "Get", "GetAll",
    "List", "Upsert", "Remove", "Persist", "Fetch",
];

/// Go language analyzer using tree-sitter.
pub struct GoAnalyzer {
    language: Language,
    interface_query: Query,
    struct_query: Query,
    import_query: Query,
    method_query: Query,
    init_query: Query,
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
                    name: (field_identifier) @method_name
                    parameters: (parameter_list) @params
                    result: (_)? @return_type)*)))
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
                      name: (field_identifier) @field_name
                      type: (_) @field_type)*))))
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

        let method_query = Query::new(
            &language,
            r#"
            (method_declaration
              receiver: (parameter_list
                (parameter_declaration
                  type: [(pointer_type (type_identifier) @receiver_type)
                         (type_identifier) @receiver_type]))
              name: (field_identifier) @method_name
              parameters: (parameter_list) @params
              result: (_)? @return_type)
            "#,
        )
        .context("failed to compile method query")?;

        let init_query = Query::new(
            &language,
            r#"
            (function_declaration
              name: (identifier) @func_name
              body: (block) @body)
            "#,
        )
        .context("failed to compile init query")?;

        Ok(Self {
            language,
            interface_query,
            struct_query,
            import_query,
            method_query,
            init_query,
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

        // Extract methods and associate with receiver structs
        let methods = extract_methods(&self.method_query, parsed);
        associate_methods(&mut components, &methods);

        components
    }

    fn is_stdlib_import(&self, import_path: &str) -> bool {
        // Go stdlib imports never contain a dot (no domain name).
        // e.g., "fmt", "context", "encoding/json", "crypto/rand"
        // Third-party: "github.com/...", "golang.org/x/..."
        !import_path.contains('.')
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

        // Extract init() function dependencies
        let init_deps = extract_init_dependencies(&self.init_query, parsed, &pkg);
        deps.extend(init_deps);

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
    let method_name_idx = query
        .capture_names()
        .iter()
        .position(|n| *n == "method_name");
    let params_idx = query.capture_names().iter().position(|n| *n == "params");
    let return_type_idx = query
        .capture_names()
        .iter()
        .position(|n| *n == "return_type");

    let mut matches = cursor.matches(query, parsed.tree.root_node(), parsed.content.as_bytes());

    while let Some(m) = matches.next() {
        let mut name = String::new();
        let mut methods = Vec::new();
        let mut start_row = 0;
        let mut start_col = 0;

        // Collect method data from captures
        let mut current_method_name = String::new();
        let mut current_params = String::new();
        let mut current_return = String::new();

        for capture in m.captures {
            if capture.index as usize == name_idx {
                name = node_text(capture.node, &parsed.content);
                start_row = capture.node.start_position().row;
                start_col = capture.node.start_position().column;
            } else if Some(capture.index as usize) == method_name_idx {
                // Save previous method if any
                if !current_method_name.is_empty() {
                    methods.push(MethodInfo {
                        name: current_method_name.clone(),
                        parameters: current_params.clone(),
                        return_type: current_return.clone(),
                    });
                }
                current_method_name = node_text(capture.node, &parsed.content);
                current_params = String::new();
                current_return = String::new();
            } else if Some(capture.index as usize) == params_idx {
                current_params = node_text(capture.node, &parsed.content);
            } else if Some(capture.index as usize) == return_type_idx {
                current_return = node_text(capture.node, &parsed.content);
            }
        }

        // Save last method
        if !current_method_name.is_empty() {
            methods.push(MethodInfo {
                name: current_method_name,
                parameters: current_params,
                return_type: current_return,
            });
        }

        if name.is_empty() {
            continue;
        }
        // Unexported interfaces are intentionally skipped: they are internal contracts,
        // not domain ports. Only exported interfaces qualify as ports for scoring
        // purposes. (Note: unexported *structs* are included — see extract_structs.)
        if name.starts_with(|c: char| c.is_lowercase()) {
            continue;
        }

        components.push(Component {
            id: ComponentId::new(pkg, &name),
            name: name.clone(),
            kind: ComponentKind::Port(PortInfo { name, methods }),
            layer: None,
            location: SourceLocation {
                file: parsed.path.clone(),
                line: start_row + 1,
                column: start_col + 1,
            },
            is_cross_cutting: false,
            architecture_mode: ArchitectureMode::default(),
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
    let field_name_idx = query
        .capture_names()
        .iter()
        .position(|n| *n == "field_name");
    let field_type_idx = query
        .capture_names()
        .iter()
        .position(|n| *n == "field_type");

    let mut matches = cursor.matches(query, parsed.tree.root_node(), parsed.content.as_bytes());

    while let Some(m) = matches.next() {
        let mut name = String::new();
        let mut fields = Vec::new();
        let mut start_row = 0;
        let mut start_col = 0;

        let mut current_field_name = String::new();

        for capture in m.captures {
            if capture.index as usize == name_idx {
                name = node_text(capture.node, &parsed.content);
                start_row = capture.node.start_position().row;
                start_col = capture.node.start_position().column;
            } else if Some(capture.index as usize) == field_name_idx {
                current_field_name = node_text(capture.node, &parsed.content);
            } else if Some(capture.index as usize) == field_type_idx {
                let type_name = node_text(capture.node, &parsed.content);
                if !current_field_name.is_empty() {
                    fields.push(FieldInfo {
                        name: current_field_name.clone(),
                        type_name,
                    });
                    current_field_name = String::new();
                }
            }
        }

        if name.is_empty() {
            continue;
        }

        let kind = classify_struct_kind(
            &name,
            &fields,
            &parsed.path.to_string_lossy(),
            &parsed.content,
        );

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
            is_cross_cutting: false,
            architecture_mode: ArchitectureMode::default(),
        });
    }
}

/// Extract methods from method declarations and group by receiver type.
fn extract_methods(query: &Query, parsed: &ParsedFile) -> HashMap<String, Vec<MethodInfo>> {
    let mut methods: HashMap<String, Vec<MethodInfo>> = HashMap::new();

    let mut cursor = QueryCursor::new();
    let receiver_type_idx = query
        .capture_names()
        .iter()
        .position(|n| *n == "receiver_type");
    let method_name_idx = query
        .capture_names()
        .iter()
        .position(|n| *n == "method_name");
    let params_idx = query.capture_names().iter().position(|n| *n == "params");
    let return_type_idx = query
        .capture_names()
        .iter()
        .position(|n| *n == "return_type");

    let mut matches = cursor.matches(query, parsed.tree.root_node(), parsed.content.as_bytes());

    while let Some(m) = matches.next() {
        let mut receiver = String::new();
        let mut method_name = String::new();
        let mut params = String::new();
        let mut return_type = String::new();

        for capture in m.captures {
            if Some(capture.index as usize) == receiver_type_idx {
                receiver = node_text(capture.node, &parsed.content);
            } else if Some(capture.index as usize) == method_name_idx {
                method_name = node_text(capture.node, &parsed.content);
            } else if Some(capture.index as usize) == params_idx {
                params = node_text(capture.node, &parsed.content);
            } else if Some(capture.index as usize) == return_type_idx {
                return_type = node_text(capture.node, &parsed.content);
            }
        }

        if !receiver.is_empty() && !method_name.is_empty() {
            methods.entry(receiver).or_default().push(MethodInfo {
                name: method_name,
                parameters: params,
                return_type,
            });
        }
    }

    methods
}

/// Associate extracted methods with their receiver struct components.
///
/// After associating methods, entities with no methods are flagged as
/// `is_anemic_domain_model`. This must happen here (not during initial
/// classification) because methods are discovered in a separate tree-sitter
/// query and are not available when `classify_struct_kind` runs.
fn associate_methods(components: &mut [Component], methods: &HashMap<String, Vec<MethodInfo>>) {
    for component in components.iter_mut() {
        if let Some(struct_methods) = methods.get(&component.name) {
            match &mut component.kind {
                ComponentKind::Entity(info) => {
                    info.methods = struct_methods.clone();
                    info.is_active_record = is_active_record(&info.methods);
                }
                ComponentKind::DomainEvent(info) => {
                    // Domain events typically don't have methods, but store if found
                    let _ = info;
                }
                _ => {}
            }
        }

        // Flag entities that have an identity field but no domain methods.
        // An entity with zero methods after the association pass is anemic —
        // it holds data but delegates all behaviour to services.
        if let ComponentKind::Entity(info) = &mut component.kind {
            info.is_anemic_domain_model = info.methods.is_empty();
        }
    }
}

/// Check if a struct's methods indicate an Active Record pattern.
/// Returns true if 2+ methods match known CRUD/persistence method names.
fn is_active_record(methods: &[MethodInfo]) -> bool {
    methods
        .iter()
        .filter(|m| {
            ACTIVE_RECORD_METHODS
                .iter()
                .any(|ar| m.name == *ar || m.name.starts_with(ar))
        })
        .count()
        >= 2
}

/// Classify a struct using name heuristics combined with file path context.
///
/// Infrastructure-layer checks run first so that unexported concrete types and
/// explicitly-named adapters (Processor, Client, Gateway, Provider) are caught
/// before falling through to the generic domain-layer heuristics.
///
/// Handler/Controller structs in the infrastructure layer are intentionally NOT
/// caught here — `pipeline::reclassify_infra_handlers` handles them after layer
/// assignment, which is the appropriate place for that post-processing step.
/// Returns the PascalCase form of a Go unexported name by uppercasing the first character.
/// e.g. `mongoInvoiceRepository` → `MongoInvoiceRepository`
fn to_pascal_case(name: &str) -> String {
    let mut chars = name.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().chain(chars).collect(),
        None => String::new(),
    }
}

/// Returns true if the file contains a `func New<PascalName>(` declaration.
///
/// This is a fast text-search check (not a full parse) used to distinguish
/// real adapter structs — which follow the Go convention of pairing an
/// unexported type with an exported constructor — from internal utility types
/// such as DTO/document models that happen to be unexported.
fn has_constructor_for_struct(struct_name: &str, file_content: &str) -> bool {
    let pattern = format!("func New{}(", to_pascal_case(struct_name));
    file_content.contains(&pattern)
}

/// Returns true when a lowercase struct name looks like a past-tense domain event.
///
/// Covers two naming styles:
///   - `*event` suffix    — explicit (InvoiceCreatedEvent, PaymentSucceededEvent)
///   - past-tense suffix  — implicit (InvoiceFinalized, PaymentSucceeded)
///
/// The implicit list is intentionally conservative to avoid false positives on
/// domain model structs with past-participle adjectives (e.g. StoredPaymentMethod).
fn is_domain_event_name(lower: &str) -> bool {
    if lower.ends_with("event") {
        return true;
    }
    // Past-tense verb endings common in domain event naming conventions
    const PAST_TENSE_SUFFIXES: &[&str] = &[
        "created",
        "updated",
        "deleted",
        "finalized",
        "canceled",
        "cancelled",
        "succeeded",
        "failed",
        "paid",
        "voided",
        "refunded",
        "processed",
        "published",
        "dispatched",
        "completed",
        "expired",
        "activated",
        "deactivated",
        "closed",
        "opened",
        "recorded",
        "applied",
        "reversed",
        "rejected",
        "approved",
    ];
    PAST_TENSE_SUFFIXES.iter().any(|s| lower.ends_with(s))
}

/// Classify a struct using name heuristics combined with file path and content context.
///
/// Infrastructure-layer checks run first. For unexported structs a constructor
/// check (`func New<Name>(`) gates Adapter classification, filtering out internal
/// utility types (DTOs, document models) that have no exported constructor.
///
/// Handler/Controller structs in the infrastructure layer are intentionally NOT
/// caught here — `pipeline::reclassify_infra_handlers` handles them after layer
/// assignment, which is the appropriate place for that post-processing step.
fn classify_struct_kind(
    name: &str,
    fields: &[FieldInfo],
    file_path: &str,
    file_content: &str,
) -> ComponentKind {
    let lower = name.to_lowercase();

    // ── Infrastructure layer ──────────────────────────────────────────────────
    if file_path.contains("infrastructure/") {
        // Repository is the most specific subtype — check first.
        if lower.ends_with("repository") || lower.ends_with("repo") {
            return ComponentKind::Repository;
        }

        // Unexported concrete struct: only classify as Adapter when a matching
        // New<PascalName>() constructor exists in the same file. Without a
        // constructor the struct is an internal utility (DTO, document model,
        // etc.) and should not be counted as an adapter. Structs without a
        // constructor fall through to generic classification below.
        if name.starts_with(|c: char| c.is_lowercase())
            && has_constructor_for_struct(name, file_content)
        {
            return ComponentKind::Adapter(AdapterInfo {
                name: name.to_string(),
                implements: Vec::new(),
            });
        }

        // Exported struct with explicit adapter suffix.
        // "service" is included here because infrastructure-layer service structs
        // (e.g. MailGunNotificationService) are technology adapters, not domain
        // services — the infrastructure block takes precedence over the generic
        // service heuristic that applies to domain/application layers.
        if lower.ends_with("processor")
            || lower.ends_with("client")
            || lower.ends_with("gateway")
            || lower.ends_with("adapter")
            || lower.ends_with("provider")
            || lower.ends_with("publisher")
            || lower.ends_with("bus")
            || lower.ends_with("service")
        {
            return ComponentKind::Adapter(AdapterInfo {
                name: name.to_string(),
                implements: Vec::new(),
            });
        }
    }

    // ── Domain events (path-based, before generic heuristics) ────────────────
    // Structs in domain/events/ are events by definition, regardless of name.
    // For structs elsewhere, use a broad past-tense suffix list that covers
    // codebases that name events without an explicit "Event" suffix
    // (e.g. InvoiceFinalized, PaymentSucceeded).
    if file_path.contains("domain/events/") || is_domain_event_name(&lower) {
        return ComponentKind::DomainEvent(EventInfo {
            name: name.to_string(),
            fields: fields.to_vec(),
        });
    }

    // ── Generic name-based classification (layer-agnostic) ───────────────────
    if lower.ends_with("repository") || lower.ends_with("repo") {
        ComponentKind::Repository
    } else if lower.ends_with("service") || lower.ends_with("svc") {
        ComponentKind::Service
    } else if lower.ends_with("usecase") || lower.ends_with("interactor") {
        ComponentKind::UseCase
    } else if !fields.is_empty()
        && !fields.iter().any(|f| {
            let fl = f.name.to_lowercase();
            fl == "id" || fl == "uuid"
        })
    {
        // Value object heuristic: has fields but no identity field.
        ComponentKind::ValueObject
    } else {
        // is_anemic_domain_model is set to false here and updated after method
        // association in associate_methods, where method counts are available.
        ComponentKind::Entity(EntityInfo {
            name: name.to_string(),
            fields: fields.to_vec(),
            methods: Vec::new(),
            is_active_record: false,
            is_anemic_domain_model: false,
        })
    }
}

/// Extract dependencies from init() function bodies.
/// Walks the body of each init() function for qualified call expressions (pkg.Function).
fn extract_init_dependencies(query: &Query, parsed: &ParsedFile, pkg: &str) -> Vec<Dependency> {
    let mut deps = Vec::new();
    let mut cursor = QueryCursor::new();

    let func_name_idx = query
        .capture_names()
        .iter()
        .position(|n| *n == "func_name")
        .unwrap_or(0);
    let body_idx = query
        .capture_names()
        .iter()
        .position(|n| *n == "body")
        .unwrap_or(1);

    let mut matches = cursor.matches(query, parsed.tree.root_node(), parsed.content.as_bytes());

    while let Some(m) = matches.next() {
        let mut func_name = String::new();
        let mut body_node = None;

        for capture in m.captures {
            if capture.index as usize == func_name_idx {
                func_name = node_text(capture.node, &parsed.content);
            } else if capture.index as usize == body_idx {
                body_node = Some(capture.node);
            }
        }

        if func_name != "init" {
            continue;
        }

        let Some(body) = body_node else {
            continue;
        };

        let from_id = ComponentId::new(pkg, "<init>");

        // Walk the body tree for call_expression nodes with selector_expression
        let mut tree_cursor = body.walk();
        walk_for_calls(
            &mut tree_cursor,
            &parsed.content,
            &parsed.path,
            &from_id,
            &mut deps,
        );
    }

    deps
}

/// Recursively walk a tree-sitter node for qualified call expressions (pkg.Function).
fn walk_for_calls(
    cursor: &mut tree_sitter::TreeCursor,
    source: &str,
    file_path: &std::path::Path,
    from_id: &ComponentId,
    deps: &mut Vec<Dependency>,
) {
    loop {
        let node = cursor.node();

        if node.kind() == "call_expression" {
            // Check if the function is a selector_expression (pkg.Function)
            if let Some(func_node) = node.child_by_field_name("function") {
                if func_node.kind() == "selector_expression" {
                    if let Some(operand) = func_node.child_by_field_name("operand") {
                        let called_pkg = node_text(operand, source);
                        let to_id = ComponentId::new(&called_pkg, "<package>");
                        deps.push(Dependency {
                            from: from_id.clone(),
                            to: to_id,
                            kind: DependencyKind::MethodCall,
                            location: SourceLocation {
                                file: file_path.to_path_buf(),
                                line: node.start_position().row + 1,
                                column: node.start_position().column + 1,
                            },
                            import_path: Some(called_pkg),
                        });
                    }
                }
            }
        }

        // Recurse into children
        if cursor.goto_first_child() {
            walk_for_calls(cursor, source, file_path, from_id, deps);
            cursor.goto_parent();
        }

        if !cursor.goto_next_sibling() {
            break;
        }
    }
}

/// Extract text from a tree-sitter node.
fn node_text(node: tree_sitter::Node, source: &str) -> String {
    source[node.byte_range()].to_string()
}

/// Derive a package path from a file path.
/// e.g., "internal/domain/user/entity.go" -> "internal/domain/user"
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

        if let ComponentKind::Port(ref info) = interface.unwrap().kind {
            assert!(
                info.methods.iter().any(|m| m.name == "Save"),
                "should have Save method"
            );
            assert!(
                info.methods.iter().any(|m| m.name == "FindByID"),
                "should have FindByID method"
            );
        }

        let entity = components.iter().find(|c| c.name == "User");
        assert!(entity.is_some(), "should find User struct");
        if let ComponentKind::Entity(ref info) = entity.unwrap().kind {
            assert!(
                info.fields.iter().any(|f| f.name == "ID"),
                "should have ID field"
            );
            assert!(
                info.fields.iter().any(|f| f.name == "Name"),
                "should have Name field"
            );
        }
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

    #[test]
    fn test_domain_event_detection() {
        let analyzer = GoAnalyzer::new().unwrap();
        let content = r#"
package events

type PaymentSucceededEvent struct {
    PaymentID string
    Amount    float64
}
"#;
        let path = PathBuf::from("internal/domain/events/payment.go");
        let parsed = analyzer.parse_file(&path, content).unwrap();
        let components = analyzer.extract_components(&parsed);

        let event = components
            .iter()
            .find(|c| c.name == "PaymentSucceededEvent");
        assert!(event.is_some(), "should find PaymentSucceededEvent");
        assert!(
            matches!(event.unwrap().kind, ComponentKind::DomainEvent(_)),
            "should be classified as DomainEvent"
        );
    }

    #[test]
    fn test_domain_event_past_tense_suffix_no_event_word() {
        let analyzer = GoAnalyzer::new().unwrap();
        // InvoiceFinalized has no "Event" suffix but is past-tense — should be a DomainEvent.
        let content = r#"
package events

type InvoiceFinalized struct {
    InvoiceID string
    Amount    float64
}
"#;
        let path = PathBuf::from("internal/domain/events/invoice.go");
        let parsed = analyzer.parse_file(&path, content).unwrap();
        let components = analyzer.extract_components(&parsed);

        let event = components.iter().find(|c| c.name == "InvoiceFinalized");
        assert!(event.is_some(), "should find InvoiceFinalized");
        assert!(
            matches!(event.unwrap().kind, ComponentKind::DomainEvent(_)),
            "past-tense struct in domain/events/ must be DomainEvent; got {:?}",
            event.unwrap().kind
        );
    }

    #[test]
    fn test_anemic_entity_flagged_after_method_association() {
        let analyzer = GoAnalyzer::new().unwrap();
        // LineItem has an ID field but no methods — it is an anemic entity.
        let content = r#"
package models

type LineItem struct {
    ID       string
    Quantity int
    Price    float64
}
"#;
        let path = PathBuf::from("internal/domain/models/line_item.go");
        let parsed = analyzer.parse_file(&path, content).unwrap();
        let components = analyzer.extract_components(&parsed);

        let entity = components.iter().find(|c| c.name == "LineItem");
        assert!(entity.is_some(), "should find LineItem");
        if let ComponentKind::Entity(ref info) = entity.unwrap().kind {
            assert!(
                info.is_anemic_domain_model,
                "LineItem with ID but no methods must be flagged as anemic"
            );
        } else {
            panic!("expected Entity kind; got {:?}", entity.unwrap().kind);
        }
    }

    #[test]
    fn test_entity_with_methods_not_flagged_anemic() {
        let analyzer = GoAnalyzer::new().unwrap();
        // Invoice has an ID field AND methods — it is a rich entity, NOT anemic.
        let content = r#"
package models

type Invoice struct {
    ID     string
    Status string
}

func (i *Invoice) Finalize() error {
    return nil
}
"#;
        let path = PathBuf::from("internal/domain/models/invoice.go");
        let parsed = analyzer.parse_file(&path, content).unwrap();
        let components = analyzer.extract_components(&parsed);

        let entity = components.iter().find(|c| c.name == "Invoice");
        assert!(entity.is_some(), "should find Invoice");
        if let ComponentKind::Entity(ref info) = entity.unwrap().kind {
            assert!(
                !info.is_anemic_domain_model,
                "Invoice with methods must NOT be flagged as anemic"
            );
        } else {
            panic!("expected Entity kind; got {:?}", entity.unwrap().kind);
        }
    }

    #[test]
    fn test_value_object_detection() {
        let analyzer = GoAnalyzer::new().unwrap();
        let content = r#"
package domain

type Money struct {
    Amount   float64
    Currency string
}
"#;
        let path = PathBuf::from("internal/domain/money.go");
        let parsed = analyzer.parse_file(&path, content).unwrap();
        let components = analyzer.extract_components(&parsed);

        let vo = components.iter().find(|c| c.name == "Money");
        assert!(vo.is_some(), "should find Money");
        assert!(
            matches!(vo.unwrap().kind, ComponentKind::ValueObject),
            "should be classified as ValueObject (no ID field)"
        );
    }

    #[test]
    fn test_method_extraction() {
        let analyzer = GoAnalyzer::new().unwrap();
        let content = r#"
package user

type User struct {
    ID   string
    Name string
}

func (u *User) ChangeName(name string) error {
    u.Name = name
    return nil
}

func (u *User) Validate() error {
    return nil
}
"#;
        let path = PathBuf::from("internal/domain/user/entity.go");
        let parsed = analyzer.parse_file(&path, content).unwrap();
        let components = analyzer.extract_components(&parsed);

        let entity = components.iter().find(|c| c.name == "User");
        assert!(entity.is_some(), "should find User");
        if let ComponentKind::Entity(ref info) = entity.unwrap().kind {
            assert_eq!(info.methods.len(), 2, "should have 2 methods");
            assert!(
                info.methods.iter().any(|m| m.name == "ChangeName"),
                "should have ChangeName method"
            );
            assert!(
                info.methods.iter().any(|m| m.name == "Validate"),
                "should have Validate method"
            );
        } else {
            panic!("expected Entity kind");
        }
    }

    #[test]
    fn test_active_record_detection() {
        let analyzer = GoAnalyzer::new().unwrap();
        let content = r#"
package models

type User struct {
    ID   string
    Name string
}

func (u *User) Save() error {
    return nil
}

func (u *User) Delete() error {
    return nil
}

func (u *User) FindByID(id string) (*User, error) {
    return nil, nil
}
"#;
        let path = PathBuf::from("models/user.go");
        let parsed = analyzer.parse_file(&path, content).unwrap();
        let components = analyzer.extract_components(&parsed);

        let entity = components.iter().find(|c| c.name == "User");
        assert!(entity.is_some(), "should find User");
        if let ComponentKind::Entity(ref info) = entity.unwrap().kind {
            assert!(
                info.is_active_record,
                "User with Save, Delete, FindByID should be active record"
            );
        } else {
            panic!("expected Entity kind");
        }
    }

    #[test]
    fn test_not_active_record_with_few_crud_methods() {
        let analyzer = GoAnalyzer::new().unwrap();
        let content = r#"
package domain

type User struct {
    ID   string
    Name string
}

func (u *User) Validate() error {
    return nil
}
"#;
        let path = PathBuf::from("domain/user.go");
        let parsed = analyzer.parse_file(&path, content).unwrap();
        let components = analyzer.extract_components(&parsed);

        let entity = components.iter().find(|c| c.name == "User");
        assert!(entity.is_some());
        if let ComponentKind::Entity(ref info) = entity.unwrap().kind {
            assert!(
                !info.is_active_record,
                "User with only Validate should NOT be active record"
            );
        }
    }

    #[test]
    fn test_init_function_extraction() {
        let analyzer = GoAnalyzer::new().unwrap();
        let content = r#"
package main

import (
    "fmt"
    "myapp/internal/infrastructure/postgres"
)

func init() {
    postgres.Connect()
    fmt.Println("initialized")
}
"#;
        let path = PathBuf::from("cmd/main.go");
        let parsed = analyzer.parse_file(&path, content).unwrap();
        let deps = analyzer.extract_dependencies(&parsed);

        // Should have import deps + init deps
        let init_deps: Vec<_> = deps
            .iter()
            .filter(|d| d.from.0.contains("<init>"))
            .collect();
        assert!(
            !init_deps.is_empty(),
            "should extract dependencies from init() function"
        );
    }

    #[test]
    fn test_handler_struct_not_classified_as_adapter() {
        let analyzer = GoAnalyzer::new().unwrap();
        let content = r#"
package application

type UserHandler struct {
    ID   string
    Name string
}
"#;
        let path = PathBuf::from("internal/application/handler.go");
        let parsed = analyzer.parse_file(&path, content).unwrap();
        let components = analyzer.extract_components(&parsed);

        let handler = components.iter().find(|c| c.name == "UserHandler");
        assert!(handler.is_some(), "should find UserHandler");
        assert!(
            !matches!(handler.unwrap().kind, ComponentKind::Adapter(_)),
            "UserHandler must NOT be classified as Adapter; got {:?}",
            handler.unwrap().kind
        );
        // Positive assertion: UserHandler has an ID field so it classifies as Entity.
        assert!(
            matches!(handler.unwrap().kind, ComponentKind::Entity(_)),
            "UserHandler with ID/Name fields should be classified as Entity; got {:?}",
            handler.unwrap().kind
        );
    }

    #[test]
    fn test_unexported_repository_is_included() {
        let analyzer = GoAnalyzer::new().unwrap();
        // mongoRepo is classified by name suffix ("repo"), not field content —
        // the suffix match fires before the value-object heuristic is reached.
        let content = r#"
package infrastructure

type mongoRepo struct {
    client interface{}
}
"#;
        let path = PathBuf::from("internal/infrastructure/mongo_repo.go");
        let parsed = analyzer.parse_file(&path, content).unwrap();
        let components = analyzer.extract_components(&parsed);

        let repo = components.iter().find(|c| c.name == "mongoRepo");
        assert!(
            repo.is_some(),
            "unexported mongoRepo should be extracted as a real component"
        );
        assert!(
            matches!(repo.unwrap().kind, ComponentKind::Repository),
            "mongoRepo should be classified as Repository; got {:?}",
            repo.unwrap().kind
        );
    }

    #[test]
    fn test_unexported_infra_struct_with_constructor_is_adapter() {
        let analyzer = GoAnalyzer::new().unwrap();
        // stripePaymentProcessor is unexported but has a matching New* constructor —
        // it should be classified as Adapter.
        let content = r#"
package infrastructure

type stripePaymentProcessor struct {
    apiKey string
}

func NewStripePaymentProcessor(apiKey string) ports.PaymentProcessor {
    return &stripePaymentProcessor{apiKey: apiKey}
}
"#;
        let path = PathBuf::from("internal/infrastructure/stripe/processor.go");
        let parsed = analyzer.parse_file(&path, content).unwrap();
        let components = analyzer.extract_components(&parsed);

        let adapter = components
            .iter()
            .find(|c| c.name == "stripePaymentProcessor");
        assert!(
            adapter.is_some(),
            "stripePaymentProcessor should be extracted"
        );
        assert!(
            matches!(adapter.unwrap().kind, ComponentKind::Adapter(_)),
            "stripePaymentProcessor with New* constructor must be Adapter; got {:?}",
            adapter.unwrap().kind
        );
    }

    #[test]
    fn test_unexported_infra_struct_without_constructor_is_not_adapter() {
        let analyzer = GoAnalyzer::new().unwrap();
        // invoiceDocument is an unexported infrastructure utility (MongoDB document
        // model / DTO) with no New* constructor. It must NOT be classified as Adapter.
        let content = r#"
package infrastructure

type invoiceDocument struct {
    ID     string
    Status string
}
"#;
        let path = PathBuf::from("internal/infrastructure/mongodb/models.go");
        let parsed = analyzer.parse_file(&path, content).unwrap();
        let components = analyzer.extract_components(&parsed);

        let doc = components.iter().find(|c| c.name == "invoiceDocument");
        assert!(doc.is_some(), "invoiceDocument should be extracted");
        assert!(
            !matches!(doc.unwrap().kind, ComponentKind::Adapter(_)),
            "invoiceDocument with no constructor must NOT be Adapter; got {:?}",
            doc.unwrap().kind
        );
    }

    #[test]
    fn test_non_init_functions_not_extracted() {
        let analyzer = GoAnalyzer::new().unwrap();
        let content = r#"
package main

import "myapp/internal/infrastructure/postgres"

func setup() {
    postgres.Connect()
}
"#;
        let path = PathBuf::from("cmd/main.go");
        let parsed = analyzer.parse_file(&path, content).unwrap();
        let deps = analyzer.extract_dependencies(&parsed);

        let init_deps: Vec<_> = deps
            .iter()
            .filter(|d| d.from.0.contains("<init>"))
            .collect();
        assert!(
            init_deps.is_empty(),
            "non-init functions should not produce init dependencies"
        );
    }

    #[test]
    fn test_field_types() {
        let analyzer = GoAnalyzer::new().unwrap();
        let content = r#"
package user

type User struct {
    ID        string
    Name      string
    CreatedAt time.Time
}
"#;
        let path = PathBuf::from("internal/domain/user/entity.go");
        let parsed = analyzer.parse_file(&path, content).unwrap();
        let components = analyzer.extract_components(&parsed);

        let entity = components.iter().find(|c| c.name == "User");
        assert!(entity.is_some());
        if let ComponentKind::Entity(ref info) = entity.unwrap().kind {
            let id_field = info.fields.iter().find(|f| f.name == "ID");
            assert!(id_field.is_some());
            assert_eq!(id_field.unwrap().type_name, "string");
        }
    }
}
