use anyhow::{anyhow, Result};
use async_trait::async_trait;
use reqwest::Client;
use serde::{Deserialize, Serialize};

use crate::{ChatRequest, ChatResponse, Provider, ProviderModel};

#[derive(Debug, Clone)]
pub struct OpenAiCompatibleProvider {
    pub id: String,
    pub api_key_env: String,
    pub base_url: String,
    pub default_model: String,
    pub models: Vec<ProviderModel>,
    pub app_title: String,
    pub app_url: String,
}

impl OpenAiCompatibleProvider {
    fn api_key(&self) -> Result<String> {
        std::env::var(&self.api_key_env)
            .map_err(|_| anyhow!("missing env var {}", self.api_key_env))
    }
}

#[async_trait]
impl Provider for OpenAiCompatibleProvider {
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
        let url = format!("{}/chat/completions", self.base_url.trim_end_matches('/'));

        let body = OpenAiChatRequest {
            model: req.model.clone(),
            messages: req
                .messages
                .into_iter()
                .map(|m| OpenAiMessage {
                    role: m.role,
                    content: m.content,
                })
                .collect(),
            temperature: req.temperature,
            max_tokens: req.max_tokens,
        };

        let client = Client::new();
        let mut builder = client.post(url).bearer_auth(api_key).json(&body);

        if self.id == "openrouter" {
            builder = builder
                .header("HTTP-Referer", &self.app_url)
                .header("X-Title", &self.app_title);
        }

        let res = builder.send().await?;

        if !res.status().is_success() {
            let status = res.status();
            let text = res.text().await.unwrap_or_default();
            return Err(anyhow!("provider {} error {}: {}", self.id, status, text));
        }

        let parsed: OpenAiChatResponse = res.json().await?;
        let text = parsed
            .choices
            .first()
            .map(|c| c.message.content.clone())
            .unwrap_or_default();

        Ok(ChatResponse {
            text,
            provider: self.id.clone(),
            model: req.model,
            input_tokens: parsed.usage.as_ref().and_then(|u| u.prompt_tokens),
            output_tokens: parsed.usage.as_ref().and_then(|u| u.completion_tokens),
        })
    }
}

#[derive(Debug, Serialize)]
struct OpenAiChatRequest {
    model: String,
    messages: Vec<OpenAiMessage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    max_tokens: Option<u32>,
}

#[derive(Debug, Serialize)]
struct OpenAiMessage {
    role: String,
    content: String,
}

#[derive(Debug, Deserialize)]
struct OpenAiChatResponse {
    choices: Vec<OpenAiChoice>,
    usage: Option<OpenAiUsage>,
}

#[derive(Debug, Deserialize)]
struct OpenAiChoice {
    message: OpenAiResponseMessage,
}

#[derive(Debug, Deserialize)]
struct OpenAiResponseMessage {
    content: String,
}

#[derive(Debug, Deserialize)]
struct OpenAiUsage {
    prompt_tokens: Option<u64>,
    completion_tokens: Option<u64>,
}
