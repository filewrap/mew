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

    fn default_model(&self) -> &str {
        &self.default_model
    }

    fn models(&self) -> Vec<ProviderModel> {
        self.models.clone()
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

        let mut text = String::new();
        for msg in req.messages {
            if msg.role == "system" {
                text.push_str("System: ");
                text.push_str(&msg.content);
                text.push_str("\n\n");
            } else {
                text.push_str(&msg.content);
                text.push_str("\n\n");
            }
        }

        let body = GeminiRequest {
            contents: vec![GeminiContent {
                parts: vec![GeminiPart { text }],
            }],
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
}

#[derive(Debug, Serialize)]
struct GeminiRequest {
    contents: Vec<GeminiContent>,
    #[serde(rename = "generationConfig")]
    generation_config: GeminiGenerationConfig,
}

#[derive(Debug, Serialize, Deserialize)]
struct GeminiContent {
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
