use anyhow::Result;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    pub role: String,
    pub content: String,
}

#[derive(Debug, Clone)]
pub struct ChatRequest {
    pub model: String,
    pub messages: Vec<ChatMessage>,
    pub temperature: Option<f32>,
    pub max_tokens: Option<u32>,
}

#[derive(Debug, Clone)]
pub struct ChatResponse {
    pub text: String,
    pub provider: String,
    pub model: String,
    pub input_tokens: Option<u64>,
    pub output_tokens: Option<u64>,
}

#[derive(Debug, Clone)]
pub struct ProviderModel {
    pub provider: String,
    pub id: String,
    pub context: usize,
    pub supports_tools: bool,
    pub supports_vision: bool,
    pub notes: String,
}

#[derive(Debug, Clone)]
pub struct ProviderInfo {
    pub id: String,
    pub name: String,
    pub enabled: bool,
    pub auth: String,
    pub base_url: String,
    pub default_model: String,
}

#[async_trait]
pub trait Provider: Send + Sync {
    fn id(&self) -> &str;
    fn default_model(&self) -> &str;
    fn models(&self) -> Vec<ProviderModel>;
    async fn chat(&self, req: ChatRequest) -> Result<ChatResponse>;
    async fn test(&self) -> Result<()>;
}

pub fn user_message(content: impl Into<String>) -> ChatMessage {
    ChatMessage {
        role: "user".to_string(),
        content: content.into(),
    }
}

pub fn system_message(content: impl Into<String>) -> ChatMessage {
    ChatMessage {
        role: "system".to_string(),
        content: content.into(),
    }
}
