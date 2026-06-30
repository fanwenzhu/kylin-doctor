use super::provider::{FunctionCall, LlmProvider, Message, ToolCall, ToolDefinition};
use crate::util::sanitize_api_error;
use async_trait::async_trait;
use futures_util::StreamExt;
use serde::{Deserialize, Serialize};

/// Anthropic Claude 模型提供商（Messages API）
pub struct AnthropicProvider {
    endpoint: String,
    model: String,
    api_key: String,
    client: reqwest::Client,
}

impl AnthropicProvider {
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

    /// 构建请求头
    fn build_headers(&self) -> reqwest::header::HeaderMap {
        let mut headers = reqwest::header::HeaderMap::new();
        headers.insert("x-api-key", self.api_key.parse().unwrap());
        headers.insert("anthropic-version", "2023-06-01".parse().unwrap());
        headers.insert("content-type", "application/json".parse().unwrap());
        headers
    }

    /// 将通用 Message 转换为 Anthropic 消息格式，同时提取 system 消息
    fn convert_messages(messages: &[Message]) -> (String, Vec<AnthropicMessage>) {
        let mut system = String::new();
        let mut anthropic_messages = Vec::new();

        for msg in messages {
            match msg.role.as_str() {
                "system" => {
                    if !system.is_empty() {
                        system.push_str("\n\n");
                    }
                    system.push_str(&msg.content);
                }
                "user" => {
                    anthropic_messages.push(AnthropicMessage {
                        role: "user".to_string(),
                        content: AnthropicContent::Text(msg.content.clone()),
                    });
                }
                "assistant" => {
                    if let Some(tool_calls) = &msg.tool_calls {
                        // 助手消息包含工具调用
                        let mut blocks: Vec<ContentBlock> = vec![];
                        if !msg.content.is_empty() {
                            blocks.push(ContentBlock::Text {
                                text: msg.content.clone(),
                            });
                        }
                        for tc in tool_calls {
                            blocks.push(ContentBlock::ToolUse {
                                id: tc.id.clone(),
                                name: tc.function.name.clone(),
                                input: serde_json::from_str(&tc.function.arguments)
                                    .unwrap_or(serde_json::Value::Null),
                            });
                        }
                        anthropic_messages.push(AnthropicMessage {
                            role: "assistant".to_string(),
                            content: AnthropicContent::Blocks(blocks),
                        });
                    } else {
                        anthropic_messages.push(AnthropicMessage {
                            role: "assistant".to_string(),
                            content: AnthropicContent::Text(msg.content.clone()),
                        });
                    }
                }
                "tool" => {
                    // 工具结果作为 user 消息的 tool_result 块
                    let tool_call_id = msg.tool_call_id.clone().unwrap_or_default();
                    anthropic_messages.push(AnthropicMessage {
                        role: "user".to_string(),
                        content: AnthropicContent::Blocks(vec![ContentBlock::ToolResult {
                            tool_use_id: tool_call_id,
                            content: msg.content.clone(),
                        }]),
                    });
                }
                _ => {
                    // 其他角色当作 user
                    anthropic_messages.push(AnthropicMessage {
                        role: msg.role.clone(),
                        content: AnthropicContent::Text(msg.content.clone()),
                    });
                }
            }
        }

        (system, anthropic_messages)
    }
}

// ==================== 请求/响应类型 ====================

#[derive(Serialize)]
struct AnthropicChatRequest {
    model: String,
    max_tokens: u32,
    #[serde(skip_serializing_if = "String::is_empty")]
    system: String,
    messages: Vec<AnthropicMessage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tools: Option<Vec<AnthropicTool>>,
    #[serde(skip_serializing_if = "is_false")]
    stream: bool,
}

fn is_false(b: &bool) -> bool {
    !*b
}

#[derive(Serialize, Deserialize, Debug)]
struct AnthropicMessage {
    role: String,
    content: AnthropicContent,
}

/// Anthropic 消息内容（文本或块数组）
#[derive(Serialize, Deserialize, Debug)]
#[serde(untagged)]
enum AnthropicContent {
    Text(String),
    Blocks(Vec<ContentBlock>),
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(tag = "type")]
enum ContentBlock {
    #[serde(rename = "text")]
    Text { text: String },
    #[serde(rename = "tool_use")]
    ToolUse {
        id: String,
        name: String,
        input: serde_json::Value,
    },
    #[serde(rename = "tool_result")]
    ToolResult {
        tool_use_id: String,
        content: String,
    },
}

