use super::provider::{FunctionCall, LlmProvider, Message, ToolCall, ToolDefinition};
use crate::util::sanitize_api_error;
use async_trait::async_trait;
use futures_util::StreamExt;
use serde::{Deserialize, Serialize};

/// OpenAI 兼容 API 提供商（适配 Qwen/DeepSeek/Moonshot 等）
pub struct OpenAiCompatProvider {
    endpoint: String,
    model: String,
    api_key: String,
    client: reqwest::Client,
}

impl OpenAiCompatProvider {
    pub fn new(endpoint: &str, model: &str, api_key: &str) -> Self {
        Self {
            endpoint: endpoint.trim_end_matches('/').to_string(),
            model: model.to_string(),
            api_key: api_key.to_string(),
            client: reqwest::Client::new(),
        }
    }

    /// 从环境变量读取 API Key 创建
    pub fn from_env(endpoint: &str, model: &str, api_key_env: &str) -> anyhow::Result<Self> {
        let api_key = std::env::var(api_key_env).map_err(|_| {
            anyhow::anyhow!(
                "环境变量 {} 未设置，请设置后重试或在 ~/.kylin-doctor/config.toml 中配置",
                api_key_env
            )
        })?;
        Ok(Self::new(endpoint, model, &api_key))
    }

    /// 将通用 Message 转换为 OpenAI 消息格式
    fn convert_messages(messages: &[Message]) -> Vec<ChatMessage> {
        messages
            .iter()
            .map(|m| {
                let tool_calls = m.tool_calls.as_ref().map(|tcs| {
                    tcs.iter()
                        .map(|tc| OpenAiToolCall {
                            id: tc.id.clone(),
                            tool_type: "function".to_string(),
                            function: OpenAiFunctionCall {
                                name: tc.function.name.clone(),
                                arguments: tc.function.arguments.clone(),
                            },
                        })
                        .collect()
                });

                ChatMessage {
                    role: m.role.clone(),
                    content: if m.content.is_empty() {
                        None
                    } else {
                        Some(m.content.clone())
                    },
                    tool_calls,
                    tool_call_id: m.tool_call_id.clone(),
                }
            })
            .collect()
    }
}

// ==================== 请求/响应类型 ====================

#[derive(Serialize)]
struct ChatCompletionRequest {
    model: String,
    messages: Vec<ChatMessage>,
    stream: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    temperature: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tools: Option<Vec<OpenAiTool>>,
}

#[derive(Serialize, Deserialize, Debug)]
struct ChatMessage {
    role: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    content: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tool_calls: Option<Vec<OpenAiToolCall>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tool_call_id: Option<String>,
}

#[derive(Serialize, Deserialize, Debug)]
struct OpenAiToolCall {
    id: String,
    #[serde(rename = "type")]
    tool_type: String,
    function: OpenAiFunctionCall,
}

#[derive(Serialize, Deserialize, Debug)]
struct OpenAiFunctionCall {
    name: String,
    arguments: String,
}

#[derive(Serialize)]
struct OpenAiTool {
    #[serde(rename = "type")]
    tool_type: String,
    function: OpenAiToolDef,
}

#[derive(Serialize)]
struct OpenAiToolDef {
    name: String,
    description: String,
    parameters: serde_json::Value,
}

// ==================== 非流式响应 ====================

#[derive(Deserialize)]
struct ChatCompletionResponse {
    choices: Vec<Choice>,
}

#[derive(Deserialize)]
struct Choice {
    message: ChoiceMessage,
}

#[derive(Deserialize)]
struct ChoiceMessage {
    #[serde(default)]
    content: Option<String>,
    #[serde(default)]
    tool_calls: Option<Vec<OpenAiToolCall>>,
}

// ==================== 流式响应 ====================

#[derive(Deserialize)]
#[allow(dead_code)]
struct StreamResponse {
    choices: Vec<StreamChoice>,
}

#[derive(Deserialize)]
#[allow(dead_code)]
struct StreamChoice {
    delta: StreamDelta,
    #[serde(default)]
    finish_reason: Option<String>,
}

