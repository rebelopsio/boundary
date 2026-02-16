use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;

use crate::types::Severity;

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
}

fn default_languages() -> Vec<String> {
    vec!["go".to_string()]
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
        }
    }
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

/// Rule configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RulesConfig {
    #[serde(default = "default_severities")]
    pub severities: HashMap<String, Severity>,
    #[serde(default = "default_fail_on")]
    pub fail_on: Severity,
    #[serde(default)]
    pub min_score: Option<f64>,
}

fn default_severities() -> HashMap<String, Severity> {
    let mut m = HashMap::new();
    m.insert("layer_boundary".to_string(), Severity::Error);
    m.insert("circular_dependency".to_string(), Severity::Error);
    m.insert("missing_port".to_string(), Severity::Warning);
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
        }
    }
}

impl Config {
    /// Load configuration from a `.boundary.toml` file.
    pub fn load(path: &Path) -> Result<Self> {
        let content = std::fs::read_to_string(path)?;
        let config: Config = toml::from_str(&content)?;
        Ok(config)
    }

    /// Load from `.boundary.toml` in the given directory, or return defaults.
    pub fn load_or_default(dir: &Path) -> Self {
        let config_path = dir.join(".boundary.toml");
        if config_path.exists() {
            Self::load(&config_path).unwrap_or_default()
        } else {
            Self::default()
        }
    }

    /// Generate default TOML content for `boundary init`.
    pub fn default_toml() -> String {
        r#"# Boundary - Architecture Analysis Configuration
# See https://github.com/stephencaudill/boundary for documentation

[project]
languages = ["go"]
exclude_patterns = ["vendor/**", "**/*_test.go", "**/testdata/**"]

[layers]
# Glob patterns to classify files into architectural layers
domain = ["**/domain/**", "**/entity/**", "**/model/**"]
application = ["**/application/**", "**/usecase/**", "**/service/**"]
infrastructure = ["**/infrastructure/**", "**/adapter/**", "**/repository/**", "**/persistence/**"]
presentation = ["**/presentation/**", "**/handler/**", "**/api/**", "**/cmd/**"]

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
        assert_eq!(config.project.languages, vec!["go"]);
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
        assert_eq!(config.project.languages, vec!["go"]);
    }
}
