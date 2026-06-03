use serde::{Deserialize, Serialize};

/// kylin-doctor 配置
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Config {
    #[serde(default)]
    pub general: GeneralConfig,
    #[serde(default)]
    pub llm: LlmConfig,
    #[serde(default)]
    pub web: WebConfig,
    #[serde(default)]
    pub daemon: DaemonConfig,
}

/// 通用配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeneralConfig {
    /// 输出详细程度: 0=简要, 1=标准, 2=详细
    #[serde(default = "default_verbose")]
    pub verbose: u8,
    /// 是否自动修复
    #[serde(default)]
    pub auto_fix: bool,
    /// 修复前是否确认
    #[serde(default = "default_true")]
    pub confirm_before_fix: bool,
    /// 是否完全禁止网络请求
    #[serde(default)]
    pub offline: bool,
}

/// LLM 配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmConfig {
    /// 策略: local / cloud / hybrid
    #[serde(default = "default_strategy")]
    pub strategy: String,
    /// 本地模型配置
    #[serde(default)]
    pub local: LocalLlmConfig,
    /// 云端模型配置
    #[serde(default)]
    pub cloud: CloudLlmConfig,
}

impl Default for LlmConfig {
    fn default() -> Self {
        Self {
            strategy: default_strategy(),
            local: LocalLlmConfig::default(),
            cloud: CloudLlmConfig::default(),
        }
    }
}

/// 本地 LLM 配置（Ollama）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LocalLlmConfig {
    #[serde(default = "default_endpoint")]
    pub endpoint: String,
    #[serde(default = "default_model")]
    pub model: String,
}

/// 云端 LLM 配置（OpenAI 兼容）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CloudLlmConfig {
    /// 供应商: qwen / deepseek / moonshot / custom
    #[serde(default = "default_cloud_provider")]
    pub provider: String,
    #[serde(default)]
    pub model: String,
    /// API Key 环境变量名
    #[serde(default)]
    pub api_key_env: String,
    #[serde(default)]
    pub endpoint: String,
}

/// Web 仪表盘配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebConfig {
    #[serde(default = "default_web_host")]
    pub host: String,
    #[serde(default = "default_web_port")]
    pub port: u16,
}

/// 守护进程配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DaemonConfig {
    /// 巡检间隔（秒）
    #[serde(default = "default_daemon_interval")]
    pub interval: u64,
    /// 是否桌面通知
    #[serde(default = "default_true")]
    pub notify: bool,
}

// --- Default 值 ---

impl Default for GeneralConfig {
    fn default() -> Self {
        Self {
            verbose: default_verbose(),
            auto_fix: false,
            confirm_before_fix: true,
            offline: false,
        }
    }
}

impl Default for LocalLlmConfig {
    fn default() -> Self {
        Self {
            endpoint: default_endpoint(),
            model: default_model(),
        }
    }
}

impl Default for CloudLlmConfig {
    fn default() -> Self {
        Self {
            provider: default_cloud_provider(),
            model: "qwen-plus".to_string(),
            api_key_env: "QWEN_API_KEY".to_string(),
            endpoint: "https://dashscope.aliyuncs.com/compatible-mode/v1".to_string(),
        }
    }
}

impl Default for WebConfig {
    fn default() -> Self {
        Self {
            host: default_web_host(),
            port: default_web_port(),
        }
    }
}

impl Default for DaemonConfig {
    fn default() -> Self {
        Self {
            interval: default_daemon_interval(),
            notify: true,
        }
    }
}

fn default_verbose() -> u8 { 1 }
fn default_true() -> bool { true }
fn default_strategy() -> String { "local".to_string() }
fn default_endpoint() -> String { "http://localhost:11434".to_string() }
fn default_model() -> String { "qwen2.5:3b".to_string() }
fn default_cloud_provider() -> String { "qwen".to_string() }
fn default_web_host() -> String { "127.0.0.1".to_string() }
fn default_web_port() -> u16 { 8080 }
fn default_daemon_interval() -> u64 { 3600 }

impl Config {
    /// 从配置文件加载，不存在则返回默认配置
    pub fn load() -> Self {
        let config_path = Self::config_path();
        if config_path.exists() {
            match std::fs::read_to_string(&config_path) {
                Ok(content) => match toml::from_str::<Config>(&content) {
                    Ok(config) => return config,
                    Err(e) => {
                        eprintln!("⚠️  配置文件解析失败: {}，使用默认配置", e);
                    }
                },
                Err(e) => {
                    eprintln!("⚠️  读取配置文件失败: {}，使用默认配置", e);
                }
            }
        }
        Config::default()
    }

