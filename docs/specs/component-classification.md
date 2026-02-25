# Component Classification Spec

**Version:** 1.1
**Purpose:** Define unambiguous rules for classifying Go code components into DDD/Hexagonal patterns
**Scope:** Automated static analysis (no runtime information)

---

## 1. Conceptual Definitions

### 1.1 Port (Interface)

**DDD Definition:** An interface that defines how the domain interacts with external systems or
how external systems interact with the domain.

**Characteristics:**

- Always an **interface** (not a struct)
- Defines operations without implementation
- Located in domain layer (typically `domain/ports/`)
- Named for capability, not technology (`PaymentProcessor`, not `StripeClient`)

**Two types:**

- **Inbound ports:** Application service interfaces (use cases)
- **Outbound ports:** Infrastructure dependencies (repositories, external services)

---

### 1.2 Adapter (Implementation)

**DDD Definition:** A concrete implementation that adapts an external system to conform to a
port interface.

**Characteristics:**

- Implements a port interface
- Located in infrastructure layer
- Contains technology-specific code (MongoDB, HTTP, Stripe)
- Translates between domain concepts and external system concepts

**Canonical Go pattern:**

```go
// Unexported struct (implementation detail)
type mongoInvoiceRepository struct { ... }

// Constructor returns port interface
func NewMongoInvoiceRepository(...) ports.InvoiceRepository {
    return &mongoInvoiceRepository{...}
}
```

---

### 1.3 Repository

**DDD Definition:** A specialized adapter that provides collection-like access to aggregates,
abstracting persistence.

**Characteristics:**

- Implements a repository port interface
- Methods follow CRUD patterns: `Save`, `Find`, `Delete`, `List`
- Returns domain entities/aggregates (not DTOs or database models)
- Located in infrastructure layer
- Is-a Adapter, but more specific — counted separately in metrics

**Pattern:**

```go
type InvoiceRepository interface {
    Save(ctx context.Context, invoice *Invoice) error
    FindByID(ctx context.Context, id string) (*Invoice, error)
    FindByCustomer(ctx context.Context, customerID string) ([]*Invoice, error)
    Delete(ctx context.Context, id string) error
}
```

---

### 1.4 Domain Event

**DDD Definition:** An immutable record of something significant that happened in the domain.

**Characteristics:**

- Struct (not interface)
- Named in past tense (`InvoiceFinalized`, not `FinalizeInvoice`)
- Located in domain layer (`domain/events/`)
- Contains event data (what happened, when, to which aggregate)
- Typically embeds base event fields (ID, timestamp, version)

**Pattern:**

```go
type InvoiceFinalizedEvent struct {
    EventID     string
    OccurredAt  time.Time
    InvoiceID   string
    CustomerID  string
    TotalAmount int64
}
```

---

### 1.5 Service

**DDD Definition:** Operations that don't naturally belong to an entity or value object.

**Two types:**

**Domain Service:**

- Contains domain logic spanning multiple aggregates
- Pure domain concepts (no infrastructure)
- Located in domain layer

**Application Service:**

- Orchestrates domain operations
- Implements use cases
- Coordinates between domain and infrastructure
- Located in application layer

**Pattern:**

```go
// Domain service
type PricingService struct {}
func (s *PricingService) CalculateDiscount(customer, product) Money

// Application service
type InvoiceService struct {
    invoiceRepo ports.InvoiceRepository
    eventBus    ports.EventPublisher
}
func (s *InvoiceService) FinalizeInvoice(ctx context.Context, invoiceID string) error
```

---

### 1.6 Entity

**DDD Definition:** An object with a distinct identity that persists over time, even as its
attributes change.

**Characteristics:**

- Has an identity field (ID)
- Has methods (behavior)
- Mutable state
- Equality based on ID, not attributes
- Located in domain layer (`domain/models/`)

**Pattern:**

```go
type Invoice struct {
    ID         string  // ← Identity
    CustomerID string
    Status     InvoiceStatus
}

func (i *Invoice) Finalize(invoiceNumber string) error {
    // Business logic
}
```

---

### 1.7 Value Object

**DDD Definition:** An immutable object whose identity is defined by its attributes, not an ID.

**Characteristics:**