#[derive(Deserialize)]
#[allow(dead_code)]
struct StreamDelta {
    #[serde(default)]
    content: Option<String>,
    #[serde(default)]
    tool_calls: Option<Vec<StreamToolCall>>,
}

#[derive(Deserialize)]
#[allow(dead_code)]
struct StreamToolCall {
    #[serde(default)]
    index: Option<usize>,
    #[serde(default)]
    id: Option<String>,
    #[serde(default)]
    function: Option<StreamFunction>,
}

#[derive(Deserialize)]
#[allow(dead_code)]
struct StreamFunction {
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    arguments: Option<String>,
}

// ==================== LlmProvider 实现 ====================

#[async_trait]
impl LlmProvider for OpenAiCompatProvider {
    async fn chat(&self, messages: &[Message]) -> anyhow::Result<String> {
        let chat_messages = Self::convert_messages(messages);

        let request = ChatCompletionRequest {
            model: self.model.clone(),
            messages: chat_messages,
            stream: false,
            temperature: Some(0.7),
            tools: None,
        };

        let url = format!("{}/chat/completions", self.endpoint);
        let response = self
            .client
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/json")
            .json(&request)
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            anyhow::bail!("云端 API 错误 ({}): {}", status, sanitize_api_error(&body));
        }

        let chat_response: ChatCompletionResponse = response.json().await?;

