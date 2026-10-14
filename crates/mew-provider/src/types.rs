use anyhow::Result;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    pub role: String,
    pub content: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tool_calls: Vec<ToolCall>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool_call_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCall {
    pub id: String,
    pub name: String,
    /// JSON-encoded arguments returned by the provider.
    pub arguments: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolSpec {
    pub name: String,
    pub description: String,
    pub parameters: Value,
}

#[derive(Debug, Clone)]
pub struct ChatRequest {
    pub model: String,
    pub messages: Vec<ChatMessage>,
    pub temperature: Option<f32>,
    pub max_tokens: Option<u32>,
    pub tools: Vec<ToolSpec>,
}

#[derive(Debug, Clone)]
pub struct ChatResponse {
    pub text: String,
    pub provider: String,
    pub model: String,
    pub input_tokens: Option<u64>,
    pub output_tokens: Option<u64>,
    pub tool_calls: Vec<ToolCall>,
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
    pub authorized: bool,
    pub auth: String,
    pub base_url: String,
    pub default_model: String,
}

#[async_trait]
pub trait Provider: Send + Sync {
    fn id(&self) -> &str;
    fn api_key_env(&self) -> &str;
    fn default_model(&self) -> &str;
    fn models(&self) -> Vec<ProviderModel>;

    fn is_authorized(&self) -> bool {
        std::env::var(self.api_key_env())
            .map(|v| !v.trim().is_empty())
            .unwrap_or(false)
    }

    fn supports_tool_calls(&self, model: &str) -> bool {
        self.models()
            .iter()
            .any(|m| m.id == model && m.supports_tools)
    }

    async fn list_remote_models(&self) -> Result<Vec<ProviderModel>> {
        Ok(self.models())
    }

    async fn chat(&self, req: ChatRequest) -> Result<ChatResponse>;

    async fn chat_stream(
        &self,
        req: ChatRequest,
        on_delta: &mut (dyn FnMut(String) + Send),
    ) -> Result<ChatResponse> {
        let res = self.chat(req).await?;
        on_delta(res.text.clone());
        Ok(res)
    }

    async fn test(&self) -> Result<()>;
}

pub fn user_message(content: impl Into<String>) -> ChatMessage {
    ChatMessage {
        role: "user".to_string(),
        content: content.into(),
        tool_calls: Vec::new(),
        tool_call_id: None,
        name: None,
    }
}

pub fn system_message(content: impl Into<String>) -> ChatMessage {
    ChatMessage {
        role: "system".to_string(),
        content: content.into(),
        tool_calls: Vec::new(),
        tool_call_id: None,
        name: None,
    }
}

pub fn assistant_message(content: impl Into<String>) -> ChatMessage {
    ChatMessage {
        role: "assistant".to_string(),
        content: content.into(),
        tool_calls: Vec::new(),
        tool_call_id: None,
        name: None,
    }
}
