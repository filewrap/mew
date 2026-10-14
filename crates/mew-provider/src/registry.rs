use std::collections::BTreeMap;
use std::sync::Arc;

use anyhow::{anyhow, Result};
use mew_common::{CustomProviderConfig, MewConfig, ModelConfig};

use crate::{GeminiProvider, OpenAiCompatibleProvider, Provider, ProviderInfo, ProviderModel};

pub struct ProviderRegistry {
    providers: BTreeMap<String, Arc<dyn Provider>>,
    info: Vec<ProviderInfo>,
}

impl ProviderRegistry {
    pub fn from_config(cfg: &MewConfig) -> Self {
        let mut providers: BTreeMap<String, Arc<dyn Provider>> = BTreeMap::new();
        let mut info = Vec::new();

        if cfg.providers.openai.enabled {
            let models = convert_models("openai", &cfg.providers.openai.models);
            let provider = Arc::new(OpenAiCompatibleProvider {
                id: "openai".to_string(),
                api_key_env: cfg.providers.openai.api_key_env.clone(),
                base_url: cfg.providers.openai.base_url.clone(),
                default_model: cfg.providers.openai.default_model.clone(),
                models,
                app_title: "mew".to_string(),
                app_url: "https://github.com/filewrap/mew".to_string(),
            });

            info.push(ProviderInfo {
                id: "openai".to_string(),
                name: "OpenAI / Codex".to_string(),
                enabled: true,
                authorized: provider.is_authorized(),
                auth: format!("api-key env {}", cfg.providers.openai.api_key_env),
                base_url: cfg.providers.openai.base_url.clone(),
                default_model: cfg.providers.openai.default_model.clone(),
            });

            providers.insert("openai".to_string(), provider);
        }

        if cfg.providers.openrouter.enabled {
            let models = convert_models("openrouter", &cfg.providers.openrouter.models);
            let provider = Arc::new(OpenAiCompatibleProvider {
                id: "openrouter".to_string(),
                api_key_env: cfg.providers.openrouter.api_key_env.clone(),
                base_url: cfg.providers.openrouter.base_url.clone(),
                default_model: cfg.providers.openrouter.default_model.clone(),
                models,
                app_title: "mew".to_string(),
                app_url: "https://github.com/filewrap/mew".to_string(),
            });

            info.push(ProviderInfo {
                id: "openrouter".to_string(),
                name: "OpenRouter".to_string(),
                enabled: true,
                authorized: provider.is_authorized(),
                auth: format!("api-key env {}", cfg.providers.openrouter.api_key_env),
                base_url: cfg.providers.openrouter.base_url.clone(),
                default_model: cfg.providers.openrouter.default_model.clone(),
            });

            providers.insert("openrouter".to_string(), provider);
        }

        if cfg.providers.gemini.enabled {
            let models = convert_gemini_models("gemini", &cfg.providers.gemini.models);
            let provider = Arc::new(GeminiProvider {
                id: "gemini".to_string(),
                api_key_env: cfg.providers.gemini.api_key_env.clone(),
                base_url: cfg.providers.gemini.base_url.clone(),
                default_model: cfg.providers.gemini.default_model.clone(),
                models,
            });

            info.push(ProviderInfo {
                id: "gemini".to_string(),
                name: "Gemini".to_string(),
                enabled: true,
                authorized: provider.is_authorized(),
                auth: format!(
                    "API key env {} or Google Gemini OAuth (`mew provider login gemini`)",
                    cfg.providers.gemini.api_key_env
                ),
                base_url: cfg.providers.gemini.base_url.clone(),
                default_model: cfg.providers.gemini.default_model.clone(),
            });

            providers.insert("gemini".to_string(), provider);
        }

        for (id, custom) in &cfg.providers.custom {
            if custom.enabled && custom.kind == "openai-compatible" {
                add_custom_openai_compatible(id, custom, &mut providers, &mut info);
            }
        }

        Self { providers, info }
    }

    pub fn list_info(&self) -> Vec<ProviderInfo> {
        self.info.clone()
    }

    pub fn list_authorized_info(&self) -> Vec<ProviderInfo> {
        self.info
            .iter()
            .filter(|p| p.authorized)
            .cloned()
            .collect::<Vec<_>>()
    }

