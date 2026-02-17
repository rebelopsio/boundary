use globset::{Glob, GlobSet, GlobSetBuilder};

use crate::config::LayersConfig;
use crate::types::ArchLayer;

/// A compiled per-module layer override.
struct LayerOverride {
    scope: GlobSet,
    domain: GlobSet,
    application: GlobSet,
    infrastructure: GlobSet,
    presentation: GlobSet,
    has_domain: bool,
    has_application: bool,
    has_infrastructure: bool,
    has_presentation: bool,
}

/// Classifies file paths into architectural layers using glob patterns.
pub struct LayerClassifier {
    domain: GlobSet,
    application: GlobSet,
    infrastructure: GlobSet,
    presentation: GlobSet,
    overrides: Vec<LayerOverride>,
}

fn build_globset(patterns: &[String]) -> GlobSet {
    let mut builder = GlobSetBuilder::new();
    for pattern in patterns {
        if let Ok(glob) = Glob::new(pattern) {
            builder.add(glob);
        }
    }
    builder
        .build()
        .unwrap_or_else(|_| GlobSetBuilder::new().build().unwrap())
}

impl LayerClassifier {
    pub fn new(config: &LayersConfig) -> Self {
        let overrides = config
            .overrides
            .iter()
            .map(|o| LayerOverride {
                scope: build_globset(std::slice::from_ref(&o.scope)),
                domain: build_globset(&o.domain),
                application: build_globset(&o.application),
                infrastructure: build_globset(&o.infrastructure),
                presentation: build_globset(&o.presentation),
                has_domain: !o.domain.is_empty(),
                has_application: !o.application.is_empty(),
                has_infrastructure: !o.infrastructure.is_empty(),
                has_presentation: !o.presentation.is_empty(),
            })
            .collect();

        Self {
            domain: build_globset(&config.domain),
            application: build_globset(&config.application),
            infrastructure: build_globset(&config.infrastructure),
            presentation: build_globset(&config.presentation),
            overrides,
        }
    }

    /// Classify a file path into an architectural layer.
    pub fn classify(&self, path: &str) -> Option<ArchLayer> {
        let normalized = path.replace('\\', "/");

        // Check overrides first (first matching scope wins)
        for ovr in &self.overrides {
            if ovr.scope.is_match(&normalized) {
                return self.classify_with_override(ovr, &normalized);
            }
        }

        // No override matched — use global patterns
        self.classify_global(&normalized)
    }

    /// Classify an import path string into an architectural layer.
    pub fn classify_import(&self, import_path: &str) -> Option<ArchLayer> {
        let candidates = [
            import_path.to_string(),
            format!("**/{import_path}"),
            format!("{import_path}/**"),
        ];
        for candidate in &candidates {
            if let Some(layer) = self.classify(candidate) {
                return Some(layer);
            }
        }
        // Fallback: simple substring heuristic
        let lower = import_path.to_lowercase();
        if lower.contains("/domain") || lower.contains("/entity") || lower.contains("/model") {
            Some(ArchLayer::Domain)
        } else if lower.contains("/application")
            || lower.contains("/usecase")
            || lower.contains("/service")
        {
            Some(ArchLayer::Application)
        } else if lower.contains("/infrastructure")
            || lower.contains("/adapter")
            || lower.contains("/repository")
            || lower.contains("/persistence")
        {
            Some(ArchLayer::Infrastructure)
        } else if lower.contains("/presentation")
            || lower.contains("/handler")
            || lower.contains("/api/")
            || lower.contains("/cmd")
        {
            Some(ArchLayer::Presentation)
        } else {
            None
        }
    }

    /// Classify using global patterns only.
    fn classify_global(&self, normalized: &str) -> Option<ArchLayer> {
        if self.domain.is_match(normalized) {
            Some(ArchLayer::Domain)
        } else if self.application.is_match(normalized) {
            Some(ArchLayer::Application)
        } else if self.infrastructure.is_match(normalized) {
            Some(ArchLayer::Infrastructure)
        } else if self.presentation.is_match(normalized) {
            Some(ArchLayer::Presentation)
        } else {
            None
        }
    }