- No identity field (no ID)
- Immutable (no setters, methods return new instances)
- Equality based on attributes
- Located in domain layer
- Often small and focused (Money, Email, Address)

**Pattern:**

```go
type Money struct {
    Amount   int64
    Currency string
}

func (m Money) Add(other Money) (Money, error) {
    // Returns new instance, doesn't mutate
}
```

---

## 2. Detection Rules (Go-Specific)

### 2.1 Port Detection

**Primary Rule:** Interface in domain/ports directory

```
IF:
  - Node is interface (not struct)
  - File path matches: **/domain/ports/**
THEN:
  ComponentKind = Port
  Confidence = HIGH
```

**Secondary Rule:** Interface with port-like naming in domain layer

```
IF:
  - Node is interface
  - Layer = Domain
  - Name ends with: Repository | Processor | Provider | Publisher | Gateway
THEN:
  ComponentKind = Port
  Confidence = MEDIUM
```

Note: `Service` is intentionally excluded from the secondary rule. An interface named
`InvoiceService` is ambiguous — it may be an application service contract or a domain service
interface. The disambiguation rule below applies:

```
IF:
  - Node is interface
  - Name ends with: Service
  - Layer = Domain
THEN:
  IF file path contains "domain/ports/"  → Port (MEDIUM confidence)
  IF file path contains "domain/services/" → Service/Domain (MEDIUM confidence)
  ELSE → Unclassified (needs human review)
```

**Test Cases:**

```
✅ domain/ports/invoice_repository.go:InvoiceRepository (interface) → Port
✅ domain/ports/payment_processor.go:PaymentProcessor (interface) → Port
❌ application/invoice_service.go:InvoiceService (struct) → NOT Port
❌ infrastructure/mongodb/repository.go:mongoRepository (struct) → NOT Port
```

---

### 2.2 Adapter Detection

Detection proceeds through three rules in priority order. Repository detection (Section 2.3)
runs before adapter detection — see Section 3 for the full precedence order.

**Primary Rule:** Constructor returns port interface

```
IF:
  - Function name starts with: New
  - Return type is qualified: <pkg>.<Interface>
  - Return type matches a known port
  - File path matches: **/infrastructure/**
THEN:
  ComponentKind = Adapter(port_name)   [or Repository if port_name ends with "Repository"]
  Confidence = HIGH
```

See Section 2.2.1 for the two-pass analysis model required to implement this rule.

**Secondary Rule:** Unexported struct in infrastructure

```
IF:
  - Node is struct
  - First character is lowercase (unexported)
  - File path matches: **/infrastructure/**
THEN:
  ComponentKind = Adapter(inferred_port_name)
  Confidence = MEDIUM

WHERE inferred_port_name:
  - Strip known technology prefix: mongo | stripe | mailgun | cycle | inmemory | async | memory
  - Title-case first letter
  - Example: mongoInvoiceRepository → InvoiceRepository
```

**Tertiary Rule:** Explicit adapter naming (exported struct)

```
IF:
  - Node is struct
  - File path matches: **/infrastructure/**
  - Name ends with: Adapter | Client | Gateway | Processor | Provider
THEN:
  ComponentKind = Adapter(name)
  Confidence = LOW
```

**Exclusion Rule:** Application-layer handlers are orchestrators, not adapters

```
IF:
  - File path matches: **/application/**
  - Name ends with: Handler | Controller
THEN:
  ComponentKind = Service (NOT Adapter)
```

Note: Infrastructure-layer `*Handler` and `*Controller` structs (driving/primary adapters such
as HTTP handlers wired at the infrastructure boundary) are upgraded to `Adapter` in a
post-processing pass after layer assignment, not during initial classification. This is because
`classify_struct_kind` operates before layers are assigned.

**Test Cases:**

