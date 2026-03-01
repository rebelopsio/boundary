use globset::{Glob, GlobSet, GlobSetBuilder};

use crate::config::IgnoreRuleConfig;

/// Filters violations by rule ID + file path using glob patterns from config.
pub struct RuleIgnoreFilter {
    rules: Vec<(String, GlobSet)>,
}

impl RuleIgnoreFilter {
    pub fn new(ignores: &[IgnoreRuleConfig]) -> Self {
        let rules = ignores
            .iter()
            .filter_map(|entry| {
                let mut builder = GlobSetBuilder::new();
                for pattern in &entry.paths {
                    match Glob::new(pattern) {
                        Ok(glob) => {
                            builder.add(glob);
                        }
                        Err(e) => {
                            eprintln!(
                                "Warning: invalid glob pattern '{}' in rules.ignore: {e}",
                                pattern
                            );
                        }
                    }
                }
                match builder.build() {
                    Ok(globset) => Some((entry.rule.clone(), globset)),
                    Err(e) => {
                        eprintln!(
                            "Warning: failed to build glob set for rule '{}': {e}",
                            entry.rule
                        );
                        None
                    }
                }
            })
            .collect();
        Self { rules }
    }

    /// Returns true if the given rule ID + file path should be ignored.
    pub fn is_ignored(&self, rule_id: &str, file_path: &str) -> bool {
        self.rules
            .iter()
            .any(|(rule, globset)| rule == rule_id && globset.is_match(file_path))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ignore_filter_matches_rule_and_path() {
        let ignores = vec![IgnoreRuleConfig {
            rule: "PA001".to_string(),
            paths: vec!["infrastructure/**/*.go".to_string()],
        }];
        let filter = RuleIgnoreFilter::new(&ignores);
        assert!(filter.is_ignored("PA001", "infrastructure/repo/store.go"));
    }

    #[test]
    fn test_ignore_filter_no_match_wrong_path() {
        let ignores = vec![IgnoreRuleConfig {
            rule: "PA001".to_string(),
            paths: vec!["infrastructure/**/*.go".to_string()],
        }];
        let filter = RuleIgnoreFilter::new(&ignores);
        assert!(!filter.is_ignored("PA001", "domain/model/user.go"));
    }

    #[test]
    fn test_ignore_filter_no_match_wrong_rule() {
        let ignores = vec![IgnoreRuleConfig {
            rule: "PA001".to_string(),
            paths: vec!["infrastructure/**/*.go".to_string()],
        }];
        let filter = RuleIgnoreFilter::new(&ignores);
        assert!(!filter.is_ignored("L001", "infrastructure/repo/store.go"));
    }

    #[test]
    fn test_ignore_filter_empty_config() {
        let filter = RuleIgnoreFilter::new(&[]);
        assert!(!filter.is_ignored("PA001", "anything.go"));
    }
}
