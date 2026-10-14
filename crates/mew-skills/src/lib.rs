use anyhow::{anyhow, Result};
use std::fs;
use std::path::Path;

#[derive(Debug, Clone)]
pub struct Skill {
    pub name: String,
    pub description: String,
    pub body: String,
    pub source: String,
}

const BUILTINS: &[(&str, &str, &str)] = &[
    ("caveman", "Use compact, plain language and avoid unnecessary tokens.", "Prefer short direct sentences. Keep code explanations concise, but never omit safety or correctness details."),
    ("review", "Review for correctness, regressions, security, and missing tests.", "Prioritize actionable findings by severity. Cite file paths and line numbers. Do not rewrite code unless asked."),
    ("tests", "Design focused regression tests.", "Test boundary conditions and failure paths. Prefer deterministic, hermetic tests and explain gaps."),
];

pub fn list(root: &Path) -> Result<Vec<Skill>> {
    let mut out = BUILTINS
        .iter()
        .map(|(name, description, body)| Skill {
            name: (*name).into(),
            description: (*description).into(),
            body: (*body).into(),
            source: "built-in".into(),
        })
        .collect::<Vec<_>>();
    let dir = root.join(".mew").join("skills");
    if !dir.exists() {
        return Ok(out);
    }
    let canonical_root = fs::canonicalize(root)?;
    let canonical_dir = fs::canonicalize(&dir)?;
    if !canonical_dir.starts_with(&canonical_root) {
        return Err(anyhow!("skills directory resolves outside workspace"));
    }
    for entry in fs::read_dir(&canonical_dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.extension().and_then(|s| s.to_str()) != Some("md") || !path.is_file() {
            continue;
        }
        let name = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("")
            .to_string();
        if !valid_name(&name) {
            continue;
        }
        let raw = fs::read_to_string(&path)?;
        if raw.len() > 64_000 {
            continue;
        }
        let (description, body) = parse_skill(&raw);
        out.retain(|s| s.name != name);
        out.push(Skill {
            name,
            description,
            body,
            source: "workspace".into(),
        });
    }
    out.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(out)
}

pub fn load(root: &Path, name: &str) -> Result<Skill> {
    if !valid_name(name) {
        return Err(anyhow!("invalid skill name"));
    }
    if let Some(skill) = list(root)?.into_iter().find(|s| s.name == name) {
        Ok(skill)
    } else {
        Err(anyhow!("unknown skill {name:?}"))
    }
}

fn parse_skill(raw: &str) -> (String, String) {
    let body = raw.trim().to_string();
    let description = raw
        .lines()
        .find_map(|line| {
            line.strip_prefix("description:")
                .map(str::trim)
                .map(str::to_string)
        })
        .or_else(|| {
            raw.lines()
                .find(|line| !line.trim().is_empty() && !line.starts_with('#'))
                .map(|s| s.trim().to_string())
        })
        .unwrap_or_else(|| "Workspace skill".to_string());
    let body = if body.starts_with("---") {
        body.splitn(3, "---")
            .nth(2)
            .unwrap_or(&body)
            .trim()
            .to_string()
    } else {
        body
    };
    (description, body)
}

fn valid_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 48
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn parses_description_and_body() {
        let (desc, body) = parse_skill("---\ndescription: compact answers\n---\nUse short words.");
        assert_eq!(desc, "compact answers");
        assert_eq!(body, "Use short words.");
    }
    #[test]
    fn rejects_path_like_skill_names() {
        assert!(!valid_name("../secret"));
    }
}
