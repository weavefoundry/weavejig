//! `scoring.toml`: the pinned reference tools and corpus.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::Deserialize;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Manifest {
    pub references: BTreeMap<String, Reference>,
    #[serde(default)]
    pub corpus: Vec<Codebase>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "kebab-case")]
pub struct Reference {
    pub version: String,
    pub url: String,
    pub sha256: String,
    pub min_jdk: u32,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Codebase {
    pub name: String,
    pub url: String,
    pub rev: String,
    pub set: String,
}

/// The repository root: the directory that holds `scoring.toml`.
pub fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("xtask lives in the repo")
        .to_path_buf()
}

/// Where downloads go. Ignored by git.
pub fn cache_dir() -> PathBuf {
    repo_root().join(".weavejig")
}

impl Manifest {
    pub fn load() -> Result<Manifest> {
        let path = repo_root().join("scoring.toml");
        let text =
            fs::read_to_string(&path).with_context(|| format!("cannot read {}", path.display()))?;
        toml::from_str(&text).with_context(|| format!("invalid {}", path.display()))
    }

    pub fn reference(&self, name: &str) -> Result<&Reference> {
        self.references
            .get(name)
            .with_context(|| format!("scoring.toml has no [references.{name}] entry"))
    }

    pub fn codebases(&self, set: &str) -> Vec<&Codebase> {
        self.corpus
            .iter()
            .filter(|codebase| codebase.set == set)
            .collect()
    }
}

impl Reference {
    /// Where `cargo xtask fetch references` puts the jar.
    pub fn jar_path(&self) -> PathBuf {
        let file = self.url.rsplit('/').next().unwrap_or("reference.jar");
        cache_dir().join("reference").join(file)
    }
}

impl Codebase {
    pub fn checkout_dir(&self) -> PathBuf {
        cache_dir().join("corpus").join(&self.name)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn checked_in_manifest_is_valid() {
        let manifest = Manifest::load().unwrap();
        for name in ["google-java-format", "checkstyle"] {
            let reference = manifest.reference(name).unwrap();
            assert_eq!(reference.sha256.len(), 64, "{name}");
            assert!(reference.url.contains(&reference.version), "{name}");
            assert!(reference.url.starts_with("https://"), "{name}");
        }
        assert!(!manifest.codebases("dev").is_empty());
        assert!(!manifest.codebases("held-out").is_empty());
        for codebase in &manifest.corpus {
            // Pinned by full commit hash, never by a branch or tag.
            assert_eq!(codebase.rev.len(), 40, "{}", codebase.name);
            assert!(
                matches!(codebase.set.as_str(), "dev" | "held-out"),
                "{}",
                codebase.name
            );
        }
    }
}
