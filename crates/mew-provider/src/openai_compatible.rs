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

    /// Validate a candidate key and discover account models without persisting the key first.
    pub async fn list_remote_models_with_key(&self, api_key: &str) -> Result<Vec<ProviderModel>> {
        if api_key.trim().is_empty() {
            return Err(anyhow!("API key cannot be empty"));
        }
        let url = format!("{}/models", self.base_url.trim_end_matches('/'));
        let response = self
            .headers(
                Client::builder()
                    .timeout(std::time::Duration::from_secs(20))
                    .build()?
                    .get(url)
                    .bearer_auth(api_key),
            )
            .send()
            .await?;
        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(provider_http_error(&self.id, status, &body));
        }

        let parsed: OpenAiModelsResponse = response.json().await?;
        let mut models = parsed
            .data
            .into_iter()
            .map(|model| ProviderModel {
                provider: self.id.clone(),
                id: model.id.clone(),
                context: model.context_length.unwrap_or(0),
                supports_tools: model_supports_tools(&model.id),
                supports_vision: false,
                notes: "remote model".to_string(),
            })
            .collect::<Vec<_>>();
        models.sort_by(|left, right| left.id.cmp(&right.id));
        Ok(models)
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
        self.list_remote_models_with_key(&api_key).await
    }

    async fn test(&self) -> Result<()> {
        let api_key = self.api_key()?;
        self.list_remote_models_with_key(&api_key).await?;
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
                    tool_calls: if m.tool_calls.is_empty() {
                        None
                    } else {
                        Some(
                            m.tool_calls
                                .into_iter()
                                .map(|t| OpenAiToolCall {
                                    id: t.id,
                                    type_: "function".to_string(),
                                    function: OpenAiFunctionCall {
                                        name: t.name,
                                        arguments: t.arguments,
                                    },
                                })
                                .collect(),
                        )
                    },
                    tool_call_id: m.tool_call_id,
                    name: m.name,
                })
                .collect(),
            temperature: req.temperature,
            max_tokens: req.max_tokens,
            stream: Some(false),
            tools: if req.tools.is_empty() {
                None
            } else {
                Some(
                    req.tools
                        .into_iter()
                        .map(|t| OpenAiTool {
                            type_: "function".to_string(),
                            function: OpenAiFunctionDef {
                                name: t.name,
                                description: t.description,
                                parameters: t.parameters,
                            },
                        })
                        .collect(),
                )
            },
        };

        let res = self
            .headers(Client::new().post(url).bearer_auth(api_key).json(&body))
            .send()
            .await?;

        if !res.status().is_success() {
            let status = res.status();
            let text = res.text().await.unwrap_or_default();
            return Err(provider_http_error(&self.id, status, &text));
        }

        let parsed: OpenAiChatResponse = res.json().await?;
        let message = parsed.choices.first().map(|c| &c.message);
        let text = message.and_then(|m| m.content.clone()).unwrap_or_default();
        let tool_calls = message
            .and_then(|m| m.tool_calls.clone())
            .unwrap_or_default()
            .into_iter()
            .map(|t| crate::ToolCall {
                id: t.id,
                name: t.function.name,
                arguments: t.function.arguments,
            })
            .collect();

        Ok(ChatResponse {
            text,
            provider: self.id.clone(),
            model: req.model,
            input_tokens: parsed.usage.as_ref().and_then(|u| u.prompt_tokens),
            output_tokens: parsed.usage.as_ref().and_then(|u| u.completion_tokens),
            tool_calls,
        })
    }

    async fn chat_stream(
        &self,
        req: ChatRequest,
        on_delta: &mut (dyn FnMut(String) + Send),
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
                    tool_calls: if m.tool_calls.is_empty() {
                        None
                    } else {
                        Some(
                            m.tool_calls
                                .into_iter()
                                .map(|t| OpenAiToolCall {
                                    id: t.id,
                                    type_: "function".to_string(),
                                    function: OpenAiFunctionCall {
                                        name: t.name,
                                        arguments: t.arguments,
                                    },
                                })
                                .collect(),
                        )
                    },
                    tool_call_id: m.tool_call_id,
                    name: m.name,
                })
                .collect(),
            temperature: req.temperature,
            max_tokens: req.max_tokens,
            stream: Some(true),
            tools: if req.tools.is_empty() {
                None
            } else {
                Some(
                    req.tools
                        .into_iter()
                        .map(|t| OpenAiTool {
                            type_: "function".to_string(),
                            function: OpenAiFunctionDef {
                                name: t.name,
                                description: t.description,
                                parameters: t.parameters,
                            },
                        })
                        .collect(),
                )
            },
        };

        let res = self
            .headers(Client::new().post(url).bearer_auth(api_key).json(&body))
            .send()
            .await?;

        if !res.status().is_success() {
            let status = res.status();
            let text = res.text().await.unwrap_or_default();
            return Err(provider_http_error(&self.id, status, &text));
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
                            on_delta(content.clone());
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
            tool_calls: Vec::new(),
        })
    }
}