#[derive(Serialize)]
struct AnthropicTool {
    name: String,
    description: String,
    input_schema: serde_json::Value,
}

#[derive(Deserialize)]
struct AnthropicChatResponse {
    content: Vec<ResponseContentBlock>,
}

#[derive(Deserialize)]
struct ResponseContentBlock {
    #[serde(rename = "type")]
    block_type: String,
    #[serde(default)]
    text: Option<String>,
    #[serde(default)]
    id: Option<String>,
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    input: Option<serde_json::Value>,
}

// ==================== 流式响应类型 ====================

#[derive(Deserialize)]
struct StreamEvent {
    #[serde(rename = "type")]
    event_type: String,
    #[serde(default)]
    delta: Option<StreamDelta>,
}

#[derive(Deserialize)]
struct StreamDelta {
    #[serde(rename = "type")]
    delta_type: String,
    #[serde(default)]
    text: Option<String>,
}

// ==================== LlmProvider 实现 ====================

const DEFAULT_MAX_TOKENS: u32 = 4096;

#[async_trait]
impl LlmProvider for AnthropicProvider {
    async fn chat(&self, messages: &[Message]) -> anyhow::Result<String> {
        let (system, anthropic_messages) = Self::convert_messages(messages);

        let request = AnthropicChatRequest {
            model: self.model.clone(),
            max_tokens: DEFAULT_MAX_TOKENS,
            system,
            messages: anthropic_messages,
            tools: None,
            stream: false,
        };

        let url = format!("{}/v1/messages", self.endpoint);
        let response = self
            .client
            .post(&url)
            .headers(self.build_headers())
            .json(&request)
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            anyhow::bail!("Anthropic API 错误 ({}): {}", status, sanitize_api_error(&body));
        }

        let chat_response: AnthropicChatResponse = response.json().await?;