        chat_response
            .choices
            .first()
            .and_then(|c| c.message.content.clone())
            .ok_or_else(|| anyhow::anyhow!("云端 API 返回空响应"))
    }

    async fn chat_with_tools(
        &self,
        messages: &[Message],
        tools: &[ToolDefinition],
    ) -> anyhow::Result<Message> {
        let chat_messages = Self::convert_messages(messages);

        let openai_tools: Vec<OpenAiTool> = tools
            .iter()
            .map(|t| OpenAiTool {
                tool_type: "function".to_string(),
                function: OpenAiToolDef {
                    name: t.name.clone(),
                    description: t.description.clone(),
                    parameters: t.parameters.clone(),
                },
            })
            .collect();

        let request = ChatCompletionRequest {
            model: self.model.clone(),
            messages: chat_messages,
            stream: false,
            temperature: Some(0.7),
            tools: Some(openai_tools),
        };

        let url = format!("{}/chat/completions", self.endpoint);
        let response = self
            .client
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/json")
            .json(&request)
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            anyhow::bail!("云端 API 错误 ({}): {}", status, sanitize_api_error(&body));
        }

        let chat_response: ChatCompletionResponse = response.json().await?;
        let msg = &chat_response
            .choices
            .first()
            .ok_or_else(|| anyhow::anyhow!("云端 API 返回空响应"))?
            .message;

        let tool_calls = msg.tool_calls.as_ref().map(|tcs| {
            tcs.iter()
                .map(|tc| ToolCall {
                    id: tc.id.clone(),
                    function: FunctionCall {
                        name: tc.function.name.clone(),
                        arguments: tc.function.arguments.clone(),
                    },
                })
                .collect()
        });

        Ok(Message {
            role: "assistant".to_string(),
            content: msg.content.clone().unwrap_or_default(),
            tool_calls,
            tool_call_id: None,
        })
    }

    async fn chat_stream(
        &self,
        messages: &[Message],
        on_chunk: Box<dyn Fn(String) + Send + 'static>,
    ) -> anyhow::Result<String> {
        let chat_messages = Self::convert_messages(messages);

        let request = ChatCompletionRequest {
            model: self.model.clone(),
            messages: chat_messages,
            stream: true,
            temperature: Some(0.7),
            tools: None,
        };

        let url = format!("{}/chat/completions", self.endpoint);
        let response = self
            .client
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/json")
            .json(&request)
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            anyhow::bail!("云端 API 错误 ({}): {}", status, sanitize_api_error(&body));
        }

        let mut full_response = String::new();
        let mut stream = response.bytes_stream();
        let mut line_buffer = String::new();
        let mut done = false;

        while let Some(chunk_result) = stream.next().await {
            if done {
                break;
            }
            let chunk = chunk_result?;
            line_buffer.push_str(&String::from_utf8_lossy(&chunk));

            // 处理缓冲区中的完整行
            while let Some(newline_pos) = line_buffer.find('\n') {
                let line = line_buffer[..newline_pos].trim().to_string();
                line_buffer = line_buffer[newline_pos + 1..].to_string();

                if line.is_empty() || line.starts_with(':') {
                    continue;
                }

                let json_str = if let Some(s) = line.strip_prefix("data: ") {
                    s
                } else {
                    continue;
                };

                // 流式结束标记
                if json_str == "[DONE]" {
                    done = true;
                    break;
                }

                if let Ok(resp) = serde_json::from_str::<StreamResponse>(json_str) {
                    if let Some(choice) = resp.choices.first() {
                        if let Some(content) = &choice.delta.content {
                            if !content.is_empty() {
                                full_response.push_str(content);
                                on_chunk(content.clone());
                            }
                        }
                    }
                }
            }
        }

        Ok(full_response)
    }

    fn name(&self) -> &str {
        "openai-compat"
    }

    async fn is_available(&self) -> bool {
        let url = format!("{}/models", self.endpoint);
        match self
            .client
            .get(&url)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .send()
            .await
        {
            Ok(resp) => resp.status().is_success(),
            Err(_) => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn convert_messages_basic() {
        let messages = vec![Message::system("sys"), Message::user("hello")];
        let converted = OpenAiCompatProvider::convert_messages(&messages);
        assert_eq!(converted.len(), 2);
        assert_eq!(converted[0].role, "system");
        assert_eq!(converted[0].content.as_deref(), Some("sys"));
        assert_eq!(converted[1].role, "user");
        assert_eq!(converted[1].content.as_deref(), Some("hello"));
    }

    #[test]
    fn convert_messages_with_tool_calls() {
        let messages = vec![Message {
            role: "assistant".to_string(),
            content: "thinking...".to_string(),
            tool_calls: Some(vec![ToolCall {
                id: "call_1".to_string(),
                function: FunctionCall {
                    name: "scan_all".to_string(),
                    arguments: "{}".to_string(),
                },
            }]),
            tool_call_id: None,
        }];
        let converted = OpenAiCompatProvider::convert_messages(&messages);
        assert_eq!(converted.len(), 1);
        assert!(converted[0].tool_calls.is_some());
        let tcs = converted[0].tool_calls.as_ref().unwrap();
        assert_eq!(tcs.len(), 1);
        assert_eq!(tcs[0].id, "call_1");
        assert_eq!(tcs[0].function.name, "scan_all");
    }

    #[test]
    fn convert_messages_tool_result() {
        let messages = vec![Message::tool_result("call_1", "result data")];
        let converted = OpenAiCompatProvider::convert_messages(&messages);
        assert_eq!(converted.len(), 1);
        assert_eq!(converted[0].role, "tool");
        assert_eq!(converted[0].tool_call_id.as_deref(), Some("call_1"));
    }

    #[test]
    fn tool_serialization() {
        let tool = OpenAiTool {
            tool_type: "function".to_string(),
            function: OpenAiToolDef {
                name: "scan_system".to_string(),
                description: "Scan system".to_string(),
                parameters: serde_json::json!({"type": "object"}),
            },
        };
        let json = serde_json::to_string(&tool).unwrap();
        assert!(json.contains("\"type\":\"function\""));
        assert!(json.contains("scan_system"));
    }

    #[test]
    fn stream_response_parsing() {
        let json = r#"{"choices":[{"delta":{"content":"Hello"},"finish_reason":null}]}"#;
        let resp: StreamResponse = serde_json::from_str(json).unwrap();
        assert_eq!(
            resp.choices[0].delta.content.as_deref(),
            Some("Hello")
        );
    }

    #[test]
    fn stream_response_done() {
        let json = r#"{"choices":[{"delta":{},"finish_reason":"stop"}]}"#;
        let resp: StreamResponse = serde_json::from_str(json).unwrap();
        assert!(resp.choices[0].delta.content.is_none());
        assert_eq!(resp.choices[0].finish_reason.as_deref(), Some("stop"));
    }
}
