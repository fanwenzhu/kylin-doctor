use serde::{Deserialize, Serialize};

/// 问题严重程度
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum Severity {
    /// 信息性提示
    Info,
    /// 警告
    Warning,
    /// 严重问题
    Critical,
}

impl std::fmt::Display for Severity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Severity::Info => write!(f, "info"),
            Severity::Warning => write!(f, "warning"),
            Severity::Critical => write!(f, "critical"),
        }
    }
}

/// 修复操作
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FixAction {
    /// 修复描述
    pub description: String,
    /// 要执行的命令
    pub command: String,
    /// 风险等级: low / medium / high
    pub risk_level: String,
}

/// 单个检测发现
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Finding {
    /// 唯一标识
    pub id: String,
    /// 所属模块
    pub module: String,
    /// 严重程度
    pub severity: Severity,
    /// 简短标题
    pub title: String,
    /// 详细描述
    pub description: String,
    /// 检测证据（命令输出、日志片段等）
    pub evidence: String,
    /// 修复方案
    pub fix: Option<FixAction>,
    /// 是否可自动修复
    pub auto_fixable: bool,
}

/// 扫描报告
#[derive(Debug, Serialize, Deserialize)]
pub struct ScanReport {
    /// 模块名称
    pub module: String,
    /// 检测结果列表
    pub findings: Vec<Finding>,
    /// 扫描耗时（毫秒）
    pub duration_ms: u64,
}

impl ScanReport {
    pub fn new(module: String) -> Self {
        Self {
            module,
            findings: Vec::new(),
            duration_ms: 0,
        }
    }

    /// 统计各严重程度的数量
    pub fn summary(&self) -> (usize, usize, usize) {
        let mut info = 0;
        let mut warning = 0;
        let mut critical = 0;
        for f in &self.findings {
            match f.severity {
                Severity::Info => info += 1,
                Severity::Warning => warning += 1,
                Severity::Critical => critical += 1,
            }
        }
        (info, warning, critical)
    }
}

/// 检测模块统一接口
pub trait Detector {
    /// 模块名称
    fn name(&self) -> &str;
    /// 模块描述
    fn description(&self) -> &str;
    /// 执行检测
    fn scan(&self) -> anyhow::Result<ScanReport>;
    /// 修复指定问题
    fn fix(&self, finding: &Finding) -> anyhow::Result<bool>;
    /// 模块是否可用
    fn is_available(&self) -> bool;
}