    pub fn list_models(&self) -> Vec<ProviderModel> {
        self.providers
            .values()
            .flat_map(|p| p.models())
            .collect::<Vec<_>>()
    }

    pub fn list_models_for(&self, provider: &str) -> Result<Vec<ProviderModel>> {
        Ok(self.get(provider)?.models())
    }

    pub async fn list_remote_models_for(&self, provider: &str) -> Result<Vec<ProviderModel>> {
        let p = self.get(provider)?;
        if !p.is_authorized() {
            return Err(anyhow!("provider {} is not authorized", provider));
        }
        p.list_remote_models().await
    }

    pub fn get(&self, id: &str) -> Result<Arc<dyn Provider>> {
        self.providers
            .get(id)
            .cloned()
            .ok_or_else(|| anyhow!("unknown provider: {}", id))
    }

    pub fn parse_model_ref(model_ref: &str) -> Result<(String, String)> {
        let mut parts = model_ref.splitn(2, '/');
        let provider = parts
            .next()
            .filter(|s| !s.is_empty())
            .ok_or_else(|| anyhow!("missing provider"))?;
        let model = parts
            .next()
            .filter(|s| !s.is_empty())
            .ok_or_else(|| anyhow!("missing model"))?;

        Ok((provider.to_string(), model.to_string()))
    }
}

fn convert_models(provider: &str, models: &[ModelConfig]) -> Vec<ProviderModel> {
    models
        .iter()
        .map(|m| ProviderModel {
            provider: provider.to_string(),
            id: m.id.clone(),
            context: m.context,
            supports_tools: m.supports_tools,
            supports_vision: m.supports_vision,
            notes: m.notes.clone(),
        })
        .collect()
}

fn convert_gemini_models(provider: &str, models: &[ModelConfig]) -> Vec<ProviderModel> {
    models
        .iter()
        .map(|model| ProviderModel {
            provider: provider.to_string(),
            id: model.id.clone(),
            context: model.context,
            supports_tools: model.supports_tools
                || matches!(model.id.as_str(), "gemini-2.5-flash" | "gemini-2.5-pro"),
            supports_vision: model.supports_vision,
            notes: model.notes.clone(),
        })
        .collect()
}

fn add_custom_openai_compatible(
    id: &str,
    custom: &CustomProviderConfig,
    providers: &mut BTreeMap<String, Arc<dyn Provider>>,
    info: &mut Vec<ProviderInfo>,
) {
    let models = convert_models(id, &custom.models);

    let provider = Arc::new(OpenAiCompatibleProvider {
        id: id.to_string(),
        api_key_env: custom.api_key_env.clone(),
        base_url: custom.base_url.clone(),
        default_model: custom.default_model.clone(),
        models,
        app_title: "mew".to_string(),
        app_url: "https://github.com/filewrap/mew".to_string(),
    });

    info.push(ProviderInfo {
        id: id.to_string(),
        name: id.to_string(),
        enabled: true,
        authorized: provider.is_authorized(),
        auth: format!("api-key env {}", custom.api_key_env),
        base_url: custom.base_url.clone(),
        default_model: custom.default_model.clone(),
    });

    providers.insert(id.to_string(), provider);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_registered_backend_exposes_configured_native_tool_capability() {
        let mut config = MewConfig::default();
        config.providers.gemini.models[0].supports_tools = false;
        config.providers.custom.insert(
            "local-compatible".into(),
            CustomProviderConfig {
                enabled: true,
                kind: "openai-compatible".into(),
                api_key_env: "LOCAL_TEST_KEY".into(),
                base_url: "http://localhost:1234/v1".into(),
                default_model: "local-tool-model".into(),
                models: vec![ModelConfig {
                    id: "local-tool-model".into(),
                    context: 16_384,
                    supports_tools: true,
                    supports_vision: false,
                    notes: "test".into(),
                }],
            },
        );
        let registry = ProviderRegistry::from_config(&config);
        for (provider_id, model_id) in [
            ("openai", "codex-mini-latest"),
            ("openrouter", "qwen/qwen-2.5-coder-32b-instruct"),
            ("gemini", "gemini-2.5-flash"),
            ("local-compatible", "local-tool-model"),
        ] {
            let provider = registry.get(provider_id).unwrap();
            assert!(
                provider.supports_tool_calls(model_id),
                "{provider_id}/{model_id} should support native tool calls"
            );
        }
    }
}
