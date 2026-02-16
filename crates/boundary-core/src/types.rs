use serde::{Deserialize, Serialize};
use std::fmt;
use std::path::PathBuf;

/// Unique identifier for a component: "package::Name"
#[derive(Debug, Clone, Hash, PartialEq, Eq, Serialize, Deserialize)]
pub struct ComponentId(pub String);

impl ComponentId {
    pub fn new(package: &str, name: &str) -> Self {
        Self(format!("{package}::{name}"))
    }
}

impl fmt::Display for ComponentId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Location in source code
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SourceLocation {
    pub file: PathBuf,
    pub line: usize,
    pub column: usize,
}

impl fmt::Display for SourceLocation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}", self.file.display(), self.line)
    }
}

/// Architectural layer in hexagonal/clean architecture.
/// Ordered from innermost (Domain=0) to outermost (Presentation=3).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ArchLayer {
    Domain,
    Application,
    Infrastructure,
    Presentation,
}

impl ArchLayer {
    /// Numeric depth: 0 = innermost, 3 = outermost.
    pub fn depth(&self) -> u8 {
        match self {
            ArchLayer::Domain => 0,
            ArchLayer::Application => 1,
            ArchLayer::Infrastructure => 2,
            ArchLayer::Presentation => 3,
        }
    }

    /// Returns true if `self` depending on `other` is a violation
    /// (inner layer depending on outer layer).
    pub fn violates_dependency_on(&self, other: &ArchLayer) -> bool {
        self.depth() < other.depth()
    }
}

impl fmt::Display for ArchLayer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ArchLayer::Domain => write!(f, "domain"),
            ArchLayer::Application => write!(f, "application"),
            ArchLayer::Infrastructure => write!(f, "infrastructure"),
            ArchLayer::Presentation => write!(f, "presentation"),
        }
    }
}

/// Kind of architectural component
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ComponentKind {
    Port(PortInfo),
    Adapter(AdapterInfo),
    Entity(EntityInfo),
    ValueObject,
    UseCase,
    Repository,
    Service,
}

/// Information about a port (interface)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PortInfo {
    pub name: String,
    pub methods: Vec<String>,
}

/// Information about an adapter (implementation)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdapterInfo {
    pub name: String,
    pub implements: Vec<String>,
}

/// Information about a domain entity
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntityInfo {
    pub name: String,
    pub fields: Vec<String>,
}

/// A discovered architectural component
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Component {
    pub id: ComponentId,
    pub name: String,
    pub kind: ComponentKind,
    pub layer: Option<ArchLayer>,
    pub location: SourceLocation,
}

/// Kind of dependency relationship
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DependencyKind {
    Import,
    MethodCall,
    TypeReference,
    Inheritance,
}

/// A dependency between components or files
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Dependency {
    pub from: ComponentId,
    pub to: ComponentId,
    pub kind: DependencyKind,
    pub location: SourceLocation,
    pub import_path: Option<String>,
}

/// Severity of a violation
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    Info,
    Warning,
    Error,
}

impl fmt::Display for Severity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Severity::Info => write!(f, "info"),
            Severity::Warning => write!(f, "warning"),
            Severity::Error => write!(f, "error"),
        }
    }
}

impl std::str::FromStr for Severity {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "info" => Ok(Severity::Info),
            "warning" | "warn" => Ok(Severity::Warning),
            "error" => Ok(Severity::Error),
            _ => Err(anyhow::anyhow!("unknown severity: {s}")),
        }
    }
}

/// Kind of architectural violation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ViolationKind {
    LayerBoundary {
        from_layer: ArchLayer,
        to_layer: ArchLayer,
    },
    CircularDependency {
        cycle: Vec<ComponentId>,
    },
    MissingPort {
        adapter_name: String,
    },
}

/// An architectural violation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Violation {
    pub kind: ViolationKind,
    pub severity: Severity,
    pub location: SourceLocation,
    pub message: String,
    pub suggestion: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_arch_layer_depth() {
        assert_eq!(ArchLayer::Domain.depth(), 0);
        assert_eq!(ArchLayer::Application.depth(), 1);
        assert_eq!(ArchLayer::Infrastructure.depth(), 2);
        assert_eq!(ArchLayer::Presentation.depth(), 3);
    }

    #[test]
    fn test_violates_dependency_on_truth_table() {
        use ArchLayer::*;

        // Inner depending on outer = violation
        assert!(Domain.violates_dependency_on(&Application));
        assert!(Domain.violates_dependency_on(&Infrastructure));
        assert!(Domain.violates_dependency_on(&Presentation));
        assert!(Application.violates_dependency_on(&Infrastructure));
        assert!(Application.violates_dependency_on(&Presentation));
        assert!(Infrastructure.violates_dependency_on(&Presentation));

        // Same layer = no violation
        assert!(!Domain.violates_dependency_on(&Domain));
        assert!(!Infrastructure.violates_dependency_on(&Infrastructure));

        // Outer depending on inner = no violation
        assert!(!Infrastructure.violates_dependency_on(&Domain));
        assert!(!Infrastructure.violates_dependency_on(&Application));
        assert!(!Application.violates_dependency_on(&Domain));
        assert!(!Presentation.violates_dependency_on(&Domain));
    }

    #[test]
    fn test_severity_ordering() {
        assert!(Severity::Info < Severity::Warning);
        assert!(Severity::Warning < Severity::Error);
    }

    #[test]
    fn test_severity_parse() {
        assert_eq!("error".parse::<Severity>().unwrap(), Severity::Error);
        assert_eq!("warning".parse::<Severity>().unwrap(), Severity::Warning);
        assert_eq!("warn".parse::<Severity>().unwrap(), Severity::Warning);
        assert_eq!("info".parse::<Severity>().unwrap(), Severity::Info);
        assert!("unknown".parse::<Severity>().is_err());
    }

    #[test]
    fn test_component_id() {
        let id = ComponentId::new("pkg", "Name");
        assert_eq!(id.0, "pkg::Name");
        assert_eq!(id.to_string(), "pkg::Name");
    }
}
