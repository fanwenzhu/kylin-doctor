use super::provider::{FunctionCall, LlmProvider, Message, ToolCall, ToolDefinition};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use futures_util::StreamExt;

/// Ollama 本地模型提供商
pub struct OllamaProvider {
    endpoint: String,
    model: String,
    embedding_model: String,
    client: reqwest::Client,
}

impl OllamaProvider {
    pub fn new(endpoint: &str, model: &str) -> Self {
        Self {
            endpoint: endpoint.trim_end_matches('/').to_string(),
            model: model.to_string(),
            embedding_model: "nomic-embed-text".to_string(),
            client: reqwest::Client::new(),
        }
    }

    pub fn with_embedding_model(mut self, model: &str) -> Self {
        self.embedding_model = model.to_string();
        self
    }

    pub fn default_local() -> Self {
        Self::new("http://localhost:11434", "qwen2.5:3b")
    }
}

#[derive(Serialize)]
struct OllamaChatRequest {
    model: String,
    messages: Vec<OllamaMessage>,
    stream: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    tools: Option<Vec<OllamaTool>>,
}

#[derive(Serialize, Deserialize, Debug)]
struct OllamaMessage {
    role: String,
    content: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    tool_calls: Option<Vec<OllamaToolCall>>,
}

#[derive(Serialize, Deserialize, Debug)]
struct OllamaToolCall {
    function: OllamaFunctionCall,
}

#[derive(Serialize, Deserialize, Debug)]
struct OllamaFunctionCall {
    name: String,
    arguments: serde_json::Value,
}

#[derive(Serialize)]
struct OllamaTool {
    #[serde(rename = "type")]
    tool_type: String,
    function: OllamaToolDef,
}

#[derive(Serialize)]
struct OllamaToolDef {
    name: String,
    description: String,
    parameters: serde_json::Value,
}

#[derive(Deserialize)]
struct OllamaChatResponse {
    message: OllamaChatResponseMessage,
}

#[derive(Deserialize)]
struct OllamaChatResponseMessage {
    role: String,
    content: String,
    #[serde(default)]
    tool_calls: Option<Vec<OllamaToolCall>>,
}

#[derive(Serialize)]
struct OllamaEmbedRequest {
    model: String,
    input: Vec<String>,
}

#[derive(Deserialize)]
struct OllamaEmbedResponse {
    embeddings: Vec<Vec<f32>>,
}

#[async_trait]
impl LlmProvider for OllamaProvider {
    async fn chat(&self, messages: &[Message]) -> anyhow::Result<String> {
        let ollama_messages: Vec<OllamaMessage> = messages
            .iter()
            .map(|m| OllamaMessage {
                role: m.role.clone(),
                content: m.content.clone(),
                tool_calls: None,
            })
            .collect();

        let request = OllamaChatRequest {
            model: self.model.clone(),
            messages: ollama_messages,
            stream: false,
            tools: None,
        };

        let url = format!("{}/api/chat", self.endpoint);
        let response = self.client.post(&url).json(&request).send().await?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            anyhow::bail!("Ollama API 错误 ({}): {}", status, body);
        }

