use std::collections::HashMap;
use std::path::Path;

use anyhow::{Context, Result};
use tree_sitter::{Language, Parser, Query, QueryCursor, StreamingIterator};

use boundary_core::analyzer::{LanguageAnalyzer, ParsedFile};
use boundary_core::types::*;

/// Extracted constructor signature for a `New*()` function.
///
/// Only `return_type` is consumed during classification. The remaining fields
/// (`function_name`, `inferred_struct`, `return_package`) are retained as
/// scaffolding for future diagnostic output (e.g., reporting the constructor
/// name when flagging a dependency inversion violation).
struct ConstructorSignature {
    #[allow(dead_code)]
    function_name: String,
    #[allow(dead_code)]
    inferred_struct: String,
    #[allow(dead_code)]
    return_package: String,
    return_type: String,
}

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
    constructor_query: Query,
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

        // Pattern 1: single qualified return   func New...() pkg.Type
        // Pattern 2: multi-return parameter list  func New...() (pkg.Type, error)
        // The `error` type is a plain type_identifier (no package qualifier) so the
        // qualified_type pattern skips it automatically, extracting only the port name.
        // NOTE: tree-sitter-go uses `name:` (not `type:`) for the type_identifier
        // field inside qualified_type.
        let constructor_query = Query::new(
            &language,
            r#"
            (function_declaration
              name: (identifier) @ctor_name
              result: (qualified_type
                package: (package_identifier) @return_pkg
                name: (type_identifier) @return_type))

            (function_declaration
              name: (identifier) @ctor_name
              result: (parameter_list
                (parameter_declaration
                  type: (qualified_type
                    package: (package_identifier) @return_pkg
                    name: (type_identifier) @return_type))))
            "#,
        )
        .context("failed to compile constructor query")?;

        Ok(Self {
            language,
            interface_query,
            struct_query,
            import_query,
            method_query,
            init_query,
            constructor_query,
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

        // Extract constructors BEFORE structs so classification can use return types
        let constructors = extract_constructors(&self.constructor_query, parsed);

        // Extract structs
        extract_structs(
            &self.struct_query,
            parsed,
            &pkg,
            &constructors,
            &mut components,
        );

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

fn extract_structs(
    query: &Query,
    parsed: &ParsedFile,
    pkg: &str,
    constructors: &HashMap<String, ConstructorSignature>,
    components: &mut Vec<Component>,
) {
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

        let kind =
            classify_struct_kind(&name, &fields, &parsed.path.to_string_lossy(), constructors);

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

/// Returns the PascalCase form of a Go unexported name by uppercasing the first character.
/// e.g. `mongoInvoiceRepository` → `MongoInvoiceRepository`
fn to_pascal_case(name: &str) -> String {
    let mut chars = name.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().chain(chars).collect(),
        None => String::new(),
    }
}

/// Infer the struct name from a constructor function name.
///
/// `"NewStripePaymentProcessor"` → `"stripePaymentProcessor"`
///
/// Returns an empty string for names that don't start with `"New"`.
fn infer_struct_from_constructor(ctor_name: &str) -> String {
    let without_new = match ctor_name.strip_prefix("New") {
        Some(s) if !s.is_empty() => s,
        _ => return String::new(),
    };
    let mut chars = without_new.chars();
    match chars.next() {
        Some(first) => first.to_lowercase().chain(chars).collect(),
        None => String::new(),
    }
}

/// Extract constructor signatures from a parsed file.
///
/// Returns a map keyed by **both** the inferred lowercase struct name
/// (`stripePaymentProcessor`) AND the PascalCase variant (`StripePaymentProcessor`).
/// Dual-indexing ensures that both exported and unexported structs can be looked
/// up via `constructors.get(name)` inside `classify_struct_kind`.
fn extract_constructors(
    query: &Query,
    parsed: &ParsedFile,
) -> HashMap<String, ConstructorSignature> {
    let mut result: HashMap<String, ConstructorSignature> = HashMap::new();

    let capture_names = query.capture_names();
    let ctor_name_idx = capture_names.iter().position(|n| *n == "ctor_name");
    let return_pkg_idx = capture_names.iter().position(|n| *n == "return_pkg");
    let return_type_idx = capture_names.iter().position(|n| *n == "return_type");

    let (Some(ctor_name_idx), Some(return_pkg_idx), Some(return_type_idx)) =
        (ctor_name_idx, return_pkg_idx, return_type_idx)
    else {
        return result;
    };

    let mut cursor = QueryCursor::new();
    let mut matches = cursor.matches(query, parsed.tree.root_node(), parsed.content.as_bytes());

    while let Some(m) = matches.next() {
        let mut ctor_name = String::new();
        let mut return_pkg = String::new();
        let mut return_type = String::new();

        for capture in m.captures {
            let idx = capture.index as usize;
            if idx == ctor_name_idx {
                ctor_name = node_text(capture.node, &parsed.content);
            } else if idx == return_pkg_idx {
                return_pkg = node_text(capture.node, &parsed.content);
            } else if idx == return_type_idx {
                return_type = node_text(capture.node, &parsed.content);
            }
        }

        // tree-sitter cannot filter by name prefix, so we filter to New* here.
        if !ctor_name.starts_with("New") || ctor_name.len() <= 3 {
            continue;
        }
        if return_pkg.is_empty() || return_type.is_empty() {
            continue;
        }

        let inferred = infer_struct_from_constructor(&ctor_name);
        let pascal = to_pascal_case(&inferred);

        // Dual-index: insert under the lowercase key (for unexported structs)
        // and the PascalCase key (for exported structs). Both entries are independent
        // clones — `inferred` and `pascal` are always different strings (one starts
        // lowercase, the other uppercase), so neither `or_insert` ever skips.
        result
            .entry(inferred.clone())
            .or_insert_with(|| ConstructorSignature {
                function_name: ctor_name.clone(),
                inferred_struct: inferred.clone(),
                return_package: return_pkg.clone(),
                return_type: return_type.clone(),
            });
        result
            .entry(pascal.clone())
            .or_insert_with(|| ConstructorSignature {
                function_name: ctor_name.clone(),
                inferred_struct: inferred.clone(),
                return_package: return_pkg.clone(),
                return_type: return_type.clone(),
            });
    }

    result
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

/// Classify a struct using name heuristics combined with file path and constructor context.
///
/// Infrastructure-layer checks run first. For unexported structs a constructor
/// lookup gates Adapter classification, and when found, populates `implements`
/// with the port interface name and sets confidence to High.
///
/// Exported structs in the infrastructure layer are only classified as adapters
/// when a constructor returning a port interface is found. There is intentionally
/// no suffix-based fallback — an exported struct without a port-returning
/// constructor is not an adapter and may indicate a dependency inversion violation.
///
/// Handler/Controller structs in the infrastructure layer are intentionally NOT
/// caught here — `pipeline::reclassify_infra_handlers` handles them after layer
/// assignment, which is the appropriate place for that post-processing step.
fn classify_struct_kind(
    name: &str,
    fields: &[FieldInfo],
    file_path: &str,
    constructors: &HashMap<String, ConstructorSignature>,
) -> ComponentKind {
    let lower = name.to_lowercase();

    // ── Infrastructure layer ──────────────────────────────────────────────────
    if file_path.contains("infrastructure/") {
        // Repository suffix — highest priority, checked before constructor lookup.
        if lower.ends_with("repository") || lower.ends_with("repo") {
            return ComponentKind::Repository;
        }

        // Constructor-based classification (High confidence).
        // Covers both unexported structs (looked up by lowercase name) and exported
        // structs (looked up by PascalCase name — dual-indexed in extract_constructors).
        if let Some(ctor) = constructors.get(name) {
            let port_name = ctor.return_type.clone();
            return if port_name.to_lowercase().ends_with("repository")
                || port_name.to_lowercase().ends_with("repo")
            {
                ComponentKind::Repository
            } else {
                ComponentKind::Adapter(AdapterInfo {
                    name: name.to_string(),
                    implements: vec![port_name],
                    confidence: AdapterConfidence::High,
                })
            };
        }

        // No fallback. Unexported structs without a port-returning constructor
        // are not classified as adapters — they fall through to other classification
        // logic. Exported structs without a constructor may represent a dependency
        // inversion violation (returning concrete type instead of port).
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
    fn test_unexported_infra_no_constructor_is_not_adapter() {
        let analyzer = GoAnalyzer::new().unwrap();
        // invoiceDocument is an unexported infrastructure struct with no New* constructor.
        // Without a constructor returning a port interface, it must NOT be classified as
        // an adapter — it falls through to other classification logic (e.g. ValueObject).
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
        let is_adapter = matches!(&doc.unwrap().kind, ComponentKind::Adapter(_));
        assert!(
            !is_adapter,
            "invoiceDocument with no port-returning constructor must NOT be classified as Adapter; got {:?}",
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

    // ── Phase 3: constructor-based adapter detection ──────────────────────────

    #[test]
    fn test_tree_sitter_constructor_query() {
        // Validates three constructor patterns against the compiled query:
        //   1. Single return:      func NewSingle() domain.PaymentProcessor
        //   2. Multi-return:       func NewMulti() (domain.Repository, error)
        //   3. Error-first multi:  func NewError() (error, domain.Adapter)
        //      → `error` is a plain type_identifier, not qualified_type; query skips it
        //        and still extracts "Adapter" from the second parameter.
        let analyzer = GoAnalyzer::new().unwrap();
        let content = r#"
package infrastructure

type singleImpl struct{}
type multiImpl struct{}
type errorImpl struct{}

func NewSingle() domain.PaymentProcessor {
    return &singleImpl{}
}

func NewMulti() (domain.Repository, error) {
    return &multiImpl{}, nil
}

func NewError() (error, domain.Adapter) {
    return nil, &errorImpl{}
}
"#;
        let path = PathBuf::from("internal/infrastructure/test.go");
        let parsed = analyzer.parse_file(&path, content).unwrap();
        let constructors = extract_constructors(&analyzer.constructor_query, &parsed);

        // NewSingle → inferred key "single" / "Single", return_type = "PaymentProcessor"
        let single = constructors
            .get("single")
            .or_else(|| constructors.get("Single"));
        assert!(
            single.is_some(),
            "NewSingle constructor not extracted; keys: {:?}",
            constructors.keys().collect::<Vec<_>>()
        );
        assert_eq!(single.unwrap().return_type, "PaymentProcessor");

        // NewMulti → inferred key "multi" / "Multi", return_type = "Repository"
        let multi = constructors
            .get("multi")
            .or_else(|| constructors.get("Multi"));
        assert!(multi.is_some(), "NewMulti constructor not extracted");
        assert_eq!(multi.unwrap().return_type, "Repository");

        // NewError → inferred key "error" / "Error", return_type = "Adapter"
        // (error plain-type parameter is skipped; port qualified_type is found)
        let err_ctor = constructors
            .get("error")
            .or_else(|| constructors.get("Error"));
        assert!(err_ctor.is_some(), "NewError constructor not extracted");
        assert_eq!(err_ctor.unwrap().return_type, "Adapter");
    }

    #[test]
    fn test_infer_struct_from_constructor() {
        assert_eq!(
            infer_struct_from_constructor("NewStripePaymentProcessor"),
            "stripePaymentProcessor"
        );
        assert_eq!(infer_struct_from_constructor("NewRepo"), "repo");
        assert_eq!(infer_struct_from_constructor("New"), ""); // nothing after "New"
        assert_eq!(infer_struct_from_constructor("Create"), ""); // no "New" prefix
        assert_eq!(infer_struct_from_constructor(""), "");
    }

    #[test]
    fn test_constructor_populates_implements() {
        let analyzer = GoAnalyzer::new().unwrap();
        let content = r#"
package infrastructure

type stripePaymentProcessor struct{ apiKey string }

func NewStripePaymentProcessor(apiKey string) domain.PaymentProcessor {
    return &stripePaymentProcessor{apiKey: apiKey}
}
"#;
        let path = PathBuf::from("internal/infrastructure/stripe/processor.go");
        let parsed = analyzer.parse_file(&path, content).unwrap();
        let components = analyzer.extract_components(&parsed);

        let adapter = components
            .iter()
            .find(|c| c.name == "stripePaymentProcessor");
        assert!(adapter.is_some(), "stripePaymentProcessor not found");
        match &adapter.unwrap().kind {
            ComponentKind::Adapter(info) => {
                assert_eq!(
                    info.confidence,
                    AdapterConfidence::High,
                    "expected High confidence"
                );
                assert!(
                    info.implements.contains(&"PaymentProcessor".to_string()),
                    "implements must contain 'PaymentProcessor'; got {:?}",
                    info.implements
                );
            }
            other => panic!("expected Adapter, got {:?}", other),
        }
    }

    #[test]
    fn test_exported_struct_with_port_constructor_is_adapter() {
        let analyzer = GoAnalyzer::new().unwrap();
        let content = r#"
package infrastructure

type CycleInfrastructureProvider struct{ client interface{} }

func NewCycleInfrastructureProvider() domain.InfrastructureProvider {
    return &CycleInfrastructureProvider{}
}
"#;
        let path = PathBuf::from("internal/infrastructure/cycle/provider.go");
        let parsed = analyzer.parse_file(&path, content).unwrap();
        let components = analyzer.extract_components(&parsed);

        let adapter = components
            .iter()
            .find(|c| c.name == "CycleInfrastructureProvider");
        assert!(adapter.is_some(), "CycleInfrastructureProvider not found");
        assert!(
            matches!(adapter.unwrap().kind, ComponentKind::Adapter(_)),
            "CycleInfrastructureProvider with port constructor must be Adapter; got {:?}",
            adapter.unwrap().kind
        );
    }

    #[test]
    fn test_constructor_with_multi_return_populates_implements() {
        let analyzer = GoAnalyzer::new().unwrap();
        let content = r#"
package infrastructure

type mailgunNotificationService struct{}

func NewMailgunNotificationService() (ports.NotificationService, error) {
    return &mailgunNotificationService{}, nil
}
"#;
        let path = PathBuf::from("internal/infrastructure/notification/service.go");
        let parsed = analyzer.parse_file(&path, content).unwrap();
        let components = analyzer.extract_components(&parsed);

        let adapter = components
            .iter()
            .find(|c| c.name == "mailgunNotificationService");
        assert!(adapter.is_some(), "mailgunNotificationService not found");
        match &adapter.unwrap().kind {
            ComponentKind::Adapter(info) => {
                assert_eq!(info.confidence, AdapterConfidence::High);
                assert!(
                    info.implements.contains(&"NotificationService".to_string()),
                    "implements must contain 'NotificationService'; got {:?}",
                    info.implements
                );
            }
            other => panic!("expected Adapter, got {:?}", other),
        }
    }

    #[test]
    fn test_constructor_repository_return_classifies_as_repository() {
        let analyzer = GoAnalyzer::new().unwrap();
        let content = r#"
package infrastructure

type postgresInvoiceStore struct{}

func NewPostgresInvoiceStore() ports.InvoiceRepository {
    return &postgresInvoiceStore{}
}
"#;
        let path = PathBuf::from("internal/infrastructure/postgres/invoice.go");
        let parsed = analyzer.parse_file(&path, content).unwrap();
        let components = analyzer.extract_components(&parsed);

        let repo = components.iter().find(|c| c.name == "postgresInvoiceStore");
        assert!(repo.is_some(), "postgresInvoiceStore not found");
        assert!(
            matches!(repo.unwrap().kind, ComponentKind::Repository),
            "constructor returning InvoiceRepository must yield Repository kind; got {:?}",
            repo.unwrap().kind
        );
    }
}
