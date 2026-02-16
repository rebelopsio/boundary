use globset::{Glob, GlobSet, GlobSetBuilder};

use crate::config::LayersConfig;
use crate::types::ArchLayer;

/// Classifies file paths into architectural layers using glob patterns.
pub struct LayerClassifier {
    domain: GlobSet,
    application: GlobSet,
    infrastructure: GlobSet,
    presentation: GlobSet,
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
        Self {
            domain: build_globset(&config.domain),
            application: build_globset(&config.application),
            infrastructure: build_globset(&config.infrastructure),
            presentation: build_globset(&config.presentation),
        }
    }

    /// Classify a file path into an architectural layer.
    pub fn classify(&self, path: &str) -> Option<ArchLayer> {
        // Normalize path separators for matching
        let normalized = path.replace('\\', "/");
        if self.domain.is_match(&normalized) {
            Some(ArchLayer::Domain)
        } else if self.application.is_match(&normalized) {
            Some(ArchLayer::Application)
        } else if self.infrastructure.is_match(&normalized) {
            Some(ArchLayer::Infrastructure)
        } else if self.presentation.is_match(&normalized) {
            Some(ArchLayer::Presentation)
        } else {
            None
        }
    }

    /// Classify an import path string into an architectural layer.
    pub fn classify_import(&self, import_path: &str) -> Option<ArchLayer> {
        // Import paths use `/` separators; wrap in ** for matching
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
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::LayersConfig;

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
}