    /// 配置文件路径: ~/.kylin-doctor/config.toml
    pub fn config_path() -> std::path::PathBuf {
        let home = dirs::home_dir().unwrap_or_else(|| std::path::PathBuf::from("."));
        home.join(".kylin-doctor").join("config.toml")
    }

    /// 保存配置到文件
    pub fn save(&self) -> anyhow::Result<()> {
        let config_path = Self::config_path();
        if let Some(parent) = config_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let content = toml::to_string_pretty(self)?;
        std::fs::write(&config_path, content)?;
        Ok(())
    }

    /// 生成默认配置文件内容（用于 init 命令）
    pub fn example_toml() -> &'static str {
        r#"# kylin-doctor 配置文件
# 位置: ~/.kylin-doctor/config.toml

[general]
verbose = 1              # 0=简要, 1=标准, 2=详细
auto_fix = false         # 是否自动修复
confirm_before_fix = true # 修复前是否确认
offline = false          # 完全禁止网络请求

[llm]
strategy = "local"       # local / cloud / hybrid

[llm.local]
endpoint = "http://localhost:11434"
model = "qwen2.5:3b"

[llm.cloud]
provider = "qwen"        # qwen / deepseek / moonshot / custom
model = "qwen-plus"
api_key_env = "QWEN_API_KEY"
endpoint = "https://dashscope.aliyuncs.com/compatible-mode/v1"

[web]
host = "127.0.0.1"
port = 8080

[daemon]
interval = 3600          # 巡检间隔（秒）
notify = true            # 是否桌面通知
"#
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config_values() {
        let config = Config::default();
        assert_eq!(config.general.verbose, 1);
        assert!(config.general.confirm_before_fix);
        assert!(!config.general.auto_fix);
        assert!(!config.general.offline);
        assert_eq!(config.llm.strategy, "local");
        assert_eq!(config.llm.local.endpoint, "http://localhost:11434");
        assert_eq!(config.llm.local.model, "qwen2.5:3b");
        assert_eq!(config.web.host, "127.0.0.1");
        assert_eq!(config.web.port, 8080);
        assert_eq!(config.daemon.interval, 3600);
        assert!(config.daemon.notify);
    }

    #[test]
    fn parse_toml_config() {
        let toml = r#"
[general]
verbose = 2
auto_fix = true
offline = true

[llm]
strategy = "cloud"

[llm.local]
endpoint = "http://custom:11434"
model = "qwen2.5:14b"

[llm.cloud]
provider = "deepseek"
model = "deepseek-chat"

[web]
host = "0.0.0.0"
port = 9090

[daemon]
interval = 1800
notify = false
"#;
        let config: Config = toml::from_str(toml).unwrap();
        assert_eq!(config.general.verbose, 2);
        assert!(config.general.auto_fix);
        assert!(config.general.offline);
        assert_eq!(config.llm.strategy, "cloud");
        assert_eq!(config.llm.local.endpoint, "http://custom:11434");
        assert_eq!(config.llm.local.model, "qwen2.5:14b");
        assert_eq!(config.llm.cloud.provider, "deepseek");
        assert_eq!(config.llm.cloud.model, "deepseek-chat");
        assert_eq!(config.web.host, "0.0.0.0");
        assert_eq!(config.web.port, 9090);
        assert_eq!(config.daemon.interval, 1800);
        assert!(!config.daemon.notify);
    }

    #[test]
    fn parse_empty_toml_uses_defaults() {
        let config: Config = toml::from_str("").unwrap();
        assert_eq!(config.general.verbose, 1);
        assert_eq!(config.llm.strategy, "local");
        assert_eq!(config.web.port, 8080);
    }

    #[test]
    fn parse_partial_toml() {
        let toml = r#"
[llm]
strategy = "hybrid"

[web]
port = 3000
"#;
        let config: Config = toml::from_str(toml).unwrap();
        assert_eq!(config.llm.strategy, "hybrid");
        assert_eq!(config.web.port, 3000);
        // Other fields should use defaults
        assert_eq!(config.general.verbose, 1);
        assert_eq!(config.llm.local.model, "qwen2.5:3b");
    }

    #[test]
    fn example_toml_is_valid() {
        let config: Config = toml::from_str(Config::example_toml()).unwrap();
        assert_eq!(config.llm.local.endpoint, "http://localhost:11434");
        assert_eq!(config.web.port, 8080);
    }
}
