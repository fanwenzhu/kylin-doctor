pub mod config;
pub mod detector;
pub mod detectors;
pub mod knowledge;
pub mod llm;
pub mod util;

pub use config::Config;
pub use detector::{Detector, Finding, FixAction, ScanReport, Severity};
pub use detectors::HardwareDetector;
pub use detectors::PerformanceDetector;
pub use detectors::SecurityDetector;
pub use detectors::SoftwareDetector;
pub use detectors::SystemDetector;
pub use knowledge::{KnowledgeStore, store::{Document, Chunk, SearchResult, KnowledgeStats}};
pub use llm::{AnthropicProvider, LlmProvider, Message, OllamaProvider, OpenAiCompatProvider, FunctionCall, ToolCall, ToolDefinition};