```
✅ infrastructure/mongodb/invoice_repository.go:
   func NewMongoInvoiceRepository() ports.InvoiceRepository
   → Repository(InvoiceRepository) [constructor returns repository port]

✅ infrastructure/mongodb/invoice_repository.go:
   type mongoInvoiceRepository struct
   → Repository(InvoiceRepository) [unexported struct + ends with "repository"]

✅ infrastructure/stripe/payment_processor.go:
   type stripePaymentProcessor struct
   → Adapter(PaymentProcessor) [unexported struct in infrastructure]

⚠️  infrastructure/stripe/webhook_handler.go:
   type WebhookHandler struct (exported, no constructor returning port)
   → Adapter(inferred) BUT missing-port violation flagged
   [classified via post-processing reclassification, no matching port found]

❌ application/payment_handler.go:
   type PaymentHandler struct
   → Service (NOT Adapter) [exclusion rule: application-layer handler]
```

Legend: ✅ correct classification  ⚠️ classified with warning  ❌ explicitly not this kind

---

#### 2.2.1 Two-Pass Analysis Model for Constructor Detection

Constructor detection requires file-level context because the constructor is a separate
top-level function, not part of the struct declaration.

```rust
// PASS 1: Per-file extraction
struct FileContext {
    package_name: String,
    file_path: String,
    interfaces: Vec<InterfaceNode>,
    structs: Vec<StructNode>,
    constructors: Vec<ConstructorSignature>,
}

struct ConstructorSignature {
    function_name: String,   // "NewMongoInvoiceRepository"
    inferred_struct: String, // "mongoInvoiceRepository" (strip "New", lowercase first)
    return_package: String,  // "ports"
    return_type: String,     // "InvoiceRepository"
    file_path: String,
}

fn infer_struct_from_constructor(ctor_name: &str) -> String {
    // "NewMongoInvoiceRepository" → "mongoInvoiceRepository"
    let without_new = ctor_name.strip_prefix("New").unwrap();
    let mut chars = without_new.chars();
    match chars.next() {
        Some(first) => first.to_lowercase().chain(chars).collect(),
        None => String::new(),
    }
}
```

```rust
// PASS 2: Per-struct classification
fn classify_struct(
    struct_node: &StructNode,
    file_context: &FileContext,
    known_ports: &HashSet<PortSignature>,
) -> Classification {
    let matching_ctor = file_context.constructors.iter()
        .find(|c| c.inferred_struct == struct_node.name);

    if let Some(ctor) = matching_ctor {
        let port_sig = PortSignature {
            package: ctor.return_package.clone(),
            name: ctor.return_type.clone(),
        };
        if known_ports.contains(&port_sig) {
            return if ctor.return_type.ends_with("Repository") {
                Classification { kind: ComponentKind::Repository(ctor.return_type.clone()), confidence: Confidence::High }
            } else {
                Classification { kind: ComponentKind::Adapter(ctor.return_type.clone()), confidence: Confidence::High }
            };
        }
    }
    // Fall through to secondary rules...
}
```

**Analysis flow:**

```
For each Go file:
  1. Extract FileContext (all constructors, structs, interfaces)
  2. For each struct:
     a. Check if a matching constructor exists (by name inference)
     b. Check if constructor returns a known port
     c. Classify as Repository or Adapter accordingly
     d. Fall back to naming heuristics if no constructor found
```

---

#### 2.2.2 Generic Port Detection (Go 1.18+)

Generic ports require parsing type parameters from the return type signature.

```go
// Generic port
type Repository[T Entity] interface {
    Save(ctx context.Context, entity T) error
    FindByID(ctx context.Context, id string) (T, error)
}

// Constructor returning generic port
func NewMongoInvoiceRepository() ports.Repository[Invoice] { ... }
```

```rust
struct ReturnType {
    package: String,           // "ports"
    name: String,              // "Repository"
    type_params: Vec<String>,  // ["Invoice"]
}

fn matches_port(return_type: &ReturnType, known_ports: &HashSet<PortSignature>) -> bool {
    // Match on base type name only; ignore type parameters
    known_ports.contains(&PortSignature {
        package: return_type.package.clone(),
        name: return_type.name.clone(),
    })
}
```

---

### 2.3 Repository Detection

Repository is a specialized subtype of Adapter. It takes precedence over generic Adapter
classification in the precedence order (see Section 3).

**Primary Rule:** Implements a repository port interface (via constructor detection)

```
IF:
  - Classified as Adapter via constructor rule (Section 2.2.1)
  - Constructor return type ends with: Repository
THEN:
  ComponentKind = Repository(port_name)
  Confidence = HIGH
```

