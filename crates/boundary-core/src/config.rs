use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;

use crate::types::{ArchitectureMode, Severity};

/// Top-level configuration from `.boundary.toml`
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Config {
    #[serde(default)]
    pub project: ProjectConfig,
    #[serde(default)]
    pub layers: LayersConfig,
    #[serde(default)]
    pub scoring: ScoringConfig,
    #[serde(default)]
    pub rules: RulesConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectConfig {
    #[serde(default = "default_languages")]
    pub languages: Vec<String>,
    #[serde(default)]
    pub exclude_patterns: Vec<String>,
    #[serde(default)]
    pub services_pattern: Option<String>,
}

fn default_languages() -> Vec<String> {
    vec![]
}

impl Default for ProjectConfig {
    fn default() -> Self {
        Self {
            languages: default_languages(),
            exclude_patterns: vec![
                "vendor/**".to_string(),
                "**/*_test.go".to_string(),
                "**/testdata/**".to_string(),
            ],
            services_pattern: None,
        }
    }
}

/// Per-module override for layer classification patterns.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LayerOverrideConfig {
    pub scope: String,
    #[serde(default)]
    pub domain: Vec<String>,
    #[serde(default)]
    pub application: Vec<String>,
    #[serde(default)]
    pub infrastructure: Vec<String>,
    #[serde(default)]
    pub presentation: Vec<String>,
    #[serde(default)]
    pub architecture_mode: Option<ArchitectureMode>,
}

/// Glob patterns mapping file paths to architectural layers
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LayersConfig {
    #[serde(default = "default_domain_patterns")]
    pub domain: Vec<String>,
    #[serde(default = "default_application_patterns")]
    pub application: Vec<String>,
    #[serde(default = "default_infrastructure_patterns")]
    pub infrastructure: Vec<String>,
    #[serde(default = "default_presentation_patterns")]
    pub presentation: Vec<String>,
    #[serde(default)]
    pub overrides: Vec<LayerOverrideConfig>,
    #[serde(default)]
    pub cross_cutting: Vec<String>,
    #[serde(default)]
    pub architecture_mode: ArchitectureMode,
}

fn default_domain_patterns() -> Vec<String> {
    vec![
        "**/domain/**".to_string(),
        "**/entity/**".to_string(),
        "**/model/**".to_string(),
    ]
}

fn default_application_patterns() -> Vec<String> {
    vec![
        "**/application/**".to_string(),
        "**/usecase/**".to_string(),
        "**/service/**".to_string(),
    ]
}

fn default_infrastructure_patterns() -> Vec<String> {
    vec![
        "**/infrastructure/**".to_string(),
        "**/adapter/**".to_string(),
        "**/repository/**".to_string(),
        "**/persistence/**".to_string(),
    ]
}

fn default_presentation_patterns() -> Vec<String> {
    vec![
        "**/presentation/**".to_string(),
        "**/handler/**".to_string(),
        "**/api/**".to_string(),
        "**/cmd/**".to_string(),
    ]
}

impl Default for LayersConfig {
    fn default() -> Self {
        Self {
            domain: default_domain_patterns(),
            application: default_application_patterns(),
            infrastructure: default_infrastructure_patterns(),
            presentation: default_presentation_patterns(),
            overrides: Vec::new(),
            cross_cutting: Vec::new(),
            architecture_mode: ArchitectureMode::default(),
        }
    }
}

/// Weights for scoring sub-components (should sum to ~1.0)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScoringConfig {
    #[serde(default = "default_layer_weight")]
    pub layer_isolation_weight: f64,
    #[serde(default = "default_dep_weight")]
    pub dependency_direction_weight: f64,
    #[serde(default = "default_interface_weight")]
    pub interface_coverage_weight: f64,
}

fn default_layer_weight() -> f64 {
    0.4
}
fn default_dep_weight() -> f64 {
    0.4
}
fn default_interface_weight() -> f64 {
    0.2
}

impl Default for ScoringConfig {
    fn default() -> Self {
        Self {
            layer_isolation_weight: default_layer_weight(),
            dependency_direction_weight: default_dep_weight(),
            interface_coverage_weight: default_interface_weight(),
        }
    }
}

/// A custom rule defined in configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CustomRuleConfig {
    pub name: String,
    pub from_pattern: String,
    pub to_pattern: String,
    #[serde(default = "default_deny")]
    pub action: String,
    #[serde(default = "default_custom_rule_severity")]
    pub severity: Severity,
    #[serde(default)]
    pub message: Option<String>,
}

fn default_deny() -> String {
    "deny".to_string()
}

fn default_custom_rule_severity() -> Severity {
    Severity::Error
}

