use anyhow::{anyhow, Result};
use async_trait::async_trait;
use reqwest::Client;
use serde::{Deserialize, Serialize};

use crate::{ChatRequest, ChatResponse, Provider, ProviderModel};

#[derive(Debug, Clone)]
pub struct GeminiProvider {
    pub id: String,
    pub api_key_env: String,
    pub base_url: String,
    pub default_model: String,
    pub models: Vec<ProviderModel>,
}

impl GeminiProvider {
    fn api_key(&self) -> Result<String> {
        std::env::var(&self.api_key_env)
            .map_err(|_| anyhow!("missing env var {}", self.api_key_env))
    }
}

#[async_trait]
impl Provider for GeminiProvider {
    fn id(&self) -> &str {
        &self.id
    }

    fn api_key_env(&self) -> &str {
        &self.api_key_env
    }

    fn default_model(&self) -> &str {
        &self.default_model
    }

    fn models(&self) -> Vec<ProviderModel> {
        self.models.clone()
    }

    async fn list_remote_models(&self) -> Result<Vec<ProviderModel>> {
        let api_key = self.api_key()?;
        let url = format!(
            "{}/models?key={}",
            self.base_url.trim_end_matches('/'),
            api_key
        );

        let res = Client::new().get(url).send().await?;

        if !res.status().is_success() {
            let status = res.status();
            let text = res.text().await.unwrap_or_default();
            return Err(anyhow!("gemini models error {}: {}", status, text));
        }

        let parsed: GeminiModelsResponse = res.json().await?;
        let mut out = parsed
            .models
            .into_iter()
            .filter(|m| {
                m.supported_generation_methods
                    .iter()
                    .any(|x| x == "generateContent")
            })
            .map(|m| ProviderModel {
                provider: self.id.clone(),
                id: m.name.trim_start_matches("models/").to_string(),
                context: m.input_token_limit.unwrap_or(0),
                supports_tools: false,
                supports_vision: true,
                notes: "remote model".to_string(),
            })
            .collect::<Vec<_>>();

        out.sort_by(|a, b| a.id.cmp(&b.id));
        Ok(out)
    }

    async fn test(&self) -> Result<()> {
        let _ = self.api_key()?;
        Ok(())
    }

    async fn chat(&self, req: ChatRequest) -> Result<ChatResponse> {
        let api_key = self.api_key()?;
        let url = format!(
            "{}/models/{}:generateContent?key={}",
            self.base_url.trim_end_matches('/'),
            req.model,
            api_key
        );

        let mut contents: Vec<GeminiContent> = Vec::new();
        let mut system_parts: Vec<GeminiPart> = Vec::new();

        for msg in req.messages {
            match msg.role.as_str() {
                "system" => system_parts.push(GeminiPart {
                    text: msg.content,
                }),
                "assistant" => contents.push(GeminiContent {
                    role: "model".to_string(),
                    parts: vec![GeminiPart {
                        text: msg.content,
                    }],
                }),
                _ => contents.push(GeminiContent {
                    role: "user".to_string(),
                    parts: vec![GeminiPart {
                        text: msg.content,
                    }],
                }),
            }
        }

        let system_instruction = if system_parts.is_empty() {
            None
        } else {
            Some(GeminiContent {
                role: "user".to_string(),
                parts: system_parts,
            })
        };

        let body = GeminiRequest {
            contents,
            system_instruction,
            generation_config: GeminiGenerationConfig {
                temperature: req.temperature,
                max_output_tokens: req.max_tokens,
            },
        };

        let res = Client::new().post(url).json(&body).send().await?;

        if !res.status().is_success() {
            let status = res.status();
            let text = res.text().await.unwrap_or_default();
            return Err(anyhow!("gemini error {}: {}", status, text));
        }

        let parsed: GeminiResponse = res.json().await?;
        let text = parsed
            .candidates
            .first()
            .and_then(|c| c.content.parts.first())
            .map(|p| p.text.clone())
            .unwrap_or_default();

        Ok(ChatResponse {
            text,
            provider: self.id.clone(),
            model: req.model,
            input_tokens: None,
            output_tokens: None,
        })
    }

    async fn chat_stream(
        &self,
        req: ChatRequest,
        on_delta: &mut (dyn FnMut(String) + Send),
    ) -> Result<ChatResponse> {
        let res = self.chat(req).await?;
        let text = res.text.clone();

        for chunk in text.split_inclusive(['.', '\n']) {
            on_delta(chunk.to_string());
        }

        Ok(res)
    }
}

#[derive(Debug, Serialize)]
struct GeminiRequest {
    contents: Vec<GeminiContent>,
    #[serde(rename = "systemInstruction", skip_serializing_if = "Option::is_none")]
    system_instruction: Option<GeminiContent>,
    #[serde(rename = "generationConfig")]
    generation_config: GeminiGenerationConfig,
}

#[derive(Debug, Serialize, Deserialize)]
struct GeminiContent {
    role: String,
    parts: Vec<GeminiPart>,
}

#[derive(Debug, Serialize, Deserialize)]
struct GeminiPart {
    text: String,
}

#[derive(Debug, Serialize)]
struct GeminiGenerationConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    temperature: Option<f32>,
    #[serde(rename = "maxOutputTokens")]
    #[serde(skip_serializing_if = "Option::is_none")]
    max_output_tokens: Option<u32>,
}

#[derive(Debug, Deserialize)]
struct GeminiResponse {
    candidates: Vec<GeminiCandidate>,
}

#[derive(Debug, Deserialize)]
struct GeminiCandidate {
    content: GeminiContent,
}

#[derive(Debug, Deserialize)]
struct GeminiModelsResponse {
    models: Vec<GeminiModelItem>,
}

#[derive(Debug, Deserialize)]
struct GeminiModelItem {
    name: String,
    #[serde(default)]
    supported_generation_methods: Vec<String>,
    #[serde(rename = "inputTokenLimit")]
    input_token_limit: Option<usize>,
}