**Secondary Rule:** Repository naming pattern

```
IF:
  - Node is struct
  - Name ends with: Repository | Repo
  - File path matches: **/infrastructure/**
THEN:
  ComponentKind = Repository(name)
  Confidence = MEDIUM
```

**Test Cases:**

```
✅ mongoInvoiceRepository implements InvoiceRepository → Repository(InvoiceRepository)
✅ type InMemoryUserRepository struct (infra, ends with Repository) → Repository
❌ InvoiceRepository (interface in domain/ports) → Port (NOT Repository)
```

---

### 2.4 Domain Event Detection

**Primary Rule:** Struct in events directory with past-tense name

```
IF:
  - Node is struct
  - File path matches: **/domain/events/**
  - Name matches: *Event | *Created | *Updated | *Deleted | *Finalized | *Succeeded | *Failed
THEN:
  ComponentKind = DomainEvent
  Confidence = HIGH
```

**Secondary Rule:** Past-tense naming pattern in domain layer

```
IF:
  - Node is struct
  - Layer = Domain
  - Name ends with:
      Event | Created | Finalized | Succeeded | Failed |
      Paid | Voided | Canceled | Processed | Refunded
THEN:
  ComponentKind = DomainEvent
  Confidence = MEDIUM
```

**Tertiary Rule:** Embeds a known base event type

```
IF:
  - Node is struct
  - Has embedded field: DomainEvent | BaseEvent | EventMetadata
  - Layer = Domain
THEN:
  ComponentKind = DomainEvent
  Confidence = HIGH
```

**Test Cases:**

```
✅ domain/events/events.go:InvoiceFinalizedEvent → DomainEvent
✅ domain/events/events.go:PaymentSucceededEvent → DomainEvent
❌ application/handlers/webhook_handler.go:WebhookEvent → NOT DomainEvent (wrong layer)
❌ domain/models/invoice.go:InvoiceStatus → NOT DomainEvent (not past tense)
```

---

### 2.5 Service Detection

**Rule Set A: Application Service**

```
IF:
  - Node is struct
  - File path matches: **/application/**
  - Name ends with: Service | Svc
THEN:
  ComponentKind = Service(Application)
  Confidence = HIGH
```

**Rule Set B: Domain Service**

```
IF:
  - Node is struct
  - File path matches: **/domain/services/**
  - Name ends with: Service
THEN:
  ComponentKind = Service(Domain)
  Confidence = HIGH
```

**Note:** Infrastructure-layer structs whose names end with `Service` (e.g.
`mailgunNotificationService`) are caught by the infrastructure adapter rules (Section 2.2)
before reaching service detection.

**Test Cases:**

```
✅ application/invoice_service.go:InvoiceService → Service(Application)
✅ domain/services/pricing_service.go:PricingService → Service(Domain)
❌ infrastructure/mailgun/notification_service.go:mailgunNotificationService
   → Adapter (NOT Service — infrastructure rules run first)
```

---

### 2.6 Entity Detection

**Primary Rule:** Has ID field and methods in domain layer

```
IF:
  - Node is struct
  - Layer = Domain
  - Has field named: ID | Id
  - Has methods (method_count > 0)
THEN:
  ComponentKind = Entity
  Confidence = HIGH
```

**Secondary Rule:** Has ID field but no methods (anemic entity)

```
IF:
  - Node is struct
  - Layer = Domain
  - Has field named: ID | Id
  - Has no methods
THEN:
  ComponentKind = Entity
  Confidence = MEDIUM
  Flag = AnemicDomainModel  ← surface as improvement opportunity
```

**Exclusion Rule:** No ID field → Value Object

```
IF:
  - Node would match Entity rules
  - BUT has no ID field
THEN:
  ComponentKind = ValueObject (NOT Entity)
```

**Test Cases:**

```
✅ domain/models/invoice.go:Invoice (has ID, has Finalize() method) → Entity
✅ domain/models/payment.go:Payment (has ID, has MarkAsPaid() method) → Entity
⚠️  domain/models/line_item.go:LineItem (has ID, no methods) → Entity [AnemicDomainModel flag]
❌ domain/models/money.go:Money (no ID, immutable methods) → ValueObject (NOT Entity)
```