/// Rule configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RulesConfig {
    #[serde(default = "default_severities")]
    pub severities: HashMap<String, Severity>,
    #[serde(default = "default_fail_on")]
    pub fail_on: Severity,
    #[serde(default)]
    pub min_score: Option<f64>,
    #[serde(default)]
    pub custom_rules: Vec<CustomRuleConfig>,
    #[serde(default = "default_true")]
    pub detect_init_functions: bool,
}

fn default_true() -> bool {
    true
}

fn default_severities() -> HashMap<String, Severity> {
    let mut m = HashMap::new();
    m.insert("layer_boundary".to_string(), Severity::Error);
    m.insert("circular_dependency".to_string(), Severity::Error);
    m.insert("missing_port".to_string(), Severity::Warning);
    m.insert("init_coupling".to_string(), Severity::Warning);
    m
}

fn default_fail_on() -> Severity {
    Severity::Error
}

impl Default for RulesConfig {
    fn default() -> Self {
        Self {
            severities: default_severities(),
            fail_on: default_fail_on(),
            min_score: None,
            custom_rules: Vec::new(),
            detect_init_functions: true,
        }
    }
}

impl Config {
    /// Load configuration from a `.boundary.toml` file.
    pub fn load(path: &Path) -> Result<Self> {
        let content = std::fs::read_to_string(path)
            .with_context(|| format!("failed to read config file '{}'", path.display()))?;
        let config: Config = toml::from_str(&content).with_context(|| {
            format!(
                "failed to parse '{}'. Run `boundary init` to create a valid config file",
                path.display()
            )
        })?;
        Ok(config)
    }

    /// Load from `.boundary.toml` in the given directory or any ancestor, or return defaults.
    pub fn load_or_default(dir: &Path) -> Self {
        // Walk up from dir to find .boundary.toml (similar to how git finds .git)
        let start = dir.canonicalize().unwrap_or_else(|_| dir.to_path_buf());
        let mut current = start.as_path();
        loop {
            let config_path = current.join(".boundary.toml");
            if config_path.exists() {
                return match Self::load(&config_path) {
                    Ok(config) => config,
                    Err(e) => {
                        eprintln!(
                            "Warning: failed to load config from '{}': {e:#}. Using defaults.",
                            config_path.display()
                        );
                        Self::default()
                    }
                };
            }
            match current.parent() {
                Some(parent) => current = parent,
                None => break,
            }
        }
        Self::default()
    }

    /// Generate default TOML content for `boundary init`.
    pub fn default_toml() -> String {
        r#"# Boundary - Architecture Analysis Configuration
# See https://github.com/rebelopsio/boundary for documentation

[project]
languages = ["go"]
exclude_patterns = ["vendor/**", "**/*_test.go", "**/testdata/**"]

[layers]
# Glob patterns to classify files into architectural layers
domain = ["**/domain/**", "**/entity/**", "**/model/**"]
application = ["**/application/**", "**/usecase/**", "**/service/**"]
infrastructure = ["**/infrastructure/**", "**/adapter/**", "**/repository/**", "**/persistence/**"]
presentation = ["**/presentation/**", "**/handler/**", "**/api/**", "**/cmd/**"]

# Paths exempt from layer violation checks (cross-cutting concerns)
# cross_cutting = ["common/utils/**", "pkg/logger/**", "pkg/errors/**"]

# Per-module overrides — matched by scope, first match wins.
# Omitted layers fall back to global patterns above.
# [[layers.overrides]]
# scope = "services/auth/**"
# domain = ["services/auth/core/**"]
# infrastructure = ["services/auth/server/**", "services/auth/adapters/**"]

[scoring]
# Weights for score components (should sum to 1.0)
layer_isolation_weight = 0.4
dependency_direction_weight = 0.4
interface_coverage_weight = 0.2

[rules]
# Severity levels: "error", "warning", "info"
fail_on = "error"
# min_score = 70.0

[rules.severities]
layer_boundary = "error"
circular_dependency = "error"
missing_port = "warning"
init_coupling = "warning"
"#
        .to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = Config::default();
        assert!(
            config.project.languages.is_empty(),
            "default should be empty for auto-detection"
        );
        assert!(!config.layers.domain.is_empty());
        assert!((config.scoring.layer_isolation_weight - 0.4).abs() < f64::EPSILON);
    }

