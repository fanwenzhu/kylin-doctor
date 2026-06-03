pub mod ollama;
pub mod openai_compat;
pub mod provider;
pub mod tools;

pub use ollama::OllamaProvider;
pub use openai_compat::OpenAiCompatProvider;
pub use provider::{FunctionCall, LlmProvider, Message, ToolCall, ToolDefinition};