---

### 2.7 Value Object Detection

**Primary Rule:** No ID field in domain layer

```
IF:
  - Node is struct
  - Layer = Domain
  - Has NO field named: ID | Id
  - Has fields (not empty struct)
THEN:
  ComponentKind = ValueObject
  Confidence = HIGH
```

**Secondary Rule:** Common value object names

```
IF:
  - Node is struct
  - Layer = Domain
  - Name matches: Money | Email | Address | PhoneNumber |
                  Currency | Amount | Percentage | Period |
                  Coordinate | Color | Dimension
THEN:
  ComponentKind = ValueObject
  Confidence = MEDIUM
```

**Test Cases:**

```
✅ domain/models/money.go:Money {Amount int64, Currency string} → ValueObject
✅ domain/models/email.go:Email {Value string} + Validate() method → ValueObject
✅ domain/models/line_item.go:LineItem (no ID, no methods, just data) → ValueObject
❌ domain/models/invoice.go:Invoice (has ID, many methods) → Entity (NOT ValueObject)
```

---

### 2.8 Embedded Struct Handling

**Rule:** Embedded structs do NOT inherit the enclosing struct's classification.

```go
type baseRepository struct {
    collection *mongo.Collection
}

type mongoInvoiceRepository struct {
    baseRepository  // ← Embedded
}
```

**Classification:**

- `mongoInvoiceRepository` → Repository (has constructor returning port, or name suffix)
- `baseRepository` → Unclassified (implementation detail, not an independent architectural component)

**Rationale:** Embedding is a code-reuse mechanism, not an architectural boundary. A shared
base struct is an implementation detail of the adapters that embed it, not an adapter itself.

**Implementation:**

```rust
fn classify_struct(node: &StructNode, all_structs: &[StructNode], ...) -> Classification {
    // If this struct is only ever used as an embedded field in other structs,
    // it is an implementation detail rather than an architectural component.
    if is_only_embedded(node, all_structs) {
        return Classification {
            kind: ComponentKind::Unclassified,
            confidence: Confidence::Low,
            note: Some("Implementation detail (only used as embedded struct)".into()),
        };
    }
    // Normal classification proceeds...
}
```

---

## 3. Precedence Rules (Conflict Resolution)

When multiple rules match, apply in this order:

```
1. Port       (interface check is definitive)
2. DomainEvent (events directory + past-tense name is strong signal)
3. Repository  (more specific subtype of Adapter — check before generic Adapter)
4. Adapter     (constructor signature is highest confidence within this tier)
5. Entity      (ID field + methods)
6. ValueObject (no ID field)
7. Service     (fallback for structs with methods in app/domain layers)
```

**Example conflict:**

```
Node: mongoInvoiceRepository
Matches:
  - Repository (ends with "Repository" in infrastructure) ✓
  - Adapter (unexported struct in infrastructure) ✓

Resolution:
  Repository wins — it is the more specific classification (precedence 3 > 4).
```

**Pseudocode implementing the precedence order:**

