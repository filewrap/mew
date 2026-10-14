use anyhow::{anyhow, Result};
use async_trait::async_trait;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::{
    gemini_oauth_access_token, has_gemini_oauth_token, ChatMessage, ChatRequest, ChatResponse,
    Provider, ProviderModel, ToolCall,
};

#[derive(Debug, Clone)]
pub struct GeminiProvider {
    pub id: String,
    pub api_key_env: String,
    pub base_url: String,
    pub default_model: String,
    pub models: Vec<ProviderModel>,
}

enum GeminiAuth {
    ApiKey(String),
    OAuth {
        access_token: String,
        project_id: String,
    },
}

impl GeminiProvider {
    fn api_key(&self) -> Option<String> {
        std::env::var(&self.api_key_env)
            .ok()
            .filter(|value| !value.trim().is_empty())
    }

    async fn auth(&self) -> Result<GeminiAuth> {
        if let Some(key) = self.api_key() {
            return Ok(GeminiAuth::ApiKey(key));
        }
        let (access_token, project_id) = gemini_oauth_access_token().await?;
        Ok(GeminiAuth::OAuth {
            access_token,
            project_id,
        })
    }

    fn endpoint(&self, path: &str, auth: &GeminiAuth) -> Result<String> {
        let mut url = reqwest::Url::parse(&format!(
            "{}/{}",
            self.base_url.trim_end_matches('/'),
            path.trim_start_matches('/')
        ))?;
        if let GeminiAuth::ApiKey(key) = auth {
            url.query_pairs_mut().append_pair("key", key);
        }
        Ok(url.into())
    }

    fn authorize(
        &self,
        request: reqwest::RequestBuilder,
        auth: &GeminiAuth,
    ) -> reqwest::RequestBuilder {
        match auth {
            GeminiAuth::ApiKey(_) => request,
            GeminiAuth::OAuth {
                access_token,
                project_id,
            } => request
                .bearer_auth(access_token)
                .header("x-goog-user-project", project_id),
        }
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

    fn is_authorized(&self) -> bool {
        self.api_key().is_some() || has_gemini_oauth_token()
    }

    fn default_model(&self) -> &str {
        &self.default_model
    }

    fn models(&self) -> Vec<ProviderModel> {
        self.models.clone()
    }

    async fn list_remote_models(&self) -> Result<Vec<ProviderModel>> {
        let auth = self.auth().await?;
        let url = self.endpoint("models", &auth)?;
        let res = self.authorize(Client::new().get(url), &auth).send().await?;
        if !res.status().is_success() {
            let status = res.status();
            let text = res.text().await.unwrap_or_default();
            return Err(anyhow!("gemini models error {}: {}", status, text));
        }
        let parsed: GeminiModelsResponse = res.json().await?;
        let mut out = parsed
            .models
            .into_iter()
            .filter(|model| {
                model
                    .supported_generation_methods
                    .iter()
                    .any(|method| method == "generateContent")
            })
            .map(|model| ProviderModel {
                provider: self.id.clone(),
                id: model.name.trim_start_matches("models/").to_string(),
                context: model.input_token_limit.unwrap_or(0),
                supports_tools: true,
                supports_vision: true,
                notes: "remote generateContent model; native function calling".to_string(),
            })
            .collect::<Vec<_>>();
        out.sort_by(|a, b| a.id.cmp(&b.id));
        Ok(out)
    }

    async fn test(&self) -> Result<()> {
        let auth = self.auth().await?;
        let url = self.endpoint("models", &auth)?;
        let response = self.authorize(Client::new().get(url), &auth).send().await?;
        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(anyhow!(
                "Gemini provider test failed ({status}): {}",
                body.chars().take(500).collect::<String>()
            ));
        }
        Ok(())
    }