fn provider_http_error(
    provider: &str,
    status: reqwest::StatusCode,
    response_body: &str,
) -> anyhow::Error {
    if status == reqwest::StatusCode::UNAUTHORIZED {
        let explanation = if response_body.to_ascii_lowercase().contains("expired") {
            "the provider reports that this API key has expired"
        } else {
            "the API key may be invalid or expired"
        };
        return anyhow!(
            "provider {provider} authentication failed (HTTP 401): {explanation}. Run `mew setup` to enter and validate a replacement key; provider response details are hidden for credential safety"
        );
    }

    anyhow!("provider {provider} error {status}: {response_body}")
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
    #[serde(skip_serializing_if = "Option::is_none")]
    tools: Option<Vec<OpenAiTool>>,
}

#[derive(Debug, Serialize)]
struct OpenAiMessage {
    role: String,
    content: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    tool_calls: Option<Vec<OpenAiToolCall>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tool_call_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    name: Option<String>,
}

#[derive(Debug, Serialize)]
struct OpenAiTool {
    #[serde(rename = "type")]
    type_: String,
    function: OpenAiFunctionDef,
}

#[derive(Debug, Serialize)]
struct OpenAiFunctionDef {
    name: String,
    description: String,
    parameters: serde_json::Value,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
struct OpenAiToolCall {
    id: String,
    #[serde(rename = "type")]
    type_: String,
    function: OpenAiFunctionCall,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
struct OpenAiFunctionCall {
    name: String,
    arguments: String,
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
    #[serde(default)]
    tool_calls: Option<Vec<OpenAiToolCall>>,
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
    #[serde(default)]
    context_length: Option<usize>,
}

fn model_supports_tools(id: &str) -> bool {
    let id = id.to_ascii_lowercase();
    [
        "gpt-",
        "o1",
        "o3",
        "o4",
        "codex",
        "claude",
        "gemini",
        "deepseek",
        "qwen",
        "mistral",
        "command-r",
        "llama-3.1",
        "llama-3.3",
    ]
    .iter()
    .any(|prefix| id.starts_with(prefix) || id.contains(&format!("/{prefix}")))
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

#[cfg(test)]
mod tool_protocol_tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn serializes_function_definitions_and_tool_results() {
        let request = OpenAiChatRequest {
            model: "test-model".into(),
            messages: vec![OpenAiMessage {
                role: "tool".into(),
                content: "read complete".into(),
                tool_calls: None,
                tool_call_id: Some("call-1".into()),
                name: Some("read_file".into()),
            }],
            temperature: None,
            max_tokens: Some(512),
            stream: Some(false),
            tools: Some(vec![OpenAiTool {
                type_: "function".into(),
                function: OpenAiFunctionDef {
                    name: "read_file".into(),
                    description: "Read a workspace file".into(),
                    parameters: json!({"type":"object","properties":{"path":{"type":"string"}}}),
                },
            }]),
        };
        let value = serde_json::to_value(request).unwrap();
        assert_eq!(value["tools"][0]["type"], "function");
        assert_eq!(value["tools"][0]["function"]["name"], "read_file");
        assert_eq!(value["messages"][0]["tool_call_id"], "call-1");
        assert_eq!(value["messages"][0]["name"], "read_file");
    }

    #[test]
    fn decodes_native_function_call_responses() {
        let response: OpenAiChatResponse = serde_json::from_value(json!({
            "choices":[{"message":{"content":null,"tool_calls":[{
                "id":"call-7",
                "type":"function",
                "function":{"name":"read_file","arguments":"{\"path\":\"README.md\"}"}
            }]}}],
            "usage":{"prompt_tokens":11,"completion_tokens":3}
        }))
        .unwrap();
        let call = &response.choices[0].message.tool_calls.as_ref().unwrap()[0];
        assert_eq!(call.id, "call-7");
        assert_eq!(call.function.name, "read_file");
        assert_eq!(call.function.arguments, r#"{"path":"README.md"}"#);
        assert_eq!(response.usage.unwrap().prompt_tokens, Some(11));
    }

    #[tokio::test]
    async fn live_model_discovery_validates_the_candidate_key_and_reads_models() {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.unwrap();
            let mut request = vec![0; 4096];
            let count = socket.read(&mut request).await.unwrap();
            let request = String::from_utf8_lossy(&request[..count]);
            assert!(request
                .to_ascii_lowercase()
                .contains("authorization: bearer test-key"));
            let body = r#"{"data":[{"id":"gpt-4.1-mini","context_length":32768},{"id":"text-embedding-3-small"}]}"#;
            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                body.len(),
                body
            );
            socket.write_all(response.as_bytes()).await.unwrap();
        });
        let provider = OpenAiCompatibleProvider {
            id: "openai".into(),
            api_key_env: "UNUSED_TEST_KEY".into(),
            base_url: format!("http://{address}/v1"),
            default_model: "gpt-4.1-mini".into(),
            models: Vec::new(),
            app_title: "mew-test".into(),
            app_url: "http://localhost".into(),
        };
        let models = provider
            .list_remote_models_with_key("test-key")
            .await
            .unwrap();
        server.await.unwrap();
        assert_eq!(models.len(), 2);
        assert_eq!(models[0].id, "gpt-4.1-mini");
        assert_eq!(models[0].context, 32768);
        assert!(models[0].supports_tools);
    }

