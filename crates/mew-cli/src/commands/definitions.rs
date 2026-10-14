use anyhow::{bail, Context, Result};
use mew_common::MewConfig;
use mew_provider::ProviderRegistry;
use mew_runtime::{AgentLimits, AgentProfile};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

const MAX_DEFINITION_BYTES: u64 = 64 * 1024;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AgentDefinition {
    pub id: String,
    #[serde(default)]
    pub model: Option<String>,
    #[serde(default)]
    pub system_prompt: String,
    #[serde(default)]
    pub skills: Vec<String>,
    #[serde(default)]
    pub max_steps: Option<usize>,
    #[serde(default)]
    pub context_tokens: Option<usize>,
    #[serde(default)]
    pub response_reserve: Option<usize>,
}

impl AgentDefinition {
    pub fn scaffold(id: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            model: None,
            system_prompt: "You are a focused workspace subagent. Follow the task, use available read-only tools, and report evidence and limitations.".into(),
            skills: vec!["review".into()],
            max_steps: Some(8),
            context_tokens: None,
            response_reserve: None,
        }
    }

    pub fn validate(&self) -> Result<()> {
        validate_id(&self.id, "agent")?;
        if self.system_prompt.trim().is_empty() {
            bail!("agent {} must define a non-empty system_prompt", self.id);
        }
        if self.skills.len() > 5 {
            bail!("agent {} may load at most five skills", self.id);
        }
        if self
            .max_steps
            .is_some_and(|value| value == 0 || value > 128)
        {
            bail!("agent {} max_steps must be between 1 and 128", self.id);
        }
        if self
            .context_tokens
            .is_some_and(|value| !(2_048..=2_000_000).contains(&value))
        {
            bail!(
                "agent {} context_tokens must be between 2048 and 2000000",
                self.id
            );
        }
        if self
            .response_reserve
            .is_some_and(|value| !(256..=128_000).contains(&value))
        {
            bail!(
                "agent {} response_reserve must be between 256 and 128000",
                self.id
            );
        }
        if let (Some(context), Some(reserve)) = (self.context_tokens, self.response_reserve) {
            if reserve >= context {
                bail!(
                    "agent {} response_reserve must be smaller than context_tokens",
                    self.id
                );
            }
        }
        Ok(())
    }
}

pub fn validate_id(value: &str, kind: &str) -> Result<()> {
    if value.is_empty()
        || value.len() > 64
        || !value.chars().all(|character| {
            character.is_ascii_alphanumeric() || matches!(character, '_' | '-' | '.')
        })
        || value == "."
        || value == ".."
    {
        bail!("invalid {kind} id {value:?}; use 1–64 ASCII letters, digits, '.', '_' or '-' ");
    }
    Ok(())
}

/// Return a direct workspace-owned definition directory, rejecting symlink escapes.
pub fn workspace_definition_dir(root: &Path, kind: &str, create: bool) -> Result<Option<PathBuf>> {
    if !matches!(kind, "agents" | "workflows") {
        bail!("unsupported definition directory {kind:?}");
    }
    let canonical_root = root.canonicalize()?;
    let mew_dir = canonical_root.join(".mew");
    if fs::symlink_metadata(&mew_dir).is_ok_and(|metadata| metadata.file_type().is_symlink()) {
        bail!("refusing to access definitions through a .mew symlink");
    }
    if !mew_dir.exists() {
        if !create {
            return Ok(None);
        }
        fs::create_dir(&mew_dir).with_context(|| format!("create {}", mew_dir.display()))?;
    }
    if !mew_dir.is_dir() {
        bail!("{} is not a directory", mew_dir.display());
    }
    let directory = mew_dir.join(kind);
    if fs::symlink_metadata(&directory).is_ok_and(|metadata| metadata.file_type().is_symlink()) {
        bail!("refusing to access {kind} through a symlink");
    }
    if !directory.exists() {
        if !create {
            return Ok(None);
        }
        fs::create_dir(&directory).with_context(|| format!("create {}", directory.display()))?;
    }
    if !directory.is_dir() {
        bail!("{} is not a directory", directory.display());
    }
    let canonical_directory = directory.canonicalize()?;
    if !canonical_directory.starts_with(&canonical_root) {
        bail!("{kind} directory resolves outside the workspace");
    }
    Ok(Some(canonical_directory))
}