```rust
fn classify_component(node: &ParsedNode, context: &AnalysisContext) -> Classification {
    // 1. Port
    if node.is_interface() && context.layer == Layer::Domain {
        if context.file_path.contains("domain/ports") || is_port_like_name(&node.name) {
            return Classification { kind: ComponentKind::Port, confidence: Confidence::High };
        }
    }

    // 2. DomainEvent
    if node.is_struct() && context.layer == Layer::Domain {
        if context.file_path.contains("domain/events") && is_past_tense_name(&node.name) {
            return Classification { kind: ComponentKind::DomainEvent, confidence: Confidence::High };
        }
    }

    // Infrastructure layer: Repository and Adapter checks
    if node.is_struct() && context.layer == Layer::Infrastructure {

        // 3. Repository (before generic Adapter)
        if node.name.to_lowercase().ends_with("repository")
            || node.name.to_lowercase().ends_with("repo")
        {
            return Classification { kind: ComponentKind::Repository(node.name.clone()), confidence: Confidence::Medium };
        }

        // Constructor check — may produce Repository or Adapter
        if let Some(ctor) = find_constructor_returning_port(node, &context.file_context) {
            return if ctor.return_type.ends_with("Repository") {
                Classification { kind: ComponentKind::Repository(ctor.return_type), confidence: Confidence::High }
            } else {
                Classification { kind: ComponentKind::Adapter(ctor.return_type), confidence: Confidence::High }
            };
        }

        // 4. Adapter (unexported struct or explicit suffix)
        if node.name.starts_with(|c: char| c.is_lowercase()) {
            return Classification { kind: ComponentKind::Adapter(infer_port_name(&node.name)), confidence: Confidence::Medium };
        }
        if is_explicit_adapter_suffix(&node.name) {
            return Classification { kind: ComponentKind::Adapter(node.name.clone()), confidence: Confidence::Low };
        }
    }

    // 5. Entity (ID field + methods in domain)
    if node.is_struct() && context.layer == Layer::Domain {
        if node.has_id_field() {
            let confidence = if node.method_count() > 0 { Confidence::High } else { Confidence::Medium };
            let flags = if node.method_count() == 0 { vec![Flag::AnemicDomainModel] } else { vec![] };
            return Classification { kind: ComponentKind::Entity, confidence, flags };
        }

        // 6. ValueObject (no ID field)
        return Classification { kind: ComponentKind::ValueObject, confidence: Confidence::High };
    }

    // 7. Service
    if node.is_struct()
        && (context.layer == Layer::Application || context.layer == Layer::Domain)
        && (node.name.ends_with("Service") || node.has_dependencies())
    {
        return Classification { kind: ComponentKind::Service(context.layer), confidence: Confidence::Medium };
    }

    Classification { kind: ComponentKind::Unclassified, confidence: Confidence::Low }
}
```

---

## 4. Confidence Levels

**HIGH (90–100%):**

- Interface in `domain/ports` → Port
- Constructor returns known port → Adapter or Repository
- Struct in `domain/events` with past-tense name → DomainEvent
- Struct with ID field and methods in domain → Entity

**MEDIUM (60–90%):**

- Unexported struct in infrastructure → Adapter (inferred)
- Name ends with `Repository` in infrastructure → Repository
- Name ends with `Service` in application → Service
- Struct with ID field but no methods in domain → Entity (AnemicDomainModel flag)

**LOW (30–60%):**

- Explicit adapter suffixes only (Client, Gateway) — no constructor evidence
- Heuristics based on field count or method count alone

**If confidence < 60%:** Classify as `Unclassified` and surface for human review.

---

## 5. Test Suite Requirements

Each rule must have tests covering positive, negative, and edge cases.

**Positive:**

```rust
#[test]
fn port_detection_interface_in_ports_directory() {
    let node = mock_interface("InvoiceRepository", "domain/ports/invoice_repository.go");
    assert_eq!(classify(node).kind, ComponentKind::Port);
}
```

**Negative:**

```rust
#[test]
fn port_detection_struct_is_not_port() {
    let node = mock_struct("InvoiceRepository", "domain/ports/invoice_repository.go");
    assert_ne!(classify(node).kind, ComponentKind::Port);
}
```

**Edge cases:**

```rust
#[test]
fn adapter_vs_service_application_handler() {
    let node = mock_struct("PaymentHandler", "application/payment_handler.go");
    assert_matches!(classify(node).kind, ComponentKind::Service(_));
    assert!(!matches!(classify(node).kind, ComponentKind::Adapter(_)));
}
```

**Precedence:**

```rust
#[test]
fn repository_takes_precedence_over_adapter() {
    // Both Repository and Adapter rules match; Repository must win.
    let node = mock_struct("mongoInvoiceRepository", "infrastructure/mongodb/invoice_repository.go");
    assert_matches!(classify(node).kind, ComponentKind::Repository(_));
}

#[test]
fn unexported_infra_struct_classified_as_adapter_not_entity() {
    let node = mock_struct("stripePaymentProcessor", "infrastructure/stripe/processor.go");
    assert_matches!(classify(node).kind, ComponentKind::Adapter(_));
}
```

---

## 6. Validation Criteria

A component classification is valid if:

```
✅ Exactly ONE ComponentKind assigned (no ambiguity)
✅ Confidence >= MEDIUM (60%)
✅ Layer matches expected pattern (e.g., Adapters in Infrastructure, Ports in Domain)
✅ No contradictions (e.g., a Port that is also an Entity)
```

Invalid classifications should:

```
1. Be flagged in output (warning or info)
2. Count as "Unclassified" in metrics
3. Provide a suggestion for resolution
```

---

## 7. Extensibility Points

**Language-specific overrides:**

```rust
trait LanguageSpecificClassifier {
    fn classify_port(&self, node: &Node) -> Option<Classification>;
    fn classify_adapter(&self, node: &Node) -> Option<Classification>;
    fn classify_constructor(&self, node: &Node) -> Option<ConstructorSignature>;
}
```

**Custom patterns (user config):**

```toml
[classification.overrides]
"**/infrastructure/**/webhook_handler.go" = "adapter"
"**/domain/**/state.go" = "value_object"
```

---

## 8. Open Questions / Future Work

1. **Generic types (Go 1.18+):** Detection strategy described in Section 2.2.2. Implement
   alongside constructor detection in Phase 3.

2. **Multi-language support:** Java uses annotations (`@Repository`, `@Service`); C# uses
   interfaces and constructor injection similarly to Go. Classification rules will need
   language-specific implementations of the common precedence order.

3. **Dynamic ports:** Some frameworks use reflection or code generation to wire ports and
   adapters at runtime. Static analysis cannot detect these — they should be surfaced as
   unclassified with a note.

4. **Anonymous functions:** Go's `func(...) Interface` return type (without a named
   constructor) is not caught by the constructor detection rule. Low prevalence in
   production DDD codebases; defer to a later phase.

5. **Embedded structs:** Rule defined in Section 2.8. `is_only_embedded` requires a
   cross-struct reference pass within the same package; implement alongside two-pass
   constructor detection.

---

## 9. Implementation Priority

### Phase 1 — Immediate

**1. Port detection** ✅ Already implemented.

**2. Repository detection (name suffix)**

```rust
// Simple, high-value, no file context required
if lower.ends_with("repository") || lower.ends_with("repo") {
    return ComponentKind::Repository;
}
```

Impact: Repositories and adapters counted separately in metrics.
Effort: Trivial (part of `classify_struct_kind` rewrite).

**3. Adapter detection (unexported struct + file path)**

```rust
// Catches stripePaymentProcessor, mailgunNotificationService, etc.
if file_path.contains("infrastructure/") && name.starts_with(|c: char| c.is_lowercase()) {
    return ComponentKind::Adapter(AdapterInfo { name: name.to_string(), implements: Vec::new() });
}
```

Impact: Catches most real-world Go adapter implementations.
Effort: Requires adding `file_path` parameter to `classify_struct_kind`.

**Expected after Phase 1:**
`interface_coverage` rises from ~7% to ~60–70% for a well-structured billing module.

---

### Phase 2 — High Value

**4. Entity / ValueObject distinction (ID field check)**

```rust
if node.has_id_field() { return Entity; }
else if context.layer == Layer::Domain { return ValueObject; }
```

Impact: Correct anemic-model detection; eliminates false positives.
Effort: ~2 hours.

**5. DomainEvent detection (past-tense naming + events directory)**

```rust
if file_path.contains("domain/events/") && is_past_tense(&node.name) {
    return DomainEvent;
}
```

Impact: Events classified correctly; appear in metrics.
Effort: ~1 hour.

**Expected after Phase 2:**
AnemicDomainModel flags reflect real issues; domain events appear in output.

---

### Phase 3 — Complete

**6. Adapter detection via constructor signature (two-pass)**
Full `FileContext` implementation as described in Section 2.2.1.
Impact: Near-complete adapter detection accuracy.
Effort: 4–8 hours (requires additional tree-sitter queries for function declarations).

**7. Service refinement**
Distinguish Application vs Domain services by file path context.
Effort: ~2 hours.

**8. Generic type support**
Parse `ports.Repository[Invoice]` return types as described in Section 2.2.2.
Effort: ~3–4 hours.

**Expected after Phase 3:**
`interface_coverage` reflects constructor-verified adapter/port pairings with high accuracy.