    #[test]
    fn test_deserialize_config() {
        let toml_str = r#"
[project]
languages = ["go", "rust"]

[layers]
domain = ["**/core/**"]
application = ["**/app/**"]
infrastructure = ["**/infra/**"]
presentation = ["**/web/**"]

[scoring]
layer_isolation_weight = 0.5
dependency_direction_weight = 0.3
interface_coverage_weight = 0.2

[rules]
fail_on = "warning"
"#;
        let config: Config = toml::from_str(toml_str).unwrap();
        assert_eq!(config.project.languages, vec!["go", "rust"]);
        assert_eq!(config.layers.domain, vec!["**/core/**"]);
        assert_eq!(config.rules.fail_on, Severity::Warning);
    }

    #[test]
    fn test_default_toml_is_valid() {
        let toml_str = Config::default_toml();
        let config: Config = toml::from_str(&toml_str).unwrap();
        // The template specifies "go" as a starter config
        assert_eq!(config.project.languages, vec!["go"]);
    }

    #[test]
    fn test_deserialize_layer_overrides() {
        let toml_str = r#"
[layers]
domain = ["**/domain/**"]
application = ["**/application/**"]
infrastructure = ["**/infrastructure/**"]
presentation = ["**/handler/**"]

[[layers.overrides]]
scope = "services/auth/**"
domain = ["services/auth/core/**"]
infrastructure = ["services/auth/server/**", "services/auth/adapters/**"]

[[layers.overrides]]
scope = "common/modules/*/**"
domain = ["common/modules/*/domain/**"]
application = ["common/modules/*/app/**"]
"#;
        let config: Config = toml::from_str(toml_str).unwrap();
        assert_eq!(config.layers.overrides.len(), 2);
        assert_eq!(config.layers.overrides[0].scope, "services/auth/**");
        assert_eq!(
            config.layers.overrides[0].domain,
            vec!["services/auth/core/**"]
        );
        assert_eq!(
            config.layers.overrides[0].infrastructure,
            vec!["services/auth/server/**", "services/auth/adapters/**"]
        );
        // Second override has no infrastructure/presentation
        assert!(config.layers.overrides[1].infrastructure.is_empty());
        assert!(config.layers.overrides[1].presentation.is_empty());
    }

    #[test]
    fn test_empty_overrides_backward_compatible() {
        let toml_str = r#"
[layers]
domain = ["**/domain/**"]
"#;
        let config: Config = toml::from_str(toml_str).unwrap();
        assert!(config.layers.overrides.is_empty());
    }

    #[test]
    fn test_deserialize_cross_cutting() {
        let toml_str = r#"
[layers]
domain = ["**/domain/**"]
application = ["**/application/**"]
infrastructure = ["**/infrastructure/**"]
presentation = ["**/handler/**"]
cross_cutting = ["common/utils/**", "pkg/logger/**", "pkg/errors/**"]
"#;
        let config: Config = toml::from_str(toml_str).unwrap();
        assert_eq!(config.layers.cross_cutting.len(), 3);
        assert_eq!(config.layers.cross_cutting[0], "common/utils/**");
        assert_eq!(config.layers.cross_cutting[1], "pkg/logger/**");
        assert_eq!(config.layers.cross_cutting[2], "pkg/errors/**");
    }

    #[test]
    fn test_architecture_mode_deserializes() {
        let toml_str = r#"
[layers]
domain = ["**/domain/**"]
architecture_mode = "active-record"

[[layers.overrides]]
scope = "services/legacy/**"
architecture_mode = "service-oriented"
"#;
        let config: Config = toml::from_str(toml_str).unwrap();
        assert_eq!(
            config.layers.architecture_mode,
            ArchitectureMode::ActiveRecord
        );
        assert_eq!(
            config.layers.overrides[0].architecture_mode,
            Some(ArchitectureMode::ServiceOriented)
        );
    }

    #[test]
    fn test_architecture_mode_missing_backward_compat() {
        let toml_str = r#"
[layers]
domain = ["**/domain/**"]
"#;
        let config: Config = toml::from_str(toml_str).unwrap();
        assert_eq!(config.layers.architecture_mode, ArchitectureMode::Ddd);
    }

    #[test]
    fn test_services_pattern_parses() {
        let toml_str = r#"
[project]
services_pattern = "apps/*"
"#;
        let config: Config = toml::from_str(toml_str).unwrap();
        assert_eq!(config.project.services_pattern.as_deref(), Some("apps/*"));
    }

    #[test]
    fn test_detect_init_functions_defaults_true() {
        let config = Config::default();
        assert!(config.rules.detect_init_functions);
    }

    #[test]
    fn test_missing_cross_cutting_backward_compatible() {
        let toml_str = r#"
[layers]
domain = ["**/domain/**"]
"#;
        let config: Config = toml::from_str(toml_str).unwrap();
        assert!(config.layers.cross_cutting.is_empty());
    }
}