    async fn chat(&self, req: ChatRequest) -> Result<ChatResponse> {
        let auth = self.auth().await?;
        let url = self.endpoint(&format!("models/{}:generateContent", req.model), &auth)?;
        let (contents, system_instruction) = convert_messages(req.messages)?;
        let body = GeminiRequest {
            contents,
            system_instruction,
            generation_config: GeminiGenerationConfig {
                temperature: req.temperature,
                max_output_tokens: req.max_tokens,
            },
            tools: if req.tools.is_empty() {
                None
            } else {
                Some(vec![GeminiTool {
                    function_declarations: req
                        .tools
                        .into_iter()
                        .map(|tool| GeminiFunctionDeclaration {
                            name: tool.name,
                            description: tool.description,
                            parameters: gemini_schema(tool.parameters),
                        })
                        .collect(),
                }])
            },
        };
        let res = self
            .authorize(Client::new().post(url), &auth)
            .json(&body)
            .send()
            .await?;
        if !res.status().is_success() {
            let status = res.status();
            let text = res.text().await.unwrap_or_default();
            return Err(anyhow!("gemini error {}: {}", status, text));
        }
        let parsed: GeminiResponse = res.json().await?;
        let candidate = parsed
            .candidates
            .first()
            .ok_or_else(|| anyhow!("gemini response did not contain a candidate"))?;
        let mut text_parts = Vec::new();
        let mut tool_calls = Vec::new();
        for part in &candidate.content.parts {
            if let Some(text) = &part.text {
                text_parts.push(text.as_str());
            }
            if let Some(call) = &part.function_call {
                tool_calls.push(ToolCall {
                    // GenerateContent functionCall does not include a call id.
                    id: format!("gemini-{}-{}", tool_calls.len(), call.name),
                    name: call.name.clone(),
                    arguments: call.args.to_string(),
                });
            }
        }
        Ok(ChatResponse {
            text: text_parts.join(""),
            provider: self.id.clone(),
            model: req.model,
            input_tokens: parsed
                .usage_metadata
                .as_ref()
                .and_then(|usage| usage.prompt_token_count),
            output_tokens: parsed
                .usage_metadata
                .as_ref()
                .and_then(|usage| usage.candidates_token_count),
            tool_calls,
        })
    }

    async fn chat_stream(
        &self,
        req: ChatRequest,
        on_delta: &mut (dyn FnMut(String) + Send),
    ) -> Result<ChatResponse> {
        let res = self.chat(req).await?;
        if !res.text.is_empty() {
            for chunk in res.text.split_inclusive(['.', '\n']) {
                on_delta(chunk.to_string());
            }
        }
        Ok(res)
    }
}

fn convert_messages(
    messages: Vec<ChatMessage>,
) -> Result<(Vec<GeminiContent>, Option<GeminiContent>)> {
    let mut contents = Vec::new();
    let mut system_parts = Vec::new();
    for message in messages {
        match message.role.as_str() {
            "system" => {
                if !message.content.is_empty() {
                    system_parts.push(GeminiPart::text(message.content));
                }
            }
            "assistant" => {
                let mut parts = Vec::new();
                if !message.content.is_empty() {
                    parts.push(GeminiPart::text(message.content));
                }
                for call in message.tool_calls {
                    let args: Value = serde_json::from_str(&call.arguments).map_err(|error| {
                        anyhow!("invalid function-call arguments for {}: {error}", call.name)
                    })?;
                    parts.push(GeminiPart {
                        text: None,
                        function_call: Some(GeminiFunctionCall {
                            name: call.name,
                            args,
                        }),
                        function_response: None,
                    });
                }
                if !parts.is_empty() {
                    contents.push(GeminiContent {
                        role: "model".to_string(),
                        parts,
                    });
                }
            }
            "tool" => {
                let name = message
                    .name
                    .or(message.tool_call_id)
                    .ok_or_else(|| anyhow!("function response is missing its function name"))?;
                let response = match serde_json::from_str::<Value>(&message.content) {
                    Ok(value) if value.is_object() => value,
                    _ => json!({ "result": message.content }),
                };
                contents.push(GeminiContent {
                    role: "user".to_string(),
                    parts: vec![GeminiPart {
                        text: None,
                        function_call: None,
                        function_response: Some(GeminiFunctionResponse { name, response }),
                    }],
                });
            }
            _ => {
                if !message.content.is_empty() {
                    contents.push(GeminiContent {
                        role: "user".to_string(),
                        parts: vec![GeminiPart::text(message.content)],
                    });
                }
            }
        }
    }
    let system_instruction = (!system_parts.is_empty()).then_some(GeminiContent {
        role: "user".to_string(),
        parts: system_parts,
    });
    Ok((contents, system_instruction))
}

