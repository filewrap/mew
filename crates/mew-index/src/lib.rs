use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

/// Snapshot of a project's structure and tooling, produced by [`sniff`].
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectInfo {
    pub git: bool,
    pub file_count: usize,
    pub languages: Vec<String>,
    pub package: String,
    pub instructions: Vec<String>,
    pub tree: String,
}

/// A single indexed file (persistent context index entry).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexEntry {
    pub path: String,
    pub ext: String,
    pub size: u64,
}

/// Persistent context index stored at `.mew/index.json`.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ProjectIndex {
    pub root: String,
    pub file_count: usize,
    pub files: Vec<IndexEntry>,
    pub languages: BTreeMap<String, usize>,
}

const SKIP_DIRS: &[&str] = &[
    ".git",
    "node_modules",
    "target",
    "dist",
    "build",
    ".next",
    ".mew",
    ".cache",
];

pub fn sniff(root: &Path) -> Result<ProjectInfo> {
    let git = root.join(".git").exists();
    let mut files: Vec<PathBuf> = Vec::new();
    collect(root, &mut files, 0)?;

    let languages = detect_languages(&files);
    let package = detect_package(root);
    let instructions = detect_instructions(root);
    let tree = render_tree(root, &files);

    Ok(ProjectInfo {
        git,
        file_count: files.len(),
        languages,
        package,
        instructions,
        tree,
    })
}

pub fn build_index(root: &Path) -> Result<ProjectIndex> {
    let mut files: Vec<PathBuf> = Vec::new();
    collect(root, &mut files, 0)?;

    let mut index = ProjectIndex {
        root: root.display().to_string(),
        file_count: files.len(),
        files: Vec::with_capacity(files.len()),
        languages: BTreeMap::new(),
    };

    for f in &files {
        let meta = fs::metadata(f).with_context(|| format!("stat {}", f.display()))?;
        let ext = f
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_string();
        index.files.push(IndexEntry {
            path: f.display().to_string(),
            ext: ext.clone(),
            size: meta.len(),
        });
        *index.languages.entry(ext).or_insert(0) += 1;
    }

    Ok(index)
}

pub fn save_index(root: &Path, index: &ProjectIndex) -> Result<()> {
    let dir = root.join(".mew");
    fs::create_dir_all(&dir)?;
    let raw = serde_json::to_string_pretty(index)?;
    fs::write(dir.join("index.json"), raw)?;
    Ok(())
}

pub fn load_index(root: &Path) -> Result<Option<ProjectIndex>> {
    let path = root.join(".mew").join("index.json");
    if !path.exists() {
        return Ok(None);
    }
    let raw = fs::read_to_string(path)?;
    let index = serde_json::from_str(&raw)?;
    Ok(Some(index))
}

/// Render the markdown `repo-map.md` body for a project.
pub fn render_repo_map(info: &ProjectInfo) -> String {
    format!(
        "# repo map\n\n\
         - path: `{path}`\n\
         - git: `{git}`\n\
         - languages: `{langs}`\n\
         - package: `{pkg}`\n\
         - instructions: `{instr}`\n\
         - files: `{count}`\n\n\
         ## tree\n\n```\n{tree}\n```\n",
        path = ".",
        git = info.git,
        langs = info.languages.join(", "),
        pkg = info.package,
        instr = info.instructions.join(", "),
        count = info.file_count,
        tree = info.tree,
    )
}

fn collect(root: &Path, files: &mut Vec<PathBuf>, depth: usize) -> Result<()> {
    if depth > 4 {
        return Ok(());
    }

    let entries = fs::read_dir(root)?;
    for entry in entries {
        let entry = entry?;
        let p = entry.path();
        let name = p.file_name().and_then(|s| s.to_str()).unwrap_or("");

        if p.is_dir() {
            if SKIP_DIRS.contains(&name) {
                continue;
            }
            collect(&p, files, depth + 1)?;
        } else {
            files.push(p);
        }
    }

    Ok(())
}

fn detect_languages(files: &[PathBuf]) -> Vec<String> {
    let mut found: Vec<&str> = Vec::new();

    let mut push_if = |ext: &str, lang: &'static str, found: &mut Vec<&str>| {
        if !found.contains(&lang)
            && files
                .iter()
                .any(|p| p.extension().map(|e| e == ext).unwrap_or(false))
        {
            found.push(lang);
        }
    };

    push_if("rs", "Rust", &mut found);
    push_if("ts", "TypeScript", &mut found);
    push_if("js", "JavaScript", &mut found);
    push_if("py", "Python", &mut found);
    push_if("go", "Go", &mut found);

    if found.is_empty() {
        vec!["unknown".to_string()]
    } else {
        found.into_iter().map(|s| s.to_string()).collect()
    }
}

fn detect_package(root: &Path) -> String {
    if root.join("Cargo.toml").exists() {
        "cargo".to_string()
    } else if root.join("pnpm-lock.yaml").exists() {
        "pnpm".to_string()
    } else if root.join("yarn.lock").exists() {
        "yarn".to_string()
    } else if root.join("package-lock.json").exists() || root.join("package.json").exists() {
        "npm".to_string()
    } else if root.join("pyproject.toml").exists() {
        "python".to_string()
    } else if root.join("go.mod").exists() {
        "go".to_string()
    } else {
        "unknown".to_string()
    }
}

fn detect_instructions(root: &Path) -> Vec<String> {
    let names = [
        "AGENT.md",
        "AGENTS.md",
        "CLAUDE.md",
        "GEMINI.md",
        "OPENAI.md",
        "CODEX.md",
        ".windsurfrules",
    ];

    let mut found = Vec::new();

    for name in names {
        if root.join(name).exists() {
            found.push(name.to_string());
        }
    }

    if root
        .join(".github")
        .join("copilot-instructions.md")
        .exists()
    {
        found.push(".github/copilot-instructions.md".to_string());
    }

    if root.join(".cursor").join("rules").exists() {
        found.push(".cursor/rules".to_string());
    }

    if found.is_empty() {
        found.push("none".to_string());
    }

    found
}

fn render_tree(root: &Path, files: &[PathBuf]) -> String {
    let mut lines = Vec::new();
    for f in files.iter().take(200) {
        let rel = f.strip_prefix(root).unwrap_or(f);
        let depth = rel.components().count().saturating_sub(1);
        lines.push(format!("{}{}", "  ".repeat(depth), rel.display()));
    }
    if files.len() > 200 {
        lines.push(format!("... ({} files total)", files.len()));
    }
    lines.join("\n")
}