        let chat_response: OllamaChatResponse = response.json().await?;
        Ok(chat_response.message.content)
    }

    async fn chat_with_tools(
        &self,
        messages: &[Message],
        tools: &[ToolDefinition],
    ) -> anyhow::Result<Message> {
        let ollama_messages: Vec<OllamaMessage> = messages
            .iter()
            .map(|m| OllamaMessage {
                role: m.role.clone(),
                content: m.content.clone(),
                tool_calls: m.tool_calls.as_ref().map(|tcs| {
                    tcs.iter()
                        .map(|tc| OllamaToolCall {
                            function: OllamaFunctionCall {
                                name: tc.function.name.clone(),
                                arguments: serde_json::from_str(&tc.function.arguments)
                                    .unwrap_or(serde_json::Value::Null),
                            },
                        })
                        .collect()
                }),
            })
            .collect();

        let ollama_tools: Vec<OllamaTool> = tools
            .iter()
            .map(|t| OllamaTool {
                tool_type: "function".to_string(),
                function: OllamaToolDef {
                    name: t.name.clone(),
                    description: t.description.clone(),
                    parameters: t.parameters.clone(),
                },
            })
            .collect();

        let request = OllamaChatRequest {
            model: self.model.clone(),
            messages: ollama_messages,
            stream: false,
            tools: Some(ollama_tools),
        };

        let url = format!("{}/api/chat", self.endpoint);
        let response = self.client.post(&url).json(&request).send().await?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            anyhow::bail!("Ollama API 错误 ({}): {}", status, body);
        }

        let chat_response: OllamaChatResponse = response.json().await?;
        let msg = chat_response.message;

        let tool_calls = msg.tool_calls.map(|tcs| {
            tcs.into_iter()
                .enumerate()
                .map(|(i, tc)| ToolCall {
                    id: format!("call_{}", i),
                    function: FunctionCall {
                        name: tc.function.name,
                        arguments: tc.function.arguments.to_string(),
                    },
                })
                .collect()
        });

        Ok(Message {
            role: msg.role,
            content: msg.content,
            tool_calls,
            tool_call_id: None,
        })
    }

    async fn embed(&self, texts: &[String]) -> anyhow::Result<Vec<Vec<f32>>> {
        let request = OllamaEmbedRequest {
            model: self.embedding_model.clone(),
            input: texts.to_vec(),
        };

        let url = format!("{}/api/embed", self.endpoint);
        let response = self.client.post(&url).json(&request).send().await?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            anyhow::bail!("Ollama embed API 错误 ({}): {}", status, body);
        }

        let embed_response: OllamaEmbedResponse = response.json().await?;
        Ok(embed_response.embeddings)
    }

    fn name(&self) -> &str {
        "ollama"
    }

    async fn is_available(&self) -> bool {
        let url = format!("{}/api/tags", self.endpoint);
        match self.client.get(&url).send().await {
            Ok(resp) => resp.status().is_success(),
            Err(_) => false,
        }
    }

    async fn chat_stream(
        &self,
        messages: &[Message],
        on_chunk: Box<dyn Fn(String) + Send + 'static>,
    ) -> anyhow::Result<String> {
        // 调用 OllamaProvider 自身的方法
        OllamaProvider::chat_stream(self, messages, on_chunk).await
    }
}

/// SSE 流式响应解析
#[derive(Deserialize)]
struct OllamaStreamResponse {
    message: Option<OllamaStreamMessage>,
    done: bool,
}

#[derive(Deserialize)]
struct OllamaStreamMessage {
    content: Option<String>,
}

impl OllamaProvider {
    /// 流式聊天补全
    pub async fn chat_stream(
        &self,
        messages: &[Message],
        on_chunk: Box<dyn Fn(String) + Send + 'static>,
    ) -> anyhow::Result<String> {
        let ollama_messages: Vec<OllamaMessage> = messages
            .iter()
            .map(|m| OllamaMessage {
                role: m.role.clone(),
                content: m.content.clone(),
                tool_calls: None,
            })
            .collect();

        let request = OllamaChatRequest {
            model: self.model.clone(),
            messages: ollama_messages,
            stream: true,
            tools: None,
        };

        let url = format!("{}/api/chat", self.endpoint);
        let response = self.client.post(&url).json(&request).send().await?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            anyhow::bail!("Ollama API 错误 ({}): {}", status, body);
        }

        let mut full_response = String::new();
        let mut stream = response.bytes_stream();

        while let Some(chunk_result) = stream.next().await {
            let chunk = chunk_result?;

            // 解析 SSE 格式：每行以 "data: " 开头
            let text = String::from_utf8_lossy(&chunk);
            for line in text.lines() {
                let line = line.trim();
                if line.is_empty() || line.starts_with(':') {
                    continue;
                }

                // 移除 "data: " 前缀
                let json_str = if line.starts_with("data: ") {
                    &line[6..]
                } else {
                    line
                };

                // 解析 JSON
                if let Ok(response) = serde_json::from_str::<OllamaStreamResponse>(json_str) {
                    if let Some(msg) = response.message {
                        if let Some(content) = msg.content {
                            if !content.is_empty() {
                                full_response.push_str(&content);
                                on_chunk(content);
                            }
                        }
                    }
                    if response.done {
                        break;
                    }
                }
            }
        }

        Ok(full_response)
    }
}
