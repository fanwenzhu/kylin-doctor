use super::provider::ToolDefinition;
use crate::detector::{Detector, ScanReport, Severity};
use crate::detectors::*;
use serde_json::json;

/// 获取所有可用的诊断工具定义
pub fn get_tool_definitions() -> Vec<ToolDefinition> {
    vec![
        ToolDefinition {
            name: "scan_system".to_string(),
            description: "扫描系统健康状态：磁盘空间、systemd 服务、僵尸进程、内核日志、系统负载".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {},
                "required": []
            }),
        },
        ToolDefinition {
            name: "scan_hardware".to_string(),
            description: "扫描硬件状态：CPU 温度、内存使用率、磁盘健康/寿命、GPU、网卡、USB 设备、主板信息".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {},
                "required": []
            }),
        },
        ToolDefinition {
            name: "scan_software".to_string(),
            description: "扫描软件生态：包管理状态、依赖冲突、运行时环境、中文字体、兼容层".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {},
                "required": []
            }),
        },
        ToolDefinition {
            name: "scan_security".to_string(),
            description: "扫描安全状态：空密码账户、过期账户、SUID 文件、SSH 配置、防火墙、开放端口、审计日志、漏洞".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {},
                "required": []
            }),
        },
        ToolDefinition {
            name: "scan_performance".to_string(),
            description: "扫描性能状态：CPU 使用率/调度延迟、内存/Swap/碎片、磁盘 I/O/IOPS、网络延迟/带宽、桌面合成器".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {},
                "required": []
            }),
        },
        ToolDefinition {
            name: "scan_all".to_string(),
            description: "全面扫描所有模块（系统、硬件、软件、安全、性能）".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {},
                "required": []
            }),
        },
    ]
}

/// 允许的工具名称白名单
const ALLOWED_TOOLS: &[&str] = &[
    "scan_system",
    "scan_hardware",
    "scan_software",
    "scan_security",
    "scan_performance",
    "scan_all",
];

/// 验证工具名称是否在白名单内
pub fn is_valid_tool(name: &str) -> bool {
    ALLOWED_TOOLS.contains(&name)
}

/// 执行工具调用
pub fn execute_tool(name: &str) -> anyhow::Result<String> {
    let report = match name {
        "scan_system" => run_detector(&SystemDetector::new())?,
        "scan_hardware" => run_detector(&HardwareDetector::new())?,
        "scan_software" => run_detector(&SoftwareDetector::new())?,
        "scan_security" => run_detector(&SecurityDetector::new())?,
        "scan_performance" => run_detector(&PerformanceDetector::new())?,
        "scan_all" => run_all_detectors()?,
        _ => return Ok("未知工具".to_string()),
    };

    Ok(format_report(&report))
}

fn run_detector(detector: &dyn Detector) -> anyhow::Result<Vec<ScanReport>> {
    let report = detector.scan()?;
    Ok(vec![report])
}

fn run_all_detectors() -> anyhow::Result<Vec<ScanReport>> {
    let detectors: Vec<Box<dyn Detector>> = vec![
        Box::new(SystemDetector::new()),
        Box::new(HardwareDetector::new()),
        Box::new(SoftwareDetector::new()),
        Box::new(SecurityDetector::new()),
        Box::new(PerformanceDetector::new()),
    ];

    let mut reports = Vec::new();
    for d in &detectors {
        reports.push(d.scan()?);
    }
    Ok(reports)
}

fn format_report(reports: &[ScanReport]) -> String {
    let mut output = String::new();

    for report in reports {
        let (info, warning, critical) = report.summary();
        output.push_str(&format!(
            "\n## {} ({}ms)\n",
            report.module, report.duration_ms
        ));
        output.push_str(&format!(
            "严重: {}  警告: {}  信息: {}\n",
            critical, warning, info
        ));

        if report.findings.is_empty() {
            output.push_str("未发现问题\n");
            continue;
        }

        for f in &report.findings {
            let icon = match f.severity {
                Severity::Critical => "🔴",
                Severity::Warning => "⚠️",
                Severity::Info => "ℹ️",
            };
            output.push_str(&format!("{} [{}] {}\n", icon, f.id, f.title));
            output.push_str(&format!("   {}\n", f.description));
            if let Some(ref fix) = f.fix {
                output.push_str(&format!("   修复: {}\n", fix.description));
            }
        }
    }

    output
}
