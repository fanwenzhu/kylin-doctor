use super::provider::{LlmProvider, Message};
use async_trait::async_trait;
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
}

#[derive(Serialize)]
struct ChatCompletionRequest {
    model: String,
    messages: Vec<ChatMessage>,
    stream: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    temperature: Option<f64>,
}

#[derive(Serialize, Deserialize, Debug)]
struct ChatMessage {
    role: String,
    content: String,
}

#[derive(Deserialize)]
struct ChatCompletionResponse {
    choices: Vec<Choice>,
}

#[derive(Deserialize)]
struct Choice {
    message: ChatMessage,
}

#[async_trait]
impl LlmProvider for OpenAiCompatProvider {
    async fn chat(&self, messages: &[Message]) -> anyhow::Result<String> {
        let chat_messages: Vec<ChatMessage> = messages
            .iter()
            .map(|m| ChatMessage {
                role: m.role.clone(),
                content: m.content.clone(),
            })
            .collect();

        let request = ChatCompletionRequest {
            model: self.model.clone(),
            messages: chat_messages,
            stream: false,
            temperature: Some(0.7),
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
            anyhow::bail!("云端 API 错误 ({}): {}", status, body);
        }

        let chat_response: ChatCompletionResponse = response.json().await?;

        chat_response
            .choices
            .first()
            .map(|c| c.message.content.clone())
            .ok_or_else(|| anyhow::anyhow!("云端 API 返回空响应"))
    }

    fn name(&self) -> &str {
        "openai-compat"
    }

    async fn is_available(&self) -> bool {
        // 简单检查：尝试发送一个最小请求
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