pub fn load_agent_definitions(root: &Path) -> Result<Vec<AgentDefinition>> {
    let Some(directory) = workspace_definition_dir(root, "agents", false)? else {
        return Ok(Vec::new());
    };
    let mut definitions = Vec::new();
    for entry in fs::read_dir(&directory)? {
        let entry = entry?;
        let path = entry.path();
        if path.extension().and_then(|extension| extension.to_str()) != Some("toml") {
            continue;
        }
        if fs::symlink_metadata(&path)?.file_type().is_symlink() || !path.is_file() {
            bail!(
                "refusing to load non-regular agent definition {}",
                path.display()
            );
        }
        let stem = path
            .file_stem()
            .and_then(|value| value.to_str())
            .unwrap_or_default();
        validate_id(stem, "agent filename")?;
        let definition: AgentDefinition = read_toml(&path)?;
        definition
            .validate()
            .with_context(|| format!("validate {}", path.display()))?;
        if definition.id != stem {
            bail!(
                "agent definition id {:?} must match filename {:?}",
                definition.id,
                stem
            );
        }
        if matches!(definition.id.as_str(), "main" | "analyst" | "reviewer") {
            bail!(
                "agent id {:?} is reserved by the built-in workspace agent",
                definition.id
            );
        }
        definitions.push(definition);
    }
    definitions.sort_by(|left, right| left.id.cmp(&right.id));
    Ok(definitions)
}

pub fn read_toml<T: for<'de> Deserialize<'de>>(path: &Path) -> Result<T> {
    let metadata = fs::metadata(path).with_context(|| format!("inspect {}", path.display()))?;
    if metadata.len() > MAX_DEFINITION_BYTES {
        bail!(
            "definition {} exceeds {} bytes",
            path.display(),
            MAX_DEFINITION_BYTES
        );
    }
    let raw = fs::read_to_string(path).with_context(|| format!("read {}", path.display()))?;
    toml::from_str(&raw).with_context(|| format!("parse TOML {}", path.display()))
}

pub fn build_agent_profile(
    definition: &AgentDefinition,
    cfg: &MewConfig,
    providers: &ProviderRegistry,
    root: &Path,
    fallback_model: Option<&str>,
) -> Result<AgentProfile> {
    definition.validate()?;
    let model_ref = definition
        .model
        .as_deref()
        .or(fallback_model)
        .unwrap_or(&cfg.providers.active_model);
    let (provider_id, model_id) = ProviderRegistry::parse_model_ref(model_ref)?;
    let provider = providers.get(&provider_id)?;
    let mut profile = AgentProfile::new(&definition.id, provider.clone(), model_id.clone());
    if profile.provider.supports_tool_calls(&profile.model) {
        profile.tools = mew_core::read_only_workspace_tools(root)?;
    }
    profile.system_prompt = definition.system_prompt.clone();
    for name in &definition.skills {
        profile.skills.push(mew_skills::load(root, name)?);
    }
    let model_context = provider
        .models()
        .into_iter()
        .find(|model| model.id == model_id)
        .map(|model| model.context)
        .filter(|context| *context >= 2_048)
        .unwrap_or(32_000);
    let context_tokens = definition.context_tokens.unwrap_or(model_context);
    let response_reserve = definition
        .response_reserve
        .unwrap_or((model_context / 10).clamp(1_000, 4_000));
    if response_reserve >= context_tokens {
        bail!(
            "agent {} response_reserve must be smaller than its context window",
            definition.id
        );
    }
    profile.limits = AgentLimits {
        max_steps: definition.max_steps.unwrap_or(8),
        context_tokens,
        response_reserve,
    };
    Ok(profile)
}

pub fn create_definition_file(
    directory: &Path,
    id: &str,
    value: &impl Serialize,
) -> Result<PathBuf> {
    validate_id(id, "definition")?;
    fs::create_dir_all(directory)?;
    let path = directory.join(format!("{id}.toml"));
    let contents = toml::to_string_pretty(value)?;
    fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&path)
        .with_context(|| {
            format!(
                "create {} without overwriting an existing definition",
                path.display()
            )
        })?
        .write_all(contents.as_bytes())?;
    Ok(path)
}

use std::io::Write;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validates_agent_profiles_and_identifier_safety() {
        let definition = AgentDefinition::scaffold("security-reviewer");
        definition.validate().unwrap();
        assert!(validate_id("../outside", "agent").is_err());
        assert!(validate_id("", "agent").is_err());
    }
}
