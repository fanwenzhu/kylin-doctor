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
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct FixAction {
    /// 修复描述
    pub description: String,
    /// 要执行的命令（用于显示和 sh -c 回退）
    pub command: String,
    /// 风险等级: low / medium / high
    pub risk_level: String,
    /// 结构化执行：程序路径（优先于 sh -c）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub program: Option<String>,
    /// 结构化执行：参数列表
    #[serde(skip_serializing_if = "Option::is_none")]
    pub args: Option<Vec<String>>,
}

impl FixAction {
    /// 安全执行修复操作
    ///
    /// 优先使用结构化的 program + args（无 shell 注入风险），
    /// 回退到 `sh -c command`（仅当 program 未设置时）。
    pub fn run_fix(&self) -> anyhow::Result<bool> {
        let status = if let Some(ref program) = self.program {
            // 结构化执行：直接调用程序，不经过 shell
            let args = self.args.as_deref().unwrap_or(&[]);
            std::process::Command::new(program)
                .args(args)
                .status()?
        } else {
            // 回退：通过 sh -c 执行（兼容旧式 command 字符串）
            std::process::Command::new("sh")
                .args(["-c", &self.command])
                .status()?
        };
        Ok(status.success())
    }
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
    /// 是否包含耗时检测（--quick 模式下跳过）
    fn is_slow(&self) -> bool {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scan_report_summary_empty() {
        let report = ScanReport::new("test".to_string());
        assert_eq!(report.summary(), (0, 0, 0));
    }

    #[test]
    fn scan_report_summary_counts() {
        let mut report = ScanReport::new("test".to_string());
        report.findings.push(Finding {
            id: "1".into(), module: "test".into(), severity: Severity::Info,
            title: "t".into(), description: "d".into(), evidence: "e".into(),
            fix: None, auto_fixable: false,
        });
        report.findings.push(Finding {
            id: "2".into(), module: "test".into(), severity: Severity::Warning,
            title: "t".into(), description: "d".into(), evidence: "e".into(),
            fix: None, auto_fixable: false,
        });
        report.findings.push(Finding {
            id: "3".into(), module: "test".into(), severity: Severity::Critical,
            title: "t".into(), description: "d".into(), evidence: "e".into(),
            fix: None, auto_fixable: false,
        });
        report.findings.push(Finding {
            id: "4".into(), module: "test".into(), severity: Severity::Warning,
            title: "t".into(), description: "d".into(), evidence: "e".into(),
            fix: None, auto_fixable: false,
        });
        assert_eq!(report.summary(), (1, 2, 1));
    }

    #[test]
    fn severity_display() {
        assert_eq!(format!("{}", Severity::Info), "info");
        assert_eq!(format!("{}", Severity::Warning), "warning");
        assert_eq!(format!("{}", Severity::Critical), "critical");
    }

    #[test]
    fn severity_eq() {
        assert_eq!(Severity::Info, Severity::Info);
        assert_ne!(Severity::Info, Severity::Warning);
    }

    #[test]
    fn run_fix_with_program_and_args() {
        let fix = FixAction {
            description: "test".to_string(),
            command: "echo hello".to_string(),
            risk_level: "low".to_string(),
            program: Some("echo".to_string()),
            args: Some(vec!["hello".to_string()]),
        };
        let result = fix.run_fix();
        assert!(result.is_ok());
        assert!(result.unwrap()); // echo exits 0
    }

    #[test]
    fn run_fix_with_command_only_fallback() {
        let fix = FixAction {
            description: "test".to_string(),
            command: "true".to_string(),
            risk_level: "low".to_string(),
            ..Default::default()
        };
        let result = fix.run_fix();
        assert!(result.is_ok());
        assert!(result.unwrap());
    }

    #[test]
    fn run_fix_command_fails() {
        let fix = FixAction {
            description: "test".to_string(),
            command: "false".to_string(),
            risk_level: "low".to_string(),
            ..Default::default()
        };
        let result = fix.run_fix();
        assert!(result.is_ok());
        assert!(!result.unwrap()); // false exits 1
    }

    #[test]
    fn run_fix_program_not_found() {
        let fix = FixAction {
            description: "test".to_string(),
            command: "n/a".to_string(),
            risk_level: "low".to_string(),
            program: Some("/nonexistent/binary/12345".to_string()),
            args: None,
        };
        let result = fix.run_fix();
        assert!(result.is_err());
    }
}
