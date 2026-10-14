use anyhow::{anyhow, Context, Result};
use std::fs;
use std::path::{Component, Path, PathBuf};

/// Enforces that file access stays inside one canonical project root.
#[derive(Debug, Clone)]
pub struct WorkspaceGuard {
    root: PathBuf,
    max_read_bytes: u64,
}

impl WorkspaceGuard {
    pub fn new(root: impl AsRef<Path>) -> Result<Self> {
        let root = fs::canonicalize(root.as_ref())
            .with_context(|| format!("resolve workspace {}", root.as_ref().display()))?;
        if !root.is_dir() {
            return Err(anyhow!("workspace root is not a directory"));
        }
        Ok(Self {
            root,
            max_read_bytes: 1_000_000,
        })
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Resolve an existing relative path and reject absolute paths and traversal.
    pub fn existing(&self, relative: &str) -> Result<PathBuf> {
        let input = Path::new(relative);
        if input.is_absolute()
            || input
                .components()
                .any(|c| matches!(c, Component::ParentDir | Component::Prefix(_)))
        {
            return Err(anyhow!(
                "path must be relative to the workspace and cannot traverse parents"
            ));
        }
        let joined = self.root.join(input);
        let canonical = fs::canonicalize(&joined)
            .with_context(|| format!("resolve workspace path {relative:?}"))?;
        self.ensure_inside(&canonical)?;
        Ok(canonical)
    }

    /// Resolve a destination whose leaf may not exist; its parent must exist.
    pub fn destination(&self, relative: &str) -> Result<PathBuf> {
        let input = Path::new(relative);
        if input.is_absolute()
            || input
                .components()
                .any(|c| matches!(c, Component::ParentDir | Component::Prefix(_)))
        {
            return Err(anyhow!(
                "path must be relative to the workspace and cannot traverse parents"
            ));
        }
        let joined = self.root.join(input);
        if joined.exists() {
            let canonical = fs::canonicalize(&joined)?;
            self.ensure_inside(&canonical)?;
            return Ok(canonical);
        }
        let parent = joined
            .parent()
            .ok_or_else(|| anyhow!("destination has no parent"))?;
        let canonical_parent = fs::canonicalize(parent)
            .with_context(|| format!("resolve destination parent {}", parent.display()))?;
        self.ensure_inside(&canonical_parent)?;
        let leaf = joined
            .file_name()
            .ok_or_else(|| anyhow!("destination has no file name"))?;
        Ok(canonical_parent.join(leaf))
    }

    pub fn read_text(&self, relative: &str) -> Result<String> {
        let path = self.existing(relative)?;
        let metadata = fs::metadata(&path)?;
        if !metadata.is_file() {
            return Err(anyhow!("path is not a file"));
        }
        if metadata.len() > self.max_read_bytes {
            return Err(anyhow!(
                "file exceeds the {} byte read limit",
                self.max_read_bytes
            ));
        }
        let bytes = fs::read(&path)?;
        String::from_utf8(bytes).map_err(|_| anyhow!("file is not valid UTF-8 text"))
    }

    fn ensure_inside(&self, path: &Path) -> Result<()> {
        if path.starts_with(&self.root) {
            Ok(())
        } else {
            Err(anyhow!("path resolves outside the workspace"))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_parent_traversal_and_absolute_paths() {
        let root = std::env::temp_dir();
        let guard = WorkspaceGuard::new(root).unwrap();
        assert!(guard.existing("../outside").is_err());
        assert!(guard.existing("/etc/passwd").is_err());
    }
}
