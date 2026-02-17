use std::collections::HashMap;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::types::{Component, Dependency};

/// Cache entry for a single file's analysis results.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CachedFileResult {
    pub hash: String,
    pub components: Vec<Component>,
    pub dependencies: Vec<Dependency>,
}

/// Analysis cache stored in `.boundary/cache.json`.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AnalysisCache {
    pub files: HashMap<String, CachedFileResult>,
}

const CACHE_DIR: &str = ".boundary";
const CACHE_FILE: &str = "cache.json";

impl AnalysisCache {
    pub fn new() -> Self {
        Self {
            files: HashMap::new(),
        }
    }

    /// Load cache from `.boundary/cache.json` relative to project root.
    pub fn load(project_root: &Path) -> Result<Self> {
        let cache_path = project_root.join(CACHE_DIR).join(CACHE_FILE);
        if !cache_path.exists() {
            return Ok(Self::new());
        }
        let content =
            std::fs::read_to_string(&cache_path).context("failed to read analysis cache")?;
        let cache: Self =
            serde_json::from_str(&content).context("failed to parse analysis cache")?;
        Ok(cache)
    }

    /// Save cache to `.boundary/cache.json` relative to project root.
    pub fn save(&self, project_root: &Path) -> Result<()> {
        let cache_dir = project_root.join(CACHE_DIR);
        std::fs::create_dir_all(&cache_dir).context("failed to create .boundary directory")?;
        let cache_path = cache_dir.join(CACHE_FILE);
        let content =
            serde_json::to_string_pretty(self).context("failed to serialize analysis cache")?;
        std::fs::write(&cache_path, content).context("failed to write analysis cache")?;
        Ok(())
    }

    /// Check if a file's cached result is stale (content changed).
    pub fn is_stale(&self, rel_path: &str, content: &str) -> bool {
        match self.files.get(rel_path) {
            Some(cached) => cached.hash != compute_hash(content),
            None => true, // Not in cache = stale
        }
    }

    /// Get cached result for a file if it exists and is current.
    pub fn get(&self, rel_path: &str, content: &str) -> Option<&CachedFileResult> {
        let cached = self.files.get(rel_path)?;
        if cached.hash == compute_hash(content) {
            Some(cached)
        } else {
            None
        }
    }

    /// Insert or update a file's cache entry.
    pub fn insert(&mut self, rel_path: String, content: &str, result: CachedFileResult) {
        let mut entry = result;
        entry.hash = compute_hash(content);
        self.files.insert(rel_path, entry);
    }

    /// Remove entries for files that no longer exist.
    pub fn prune(&mut self, existing_files: &[String]) {
        let existing_set: std::collections::HashSet<&str> =
            existing_files.iter().map(|s| s.as_str()).collect();
        self.files
            .retain(|path, _| existing_set.contains(path.as_str()));
    }

    /// Try to quickly identify changed files using `git diff --name-only`.
    /// Returns None if not in a git repo or git fails.
    pub fn git_changed_files(project_root: &Path) -> Option<Vec<PathBuf>> {
        let output = std::process::Command::new("git")
            .args(["diff", "--name-only", "HEAD"])
            .current_dir(project_root)
            .output()
            .ok()?;

        if !output.status.success() {
            return None;
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let files: Vec<PathBuf> = stdout
            .lines()
            .filter(|line| !line.is_empty())
            .map(PathBuf::from)
            .collect();

        // Also include untracked files
        let untracked = std::process::Command::new("git")
            .args(["ls-files", "--others", "--exclude-standard"])
            .current_dir(project_root)
            .output()
            .ok()?;

        if untracked.status.success() {
            let untracked_stdout = String::from_utf8_lossy(&untracked.stdout);
            let mut all_files = files;
            all_files.extend(
                untracked_stdout
                    .lines()
                    .filter(|line| !line.is_empty())
                    .map(PathBuf::from),
            );
            Some(all_files)
        } else {
            Some(files)
        }
    }
}

/// Compute SHA-256 hash of file content.
pub fn compute_hash(content: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(content.as_bytes());
    format!("{:x}", hasher.finalize())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::*;
    use std::path::PathBuf;

    #[test]
    fn test_compute_hash_deterministic() {
        let h1 = compute_hash("hello world");
        let h2 = compute_hash("hello world");
        assert_eq!(h1, h2);
    }

    #[test]
    fn test_compute_hash_different_content() {
        let h1 = compute_hash("hello");
        let h2 = compute_hash("world");
        assert_ne!(h1, h2);
    }

    #[test]
    fn test_cache_is_stale() {
        let mut cache = AnalysisCache::new();
        cache.files.insert(
            "test.go".to_string(),
            CachedFileResult {
                hash: compute_hash("original content"),
                components: vec![],
                dependencies: vec![],
            },
        );

        assert!(!cache.is_stale("test.go", "original content"));
        assert!(cache.is_stale("test.go", "modified content"));
        assert!(cache.is_stale("nonexistent.go", "anything"));
    }

    #[test]
    fn test_cache_get() {
        let mut cache = AnalysisCache::new();
        let component = Component {
            id: ComponentId::new("pkg", "Test"),
            name: "Test".to_string(),
            kind: ComponentKind::Entity(EntityInfo {
                name: "Test".to_string(),
                fields: vec![],
                methods: vec![],
            }),
            layer: None,
            location: SourceLocation {
                file: PathBuf::from("test.go"),
                line: 1,
                column: 1,
            },
        };

        cache.insert(
            "test.go".to_string(),
            "content",
            CachedFileResult {
                hash: String::new(), // will be overwritten
                components: vec![component],
                dependencies: vec![],
            },
        );

        let result = cache.get("test.go", "content");
        assert!(result.is_some());
        assert_eq!(result.unwrap().components.len(), 1);

        let result = cache.get("test.go", "changed");
        assert!(result.is_none());
    }

    #[test]
    fn test_cache_prune() {
        let mut cache = AnalysisCache::new();
        cache.files.insert(
            "a.go".to_string(),
            CachedFileResult {
                hash: "h1".to_string(),
                components: vec![],
                dependencies: vec![],
            },
        );
        cache.files.insert(
            "b.go".to_string(),
            CachedFileResult {
                hash: "h2".to_string(),
                components: vec![],
                dependencies: vec![],
            },
        );

        cache.prune(&["a.go".to_string()]);
        assert!(cache.files.contains_key("a.go"));
        assert!(!cache.files.contains_key("b.go"));
    }

    #[test]
    fn test_cache_save_and_load() {
        let dir = tempfile::tempdir().unwrap();
        let mut cache = AnalysisCache::new();
        cache.insert(
            "test.go".to_string(),
            "content",
            CachedFileResult {
                hash: String::new(),
                components: vec![],
                dependencies: vec![],
            },
        );

        cache.save(dir.path()).unwrap();
        let loaded = AnalysisCache::load(dir.path()).unwrap();
        assert_eq!(loaded.files.len(), 1);
        assert!(loaded.files.contains_key("test.go"));
    }
}
