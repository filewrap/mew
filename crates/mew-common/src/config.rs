use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use crate::paths::MewPaths;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MewConfig {
    #[serde(default)]
    pub setup_complete: bool,
    #[serde(default)]
    pub trusted_workspaces: Vec<String>,
    #[serde(default)]
    pub tui: TuiConfig,
    pub identity: IdentityConfig,
    pub style: StyleConfig,
    pub agent: AgentConfig,
    pub tokens: TokenConfig,
    pub providers: ProviderConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct TuiConfig {
    pub favorite_models: Vec<String>,
    pub recent_models: Vec<String>,
    /// Selected generation preset per provider/model reference.
    pub model_variants: BTreeMap<String, String>,
    /// Key chords keyed by stable TUI action id.
    pub keybindings: BTreeMap<String, String>,
}

impl Default for TuiConfig {
    fn default() -> Self {
        Self {
            favorite_models: Vec::new(),
            recent_models: Vec::new(),
            model_variants: BTreeMap::new(),
            keybindings: BTreeMap::from([
                ("command_palette".into(), "ctrl+p,f1".into()),
                ("submit".into(), "enter".into()),
                ("cancel".into(), "ctrl+c".into()),
                ("eof".into(), "ctrl+d".into()),
                ("move_home".into(), "ctrl+a".into()),
                ("move_end".into(), "ctrl+e".into()),
                ("clear_line".into(), "ctrl+u".into()),
                ("delete_word".into(), "ctrl+w".into()),
                ("history_previous".into(), "up".into()),
                ("history_next".into(), "down".into()),
                ("complete".into(), "tab,f2".into()),
                ("transcript_up".into(), "pageup".into()),
                ("transcript_down".into(), "pagedown".into()),
            ]),
        }
    }
}

impl TuiConfig {
    pub fn is_favorite(&self, model_ref: &str) -> bool {
        self.favorite_models.iter().any(|item| item == model_ref)
    }

    pub fn toggle_favorite(&mut self, model_ref: &str) -> bool {
        if let Some(index) = self
            .favorite_models
            .iter()
            .position(|item| item == model_ref)
        {
            self.favorite_models.remove(index);
            false
        } else {
            self.favorite_models.push(model_ref.to_string());
            self.favorite_models.sort();
            true
        }
    }

    pub fn record_model(&mut self, model_ref: &str) {
        self.recent_models.retain(|item| item != model_ref);
        self.recent_models.insert(0, model_ref.to_string());
        self.recent_models.truncate(20);
    }

    pub fn model_variant(&self, model_ref: &str) -> &str {
        self.model_variants
            .get(model_ref)
            .map(String::as_str)
            .unwrap_or("balanced")
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IdentityConfig {
    pub display_name: String,
    pub persona: String,
    pub rename_enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StyleConfig {
    pub theme: String,
    pub animations: bool,
    pub emoji: bool,
    pub kaomoji: bool,
    pub density: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentConfig {
    pub default_mode: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenConfig {
    pub mode: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderConfig {
    pub default: String,
    pub default_model: String,
    pub active_model: String,
    pub openai: OpenAiProviderConfig,
    pub openrouter: OpenRouterProviderConfig,
    pub gemini: GeminiProviderConfig,
    pub custom: BTreeMap<String, CustomProviderConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenAiProviderConfig {
    pub enabled: bool,
    pub api_key_env: String,
    pub base_url: String,
    pub default_model: String,
    pub models: Vec<ModelConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenRouterProviderConfig {
    pub enabled: bool,
    pub api_key_env: String,
    pub base_url: String,
    pub default_model: String,
    pub models: Vec<ModelConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeminiProviderConfig {
    pub enabled: bool,
    pub api_key_env: String,
    pub base_url: String,
    pub default_model: String,
    pub models: Vec<ModelConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CustomProviderConfig {
    pub enabled: bool,
    pub kind: String,
    pub api_key_env: String,
    pub base_url: String,
    pub default_model: String,
    pub models: Vec<ModelConfig>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct ModelConfig {
    pub id: String,
    pub context: usize,
    pub supports_tools: bool,
    pub supports_vision: bool,
    pub notes: String,
}

impl Default for MewConfig {
    fn default() -> Self {
        Self {
            setup_complete: false,
            trusted_workspaces: Vec::new(),
            tui: TuiConfig::default(),
            identity: IdentityConfig {
                display_name: "mew".to_string(),
                persona: "cute".to_string(),
                rename_enabled: true,
            },
            style: StyleConfig {
                theme: "mew-dark".to_string(),
                animations: true,
                emoji: true,
                kaomoji: true,
                density: "cozy".to_string(),
            },
            agent: AgentConfig {
                default_mode: "ask".to_string(),
            },
            tokens: TokenConfig {
                mode: "balanced".to_string(),
            },
            providers: ProviderConfig::default(),
        }
    }
}

impl Default for ProviderConfig {
    fn default() -> Self {
        Self {
            default: "openai".to_string(),
            default_model: "codex-mini-latest".to_string(),
            active_model: "openai/codex-mini-latest".to_string(),
            openai: OpenAiProviderConfig {
                enabled: true,
                api_key_env: "OPENAI_API_KEY".to_string(),
                base_url: "https://api.openai.com/v1".to_string(),
                default_model: "codex-mini-latest".to_string(),
                models: vec![
                    ModelConfig {
                        id: "codex-mini-latest".to_string(),
                        context: 200_000,
                        supports_tools: true,
                        supports_vision: false,
                        notes: "default coding model".to_string(),
                    },
                    ModelConfig {
                        id: "gpt-4.1".to_string(),
                        context: 1_000_000,
                        supports_tools: true,
                        supports_vision: true,
                        notes: "large general coding model".to_string(),
                    },
                    ModelConfig {
                        id: "gpt-4.1-mini".to_string(),
                        context: 1_000_000,
                        supports_tools: true,
                        supports_vision: true,
                        notes: "fast balanced model".to_string(),
                    },
                ],
            },
            openrouter: OpenRouterProviderConfig {
                enabled: true,
                api_key_env: "OPENROUTER_API_KEY".to_string(),
                base_url: "https://openrouter.ai/api/v1".to_string(),
                default_model: "qwen/qwen-2.5-coder-32b-instruct".to_string(),
                models: vec![
                    ModelConfig {
                        id: "qwen/qwen-2.5-coder-32b-instruct".to_string(),
                        context: 32_768,
                        supports_tools: true,
                        supports_vision: false,
                        notes: "cheap strong coder".to_string(),
                    },
                    ModelConfig {
                        id: "deepseek/deepseek-chat".to_string(),
                        context: 64_000,
                        supports_tools: true,
                        supports_vision: false,
                        notes: "general coder".to_string(),
                    },
                    ModelConfig {
                        id: "google/gemini-2.5-flash".to_string(),
                        context: 1_000_000,
                        supports_tools: true,
                        supports_vision: true,
                        notes: "fast long-context via OpenRouter".to_string(),
                    },
                ],
            },
            gemini: GeminiProviderConfig {
                enabled: true,
                api_key_env: "GEMINI_API_KEY".to_string(),
                base_url: "https://generativelanguage.googleapis.com/v1beta".to_string(),
                default_model: "gemini-2.5-flash".to_string(),
                models: vec![
                    ModelConfig {
                        id: "gemini-2.5-flash".to_string(),
                        context: 1_000_000,
                        supports_tools: true,
                        supports_vision: true,
                        notes: "fast long-context default".to_string(),
                    },
                    ModelConfig {
                        id: "gemini-2.5-pro".to_string(),
                        context: 1_000_000,
                        supports_tools: true,
                        supports_vision: true,
                        notes: "deep reasoning long-context".to_string(),
                    },
                ],
            },
            custom: BTreeMap::new(),
        }
    }
}

impl MewConfig {
    /// Whether the exact canonical workspace path was explicitly trusted.
    pub fn workspace_is_trusted(&self, canonical_path: &Path) -> bool {
        let path = canonical_path.to_string_lossy();
        self.trusted_workspaces
            .iter()
            .any(|trusted| trusted == path.as_ref())
    }

    /// Remember an explicitly trusted canonical workspace without widening it
    /// to sibling or parent directories.
    pub fn trust_workspace(&mut self, canonical_path: &Path) {
        let path = canonical_path.to_string_lossy().into_owned();
        if !self
            .trusted_workspaces
            .iter()
            .any(|trusted| trusted == &path)
        {
            self.trusted_workspaces.push(path);
            self.trusted_workspaces.sort();
        }
    }

    pub fn load_or_create(paths: &MewPaths) -> Result<Self> {
        if !paths.config_file.exists() {
            let cfg = Self::default();
            cfg.save(paths)?;
            return Ok(cfg);
        }

        let raw = fs::read_to_string(&paths.config_file)?;
        match toml::from_str::<Self>(&raw) {
            Ok(cfg) => Ok(cfg),
            Err(e) => {
                eprintln!(
                    "mew: warning: could not parse config ({}); using defaults for this run. \
                     Fix or delete {}",
                    e,
                    paths.config_file.display()
                );
                Ok(Self::default())
            }
        }
    }

    pub fn save(&self, paths: &MewPaths) -> Result<()> {
        fs::create_dir_all(&paths.config_dir)?;
        let raw = toml::to_string_pretty(self)?;
        fs::write(&paths.config_file, raw)?;
        Ok(())
    }
}

#[cfg(test)]
mod model_config_compatibility_tests {
    use super::ModelConfig;

    #[test]
    fn legacy_model_config_defaults_tool_calls_to_disabled() {
        let model: ModelConfig = toml::from_str(
            r#"id = "legacy-model"
context = 16384
supports_vision = false
notes = "legacy config"
"#,
        )
        .unwrap();
        assert_eq!(model.id, "legacy-model");
        assert!(!model.supports_tools);
    }

    #[test]
    fn old_full_configs_load_as_not_yet_set_up() {
        let legacy = toml::to_string(&super::MewConfig::default())
            .unwrap()
            .replace("setup_complete = false\n", "");
        let loaded: super::MewConfig = toml::from_str(&legacy).unwrap();
        assert!(!loaded.setup_complete);
        assert!(loaded.trusted_workspaces.is_empty());
    }
}
