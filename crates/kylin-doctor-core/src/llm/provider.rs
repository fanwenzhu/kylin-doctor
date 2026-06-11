use async_trait::async_trait;
use serde::{Deserialize, Serialize};

/// LLM 消息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    /// 角色: "system" | "user" | "assistant" | "tool"
    pub role: String,
    /// 消息内容
    pub content: String,
    /// Function Calling: 工具调用列表（assistant 消息）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<Vec<ToolCall>>,
    /// Function Calling: 工具调用 ID（tool 角色消息）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_call_id: Option<String>,
}

/// 工具调用
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCall {
    pub id: String,
    pub function: FunctionCall,
}

/// 函数调用
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FunctionCall {
    pub name: String,
    pub arguments: String,
}

/// 工具定义
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolDefinition {
    pub name: String,
    pub description: String,
    pub parameters: serde_json::Value,
}

impl Message {
    pub fn system(content: &str) -> Self {
        Self {
            role: "system".to_string(),
            content: content.to_string(),
            tool_calls: None,
            tool_call_id: None,
        }
    }

    pub fn user(content: &str) -> Self {
        Self {
            role: "user".to_string(),
            content: content.to_string(),
            tool_calls: None,
            tool_call_id: None,
        }
    }

    pub fn assistant(content: &str) -> Self {
        Self {
            role: "assistant".to_string(),
            content: content.to_string(),
            tool_calls: None,
            tool_call_id: None,
        }
    }

    pub fn tool_result(tool_call_id: &str, content: &str) -> Self {
        Self {
            role: "tool".to_string(),
            content: content.to_string(),
            tool_calls: None,
            tool_call_id: Some(tool_call_id.to_string()),
        }
    }
}

/// LLM 提供商统一接口
#[async_trait]
pub trait LlmProvider: Send + Sync {
    /// 聊天补全
    async fn chat(&self, messages: &[Message]) -> anyhow::Result<String>;

    /// 带 Function Calling 的聊天补全
    async fn chat_with_tools(
        &self,
        messages: &[Message],
        _tools: &[ToolDefinition],
    ) -> anyhow::Result<Message> {
        // 默认实现：忽略 tools，回退到普通 chat
        let content = self.chat(messages).await?;
        Ok(Message::assistant(&content))
    }

    /// 流式聊天补全（默认实现回退到普通chat）
    async fn chat_stream(
        &self,
        messages: &[Message],
        on_chunk: Box<dyn Fn(String) + Send + 'static>,
    ) -> anyhow::Result<String> {
        // 默认实现：不支持流式，回退到普通chat
        let content = self.chat(messages).await?;
        on_chunk(content.clone());
        Ok(content)
    }

    /// 文本向量化（RAG 用）
    async fn embed(&self, _texts: &[String]) -> anyhow::Result<Vec<Vec<f32>>> {
        anyhow::bail!("当前 LLM 提供商不支持文本向量化")
    }

    /// 供应商名称
    fn name(&self) -> &str;

    /// 是否可用
    async fn is_available(&self) -> bool;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn message_system() {
        let msg = Message::system("test prompt");
        assert_eq!(msg.role, "system");
        assert_eq!(msg.content, "test prompt");
        assert!(msg.tool_calls.is_none());
        assert!(msg.tool_call_id.is_none());
    }

    #[test]
    fn message_user() {
        let msg = Message::user("hello");
        assert_eq!(msg.role, "user");
        assert_eq!(msg.content, "hello");
    }

    #[test]
    fn message_assistant() {
        let msg = Message::assistant("response");
        assert_eq!(msg.role, "assistant");
        assert_eq!(msg.content, "response");
    }

    #[test]
    fn message_tool_result() {
        let msg = Message::tool_result("call_123", "scan result");
        assert_eq!(msg.role, "tool");
        assert_eq!(msg.content, "scan result");
        assert_eq!(msg.tool_call_id.as_deref(), Some("call_123"));
    }

    #[test]
    fn message_serialization_roundtrip() {
        let msg = Message::user("test");
        let json = serde_json::to_string(&msg).unwrap();
        let deserialized: Message = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.role, "user");
        assert_eq!(deserialized.content, "test");
    }

    #[test]
    fn tool_definition_serialization() {
        let tool = ToolDefinition {
            name: "scan_system".to_string(),
            description: "Scan system health".to_string(),
            parameters: serde_json::json!({"type": "object", "properties": {}}),
        };
        let json = serde_json::to_string(&tool).unwrap();
        assert!(json.contains("scan_system"));
        assert!(json.contains("Scan system health"));
    }
}