    /// Classify using an override's patterns, falling back to global for layers
    /// the override doesn't define.
    fn classify_with_override(&self, ovr: &LayerOverride, normalized: &str) -> Option<ArchLayer> {
        // For each layer, use override patterns if defined, else global
        let domain_match = if ovr.has_domain {
            ovr.domain.is_match(normalized)
        } else {
            self.domain.is_match(normalized)
        };
        if domain_match {
            return Some(ArchLayer::Domain);
        }

        let app_match = if ovr.has_application {
            ovr.application.is_match(normalized)
        } else {
            self.application.is_match(normalized)
        };
        if app_match {
            return Some(ArchLayer::Application);
        }

        let infra_match = if ovr.has_infrastructure {
            ovr.infrastructure.is_match(normalized)
        } else {
            self.infrastructure.is_match(normalized)
        };
        if infra_match {
            return Some(ArchLayer::Infrastructure);
        }

        let pres_match = if ovr.has_presentation {
            ovr.presentation.is_match(normalized)
        } else {
            self.presentation.is_match(normalized)
        };
        if pres_match {
            return Some(ArchLayer::Presentation);
        }

        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{LayerOverrideConfig, LayersConfig};

    fn config_with_overrides(overrides: Vec<LayerOverrideConfig>) -> LayersConfig {
        LayersConfig {
            overrides,
            ..LayersConfig::default()
        }
    }

    #[test]
    fn test_classify_default_patterns() {
        let classifier = LayerClassifier::new(&LayersConfig::default());

        assert_eq!(
            classifier.classify("internal/domain/user/entity.go"),
            Some(ArchLayer::Domain)
        );
        assert_eq!(
            classifier.classify("internal/application/user/service.go"),
            Some(ArchLayer::Application)
        );
        assert_eq!(
            classifier.classify("internal/infrastructure/postgres/repo.go"),
            Some(ArchLayer::Infrastructure)
        );
        assert_eq!(
            classifier.classify("internal/handler/http.go"),
            Some(ArchLayer::Presentation)
        );
        assert_eq!(classifier.classify("main.go"), None);
    }

    #[test]
    fn test_classify_import() {
        let classifier = LayerClassifier::new(&LayersConfig::default());

        assert_eq!(
            classifier.classify_import("github.com/example/app/internal/domain/user"),
            Some(ArchLayer::Domain)
        );
        assert_eq!(
            classifier.classify_import("github.com/example/app/internal/infrastructure/postgres"),
            Some(ArchLayer::Infrastructure)
        );
    }

    #[test]
    fn test_override_scoped_classification() {
        let config = config_with_overrides(vec![LayerOverrideConfig {
            scope: "services/auth/**".to_string(),
            domain: vec!["services/auth/core/**".to_string()],
            infrastructure: vec![
                "services/auth/server/**".to_string(),
                "services/auth/adapters/**".to_string(),
            ],
            application: vec![],
            presentation: vec![],
        }]);
        let classifier = LayerClassifier::new(&config);

        // Within scope: override patterns apply
        assert_eq!(
            classifier.classify("services/auth/core/user.go"),
            Some(ArchLayer::Domain)
        );
        assert_eq!(
            classifier.classify("services/auth/server/http.go"),
            Some(ArchLayer::Infrastructure)
        );
        assert_eq!(
            classifier.classify("services/auth/adapters/pg.go"),
            Some(ArchLayer::Infrastructure)
        );
    }

    #[test]
    fn test_paths_outside_override_use_global() {
        let config = config_with_overrides(vec![LayerOverrideConfig {
            scope: "services/auth/**".to_string(),
            domain: vec!["services/auth/core/**".to_string()],
            infrastructure: vec![],
            application: vec![],
            presentation: vec![],
        }]);
        let classifier = LayerClassifier::new(&config);

        // Outside scope: global patterns apply
        assert_eq!(
            classifier.classify("internal/domain/user/entity.go"),
            Some(ArchLayer::Domain)
        );
        assert_eq!(
            classifier.classify("internal/infrastructure/postgres/repo.go"),
            Some(ArchLayer::Infrastructure)
        );
    }

    #[test]
    fn test_override_omitted_layers_fall_back_to_global() {
        // Override only defines domain; application/infrastructure/presentation
        // should fall back to global defaults.
        let config = config_with_overrides(vec![LayerOverrideConfig {
            scope: "services/billing/**".to_string(),
            domain: vec!["services/billing/core/**".to_string()],
            application: vec![],
            infrastructure: vec![],
            presentation: vec![],
        }]);
        let classifier = LayerClassifier::new(&config);

        // domain uses override pattern
        assert_eq!(
            classifier.classify("services/billing/core/invoice.go"),
            Some(ArchLayer::Domain)
        );
        // infrastructure falls back to global pattern
        assert_eq!(
            classifier.classify("services/billing/infrastructure/stripe.go"),
            Some(ArchLayer::Infrastructure)
        );
    }

    #[test]
    fn test_first_matching_override_wins() {
        let config = config_with_overrides(vec![
            LayerOverrideConfig {
                scope: "services/auth/**".to_string(),
                domain: vec!["services/auth/core/**".to_string()],
                infrastructure: vec![],
                application: vec![],
                presentation: vec![],
            },
            LayerOverrideConfig {
                scope: "services/**".to_string(),
                domain: vec!["services/*/models/**".to_string()],
                infrastructure: vec![],
                application: vec![],
                presentation: vec![],
            },
        ]);
        let classifier = LayerClassifier::new(&config);

        // First override matches services/auth/**, so its domain pattern is used
        assert_eq!(
            classifier.classify("services/auth/core/user.go"),
            Some(ArchLayer::Domain)
        );
        // The second override's pattern would NOT match this because first wins
        assert_eq!(
            classifier.classify("services/auth/models/user.go"),
            None // Not domain because first override's domain is core/**
        );
    }

    #[test]
    fn test_import_classification_respects_overrides() {
        let config = config_with_overrides(vec![LayerOverrideConfig {
            scope: "services/auth/**".to_string(),
            domain: vec!["services/auth/core/**".to_string()],
            infrastructure: vec![],
            application: vec![],
            presentation: vec![],
        }]);
        let classifier = LayerClassifier::new(&config);

        assert_eq!(
            classifier.classify_import("services/auth/core/user"),
            Some(ArchLayer::Domain)
        );
    }
}
