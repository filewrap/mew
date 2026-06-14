use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs;

use crate::paths::MewPaths;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MewConfig {
    pub identity: IdentityConfig,
    pub style: StyleConfig,
    pub agent: AgentConfig,
    pub tokens: TokenConfig,
    pub providers: ProviderConfig,
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

#[derive(Debug, Clone, Serialize, Deserialize)]
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
            identity: IdentityConfig {
                display_name: "mew".to_string(),
                persona: "cute".to_string(),
                rename_enabled: true,
            },
            style: StyleConfig {
                theme: "crush-catppuccin".to_string(),
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
                        supports_tools: false,
                        supports_vision: true,
                        notes: "fast long-context default".to_string(),
                    },
                    ModelConfig {
                        id: "gemini-2.5-pro".to_string(),
                        context: 1_000_000,
                        supports_tools: false,
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
    pub fn load_or_create(paths: &MewPaths) -> Result<Self> {
        if !paths.config_file.exists() {
            let cfg = Self::default();
            cfg.save(paths)?;
            return Ok(cfg);
        }

        let raw = fs::read_to_string(&paths.config_file)?;
        let cfg: Self = toml::from_str(&raw).unwrap_or_else(|_| Self::default());
        cfg.save(paths)?;
        Ok(cfg)
    }

    pub fn save(&self, paths: &MewPaths) -> Result<()> {
        fs::create_dir_all(&paths.config_dir)?;
        let raw = toml::to_string_pretty(self)?;
        fs::write(&paths.config_file, raw)?;
        Ok(())
    }
}