fn gemini_schema(mut schema: Value) -> Value {
    fn uppercase_types(value: &mut Value) {
        match value {
            Value::Object(object) => {
                if let Some(Value::String(kind)) = object.get_mut("type") {
                    *kind = kind.to_ascii_uppercase();
                }
                for child in object.values_mut() {
                    uppercase_types(child);
                }
            }
            Value::Array(items) => items.iter_mut().for_each(uppercase_types),
            _ => {}
        }
    }
    uppercase_types(&mut schema);
    schema
}

#[derive(Debug, Serialize)]
struct GeminiRequest {
    contents: Vec<GeminiContent>,
    #[serde(rename = "systemInstruction", skip_serializing_if = "Option::is_none")]
    system_instruction: Option<GeminiContent>,
    #[serde(rename = "generationConfig")]
    generation_config: GeminiGenerationConfig,
    #[serde(skip_serializing_if = "Option::is_none")]
    tools: Option<Vec<GeminiTool>>,
}

#[derive(Debug, Serialize, Deserialize)]
struct GeminiContent {
    role: String,
    parts: Vec<GeminiPart>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GeminiPart {
    #[serde(skip_serializing_if = "Option::is_none")]
    text: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    function_call: Option<GeminiFunctionCall>,
    #[serde(skip_serializing_if = "Option::is_none")]
    function_response: Option<GeminiFunctionResponse>,
}

impl GeminiPart {
    fn text(text: String) -> Self {
        Self {
            text: Some(text),
            function_call: None,
            function_response: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct GeminiFunctionCall {
    name: String,
    args: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct GeminiFunctionResponse {
    name: String,
    response: Value,
}

#[derive(Debug, Serialize)]
struct GeminiTool {
    #[serde(rename = "functionDeclarations")]
    function_declarations: Vec<GeminiFunctionDeclaration>,
}

#[derive(Debug, Serialize)]
struct GeminiFunctionDeclaration {
    name: String,
    description: String,
    parameters: Value,
}

#[derive(Debug, Serialize)]
struct GeminiGenerationConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    temperature: Option<f32>,
    #[serde(rename = "maxOutputTokens", skip_serializing_if = "Option::is_none")]
    max_output_tokens: Option<u32>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GeminiResponse {
    candidates: Vec<GeminiCandidate>,
    usage_metadata: Option<GeminiUsageMetadata>,
}

#[derive(Debug, Deserialize)]
struct GeminiCandidate {
    content: GeminiContent,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GeminiUsageMetadata {
    prompt_token_count: Option<u64>,
    candidates_token_count: Option<u64>,
}

#[derive(Debug, Deserialize)]
struct GeminiModelsResponse {
    models: Vec<GeminiModelItem>,
}

#[derive(Debug, Deserialize)]
struct GeminiModelItem {
    name: String,
    #[serde(rename = "supportedGenerationMethods", default)]
    supported_generation_methods: Vec<String>,
    #[serde(rename = "inputTokenLimit")]
    input_token_limit: Option<usize>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{system_message, user_message};

    #[test]
    fn gemini_api_key_is_encoded_in_endpoint_query() {
        let provider = GeminiProvider {
            id: "gemini".into(),
            api_key_env: "UNUSED_TEST_KEY".into(),
            base_url: "https://generativelanguage.googleapis.com/v1beta".into(),
            default_model: "gemini-2.5-flash".into(),
            models: Vec::new(),
        };
        let auth = GeminiAuth::ApiKey("key with&reserved=chars".into());
        let endpoint = provider.endpoint("models", &auth).unwrap();
        let parsed = reqwest::Url::parse(&endpoint).unwrap();
        assert_eq!(parsed.path(), "/v1beta/models");
        assert_eq!(
            parsed
                .query_pairs()
                .find(|(key, _)| key == "key")
                .unwrap()
                .1,
            "key with&reserved=chars"
        );
    }

    #[test]
    fn serializes_native_function_declarations_with_gemini_schema_types() {
        let request = GeminiRequest {
            contents: vec![GeminiContent {
                role: "user".into(),
                parts: vec![GeminiPart::text("inspect".into())],
            }],
            system_instruction: None,
            generation_config: GeminiGenerationConfig {
                temperature: None,
                max_output_tokens: Some(400),
            },
            tools: Some(vec![GeminiTool {
                function_declarations: vec![GeminiFunctionDeclaration {
                    name: "read_file".into(),
                    description: "Read a file".into(),
                    parameters: gemini_schema(json!({
                        "type":"object",
                        "properties":{"path":{"type":"string"}},
                        "required":["path"]
                    })),
                }],
            }]),
        };
        let value = serde_json::to_value(request).unwrap();
        assert_eq!(
            value["tools"][0]["functionDeclarations"][0]["name"],
            "read_file"
        );
        assert_eq!(
            value["tools"][0]["functionDeclarations"][0]["parameters"]["type"],
            "OBJECT"
        );
        assert_eq!(
            value["tools"][0]["functionDeclarations"][0]["parameters"]["properties"]["path"]
                ["type"],
            "STRING"
        );
    }

    #[test]
    fn maps_assistant_calls_and_tool_results_to_gemini_history() {
        let messages = vec![
            system_message("be helpful"),
            user_message("read a file"),
            ChatMessage {
                role: "assistant".into(),
                content: String::new(),
                tool_calls: vec![ToolCall {
                    id: "call-1".into(),
                    name: "read_file".into(),
                    arguments: r#"{"path":"README.md"}"#.into(),
                }],
                tool_call_id: None,
                name: None,
            },
            ChatMessage {
                role: "tool".into(),
                content: "file contents".into(),
                tool_calls: Vec::new(),
                tool_call_id: Some("call-1".into()),
                name: Some("read_file".into()),
            },
        ];
        let (contents, system) = convert_messages(messages).unwrap();
        assert_eq!(system.unwrap().parts[0].text.as_deref(), Some("be helpful"));
        let assistant = serde_json::to_value(&contents[1]).unwrap();
        assert_eq!(assistant["role"], "model");
        assert_eq!(assistant["parts"][0]["functionCall"]["name"], "read_file");
        assert_eq!(
            assistant["parts"][0]["functionCall"]["args"]["path"],
            "README.md"
        );
        let response = serde_json::to_value(&contents[2]).unwrap();
        assert_eq!(response["role"], "user");
        assert_eq!(
            response["parts"][0]["functionResponse"]["name"],
            "read_file"
        );
        assert_eq!(
            response["parts"][0]["functionResponse"]["response"]["result"],
            "file contents"
        );
    }

    #[test]
    fn malformed_tool_arguments_fail_before_network_access() {
        let message = ChatMessage {
            role: "assistant".into(),
            content: String::new(),
            tool_calls: vec![ToolCall {
                id: "call-1".into(),
                name: "do_work".into(),
                arguments: "not-json".into(),
            }],
            tool_call_id: None,
            name: None,
        };
        assert!(convert_messages(vec![message]).is_err());
    }
}