    #[test]
    fn unauthorized_provider_error_is_actionable_without_echoing_response_body() {
        let error = provider_http_error(
            "openrouter",
            reqwest::StatusCode::UNAUTHORIZED,
            r#"{"error":{"message":"API key expired; private-token-must-not-appear"}}"#,
        )
        .to_string();

        assert!(error.contains("openrouter"));
        assert!(error.contains("HTTP 401"));
        assert!(error.contains("API key has expired"));
        assert!(error.contains("mew setup"));
        assert!(error.contains("credential safety"));
        assert!(!error.contains("private-token-must-not-appear"));
    }

    #[tokio::test]
    async fn expired_key_model_discovery_shows_recovery_without_server_body() {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.unwrap();
            let mut request = vec![0; 4096];
            let _ = socket.read(&mut request).await.unwrap();
            let body = r#"{"error":{"message":"API key expired; private-token-must-not-appear"}}"#;
            let response = format!(
                "HTTP/1.1 401 Unauthorized\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                body.len(),
                body
            );
            socket.write_all(response.as_bytes()).await.unwrap();
        });
        let provider = OpenAiCompatibleProvider {
            id: "openrouter".into(),
            api_key_env: "UNUSED_TEST_KEY".into(),
            base_url: format!("http://{address}/v1"),
            default_model: "openrouter/free".into(),
            models: Vec::new(),
            app_title: "mew-test".into(),
            app_url: "http://localhost".into(),
        };

        let error = provider
            .list_remote_models_with_key("test-key")
            .await
            .unwrap_err()
            .to_string();
        server.await.unwrap();

        assert!(error.contains("authentication failed (HTTP 401)"));
        assert!(error.contains("API key has expired"));
        assert!(error.contains("mew setup"));
        assert!(!error.contains("private-token-must-not-appear"));
    }
}