        // 提取第一个 text 块
        chat_response
            .content
            .iter()
            .find(|b| b.block_type == "text")
            .and_then(|b| b.text.clone())
            .ok_or_else(|| anyhow::anyhow!("Anthropic API 返回无文本内容"))
    }

    async fn chat_with_tools(
        &self,
        messages: &[Message],
        tools: &[ToolDefinition],
    ) -> anyhow::Result<Message> {
        let (system, anthropic_messages) = Self::convert_messages(messages);

        let anthropic_tools: Vec<AnthropicTool> = tools
            .iter()
            .map(|t| AnthropicTool {
                name: t.name.clone(),
                description: t.description.clone(),
                input_schema: t.parameters.clone(),
            })
            .collect();

        let request = AnthropicChatRequest {
            model: self.model.clone(),
            max_tokens: DEFAULT_MAX_TOKENS,
            system,
            messages: anthropic_messages,
            tools: Some(anthropic_tools),
            stream: false,
        };

        let url = format!("{}/v1/messages", self.endpoint);
        let response = self
            .client
            .post(&url)
            .headers(self.build_headers())
            .json(&request)
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            anyhow::bail!("Anthropic API 错误 ({}): {}", status, sanitize_api_error(&body));
        }

        let chat_response: AnthropicChatResponse = response.json().await?;

        // 提取文本和工具调用
        let mut text_parts = Vec::new();
        let mut tool_calls = Vec::new();

        for block in &chat_response.content {
            match block.block_type.as_str() {
                "text" => {
                    if let Some(text) = &block.text {
                        text_parts.push(text.clone());
                    }
                }
                "tool_use" => {
                    if let (Some(id), Some(name), Some(input)) =
                        (&block.id, &block.name, &block.input)
                    {
                        tool_calls.push(ToolCall {
                            id: id.clone(),
                            function: FunctionCall {
                                name: name.clone(),
                                arguments: input.to_string(),
                            },
                        });
                    }
                }
                _ => {}
            }
        }

        let content = text_parts.join("");
        let tool_calls = if tool_calls.is_empty() {
            None
        } else {
            Some(tool_calls)
        };

        Ok(Message {
            role: "assistant".to_string(),
            content,
            tool_calls,
            tool_call_id: None,
        })
    }

    async fn chat_stream(
        &self,
        messages: &[Message],
        on_chunk: Box<dyn Fn(String) + Send + 'static>,
    ) -> anyhow::Result<String> {
        let (system, anthropic_messages) = Self::convert_messages(messages);

        let request = AnthropicChatRequest {
            model: self.model.clone(),
            max_tokens: DEFAULT_MAX_TOKENS,
            system,
            messages: anthropic_messages,
            tools: None,
            stream: true,
        };

        let url = format!("{}/v1/messages", self.endpoint);
        let response = self
            .client
            .post(&url)
            .headers(self.build_headers())
            .json(&request)
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            anyhow::bail!("Anthropic API 错误 ({}): {}", status, sanitize_api_error(&body));
        }

        let mut full_response = String::new();
        let mut stream = response.bytes_stream();

        while let Some(chunk_result) = stream.next().await {
            let chunk = chunk_result?;
            let text = String::from_utf8_lossy(&chunk);

            for line in text.lines() {
                let line = line.trim();
                if line.is_empty() || line.starts_with(':') {
                    continue;
                }

                // SSE 格式: "event: xxx\ndata: {...}"
                if line.starts_with("event: ") {
                    continue; // 事件类型行，我们通过 data 中的 type 字段判断
                }

                let json_str = if line.starts_with("data: ") {
                    &line[6..]
                } else {
                    continue; // 跳过非 data 行
                };

                if let Ok(event) = serde_json::from_str::<StreamEvent>(json_str) {
                    match event.event_type.as_str() {
                        "content_block_delta" => {
                            if let Some(delta) = &event.delta {
                                if delta.delta_type == "text_delta" {
                                    if let Some(text) = &delta.text {
                                        if !text.is_empty() {
                                            full_response.push_str(text);
                                            on_chunk(text.clone());
                                        }
                                    }
                                }
                            }
                        }
                        "message_stop" => {
                            break;
                        }
                        _ => {} // message_start, content_block_start, content_block_stop 等忽略
                    }
                }
            }
        }

        Ok(full_response)
    }

    fn name(&self) -> &str {
        "anthropic"
    }

    async fn is_available(&self) -> bool {
        // 发送一个最小请求测试连通性
        let request = AnthropicChatRequest {
            model: self.model.clone(),
            max_tokens: 1,
            system: String::new(),
            messages: vec![AnthropicMessage {
                role: "user".to_string(),
                content: AnthropicContent::Text("hi".to_string()),
            }],
            tools: None,
            stream: false,
        };

        let url = format!("{}/v1/messages", self.endpoint);
        match self
            .client
            .post(&url)
            .headers(self.build_headers())
            .json(&request)
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
    fn convert_messages_extracts_system() {
        let messages = vec![
            Message::system("你是一个助手"),
            Message::user("你好"),
        ];
        let (system, anthropic_msgs) = AnthropicProvider::convert_messages(&messages);
        assert_eq!(system, "你是一个助手");
        assert_eq!(anthropic_msgs.len(), 1);
        assert_eq!(anthropic_msgs[0].role, "user");
    }

    #[test]
    fn convert_messages_tool_result() {
        let messages = vec![Message {
            role: "tool".to_string(),
            content: "scan result".to_string(),
            tool_calls: None,
            tool_call_id: Some("call_123".to_string()),
        }];
        let (_, anthropic_msgs) = AnthropicProvider::convert_messages(&messages);
        assert_eq!(anthropic_msgs.len(), 1);
        assert_eq!(anthropic_msgs[0].role, "user");
    }

    #[test]
    fn convert_messages_assistant_with_tool_calls() {
        let messages = vec![Message {
            role: "assistant".to_string(),
            content: "让我扫描一下".to_string(),
            tool_calls: Some(vec![ToolCall {
                id: "call_1".to_string(),
                function: FunctionCall {
                    name: "scan_system".to_string(),
                    arguments: "{}".to_string(),
                },
            }]),
            tool_call_id: None,
        }];
        let (_, anthropic_msgs) = AnthropicProvider::convert_messages(&messages);
        assert_eq!(anthropic_msgs.len(), 1);
        assert_eq!(anthropic_msgs[0].role, "assistant");
    }

    #[test]
    fn anthropic_tool_serialization() {
        let tool = AnthropicTool {
            name: "scan_system".to_string(),
            description: "Scan system".to_string(),
            input_schema: serde_json::json!({"type": "object", "properties": {}}),
        };
        let json = serde_json::to_string(&tool).unwrap();
        assert!(json.contains("input_schema"));
        assert!(json.contains("scan_system"));
    }
}
