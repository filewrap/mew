use anyhow::{anyhow, Result};
use async_trait::async_trait;
use futures_util::StreamExt;
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

    fn headers(&self, builder: reqwest::RequestBuilder) -> reqwest::RequestBuilder {
        if self.id == "openrouter" {
            builder
                .header("HTTP-Referer", &self.app_url)
                .header("X-Title", &self.app_title)
        } else {
            builder
        }
    }
}

#[async_trait]
impl Provider for OpenAiCompatibleProvider {
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
        let url = format!("{}/models", self.base_url.trim_end_matches('/'));

        let res = self
            .headers(Client::new().get(url).bearer_auth(api_key))
            .send()
            .await?;

        if !res.status().is_success() {
            let status = res.status();
            let text = res.text().await.unwrap_or_default();
            return Err(anyhow!("provider {} models error {}: {}", self.id, status, text));
        }

        let parsed: OpenAiModelsResponse = res.json().await?;
        let mut out = parsed
            .data
            .into_iter()
            .map(|m| ProviderModel {
                provider: self.id.clone(),
                id: m.id,
                context: 0,
                supports_tools: false,
                supports_vision: false,
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
            stream: Some(false),
        };

        let res = self
            .headers(Client::new().post(url).bearer_auth(api_key).json(&body))
            .send()
            .await?;

        if !res.status().is_success() {
            let status = res.status();
            let text = res.text().await.unwrap_or_default();
            return Err(anyhow!("provider {} error {}: {}", self.id, status, text));
        }

        let parsed: OpenAiChatResponse = res.json().await?;
        let text = parsed
            .choices
            .first()
            .map(|c| c.message.content.clone().unwrap_or_default())
            .unwrap_or_default();

        Ok(ChatResponse {
            text,
            provider: self.id.clone(),
            model: req.model,
            input_tokens: parsed.usage.as_ref().and_then(|u| u.prompt_tokens),
            output_tokens: parsed.usage.as_ref().and_then(|u| u.completion_tokens),
        })
    }

    async fn chat_stream(
        &self,
        req: ChatRequest,
        on_delta: &mut (dyn FnMut(&str) + Send),
    ) -> Result<ChatResponse> {
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
            stream: Some(true),
        };

        let res = self
            .headers(Client::new().post(url).bearer_auth(api_key).json(&body))
            .send()
            .await?;

        if !res.status().is_success() {
            let status = res.status();
            let text = res.text().await.unwrap_or_default();
            return Err(anyhow!("provider {} error {}: {}", self.id, status, text));
        }

        let mut stream = res.bytes_stream();
        let mut buf = String::new();
        let mut full = String::new();

        while let Some(chunk) = stream.next().await {
            let chunk = chunk?;
            let s = String::from_utf8_lossy(&chunk);
            buf.push_str(&s);

            while let Some(idx) = buf.find('\n') {
                let line = buf[..idx].trim().to_string();
                buf = buf[idx + 1..].to_string();

                if !line.starts_with("data:") {
                    continue;
                }

                let data = line.trim_start_matches("data:").trim();

                if data == "[DONE]" {
                    break;
                }

                if data.is_empty() {
                    continue;
                }

                if let Ok(parsed) = serde_json::from_str::<OpenAiStreamChunk>(data) {
                    for choice in parsed.choices {
                        if let Some(content) = choice.delta.content {
                            full.push_str(&content);
                            on_delta(&content);
                        }
                    }
                }
            }
        }

        Ok(ChatResponse {
            text: full,
            provider: self.id.clone(),
            model: req.model,
            input_tokens: None,
            output_tokens: None,
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
    #[serde(skip_serializing_if = "Option::is_none")]
    stream: Option<bool>,
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
    content: Option<String>,
}

#[derive(Debug, Deserialize)]
struct OpenAiUsage {
    prompt_tokens: Option<u64>,
    completion_tokens: Option<u64>,
}

#[derive(Debug, Deserialize)]
struct OpenAiModelsResponse {
    data: Vec<OpenAiModelItem>,
}

#[derive(Debug, Deserialize)]
struct OpenAiModelItem {
    id: String,
}

#[derive(Debug, Deserialize)]
struct OpenAiStreamChunk {
    choices: Vec<OpenAiStreamChoice>,
}

#[derive(Debug, Deserialize)]
struct OpenAiStreamChoice {
    delta: OpenAiStreamDelta,
}

#[derive(Debug, Deserialize)]
struct OpenAiStreamDelta {
    content: Option<String>,
}
