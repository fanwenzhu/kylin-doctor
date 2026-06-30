use clap::{Args, ValueEnum};
use colored::Colorize;
use kylin_doctor_core::{
    html_escape, Detector, HardwareDetector, PerformanceDetector, ScanReport, SecurityDetector,
    SoftwareDetector, SystemDetector,
};
use std::io::Write;

#[derive(Args, Debug)]
pub struct ReportArgs {
    /// 输出格式
    #[arg(short, long, value_enum, default_value = "json")]
    pub format: ReportFormat,

    /// 输出文件路径（不指定则输出到 stdout）
    #[arg(short, long)]
    pub output: Option<String>,
}

#[derive(ValueEnum, Clone, Debug)]
pub enum ReportFormat {
    Json,
    Html,
}

pub fn execute(args: &ReportArgs) -> anyhow::Result<()> {
    eprintln!("{}", "📋 正在生成诊断报告...".bold().cyan());

    let detectors: Vec<Box<dyn Detector>> = vec![
        Box::new(SystemDetector::new()),
        Box::new(HardwareDetector::new()),
        Box::new(SoftwareDetector::new()),
        Box::new(SecurityDetector::new()),
        Box::new(PerformanceDetector::new()),
    ];

    let mut reports: Vec<ScanReport> = Vec::new();
    for d in &detectors {
        eprint!("  扫描 {}...", d.name());
        reports.push(d.scan()?);
        eprintln!(" ✓");
    }

    let (total_info, total_warning, total_critical) =
        reports.iter().fold((0, 0, 0), |(i, w, c), r| {
            let (ri, rw, rc) = r.summary();
            (i + ri, w + rw, c + rc)
        });

    let content = match args.format {
        ReportFormat::Json => generate_json(&reports, total_info, total_warning, total_critical),
        ReportFormat::Html => generate_html(&reports, total_info, total_warning, total_critical),
    };

    match &args.output {
        Some(path) => {
            std::fs::write(path, &content)?;
            eprintln!("✅ 报告已保存到: {}", path.green().bold());
        }
        None => {
            // 输出到 stdout
            let stdout = std::io::stdout();
            let mut handle = stdout.lock();
            handle.write_all(content.as_bytes())?;
            handle.flush()?;
        }
    }

    // 总结
    eprintln!();
    eprintln!("{}", "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━".dimmed());
    if total_critical > 0 {
        eprintln!(
            "🔴 严重: {}  ⚠️  警告: {}  ℹ️  信息: {}",
            total_critical.to_string().red().bold(),
            total_warning.to_string().yellow(),
            total_info.to_string().dimmed()
        );
    } else if total_warning > 0 {
        eprintln!(
            "⚠️  警告: {}  ℹ️  信息: {}",
            total_warning.to_string().yellow().bold(),
            total_info.to_string().dimmed()
        );
    } else {
        eprintln!("{}", "✅ 系统一切正常。".green().bold());
    }

    Ok(())
}

fn generate_json(
    reports: &[ScanReport],
    info: usize,
    warning: usize,
    critical: usize,
) -> String {
    let status = if critical > 0 {
        "critical"
    } else if warning > 0 {
        "warning"
    } else {
        "ok"
    };

    let report_data: Vec<serde_json::Value> = reports
        .iter()
        .map(|r| {
            let (i, w, c) = r.summary();
            serde_json::json!({
                "module": r.module,
                "duration_ms": r.duration_ms,
                "summary": {"info": i, "warning": w, "critical": c},
                "findings": r.findings.iter().map(|f| {
                    serde_json::json!({
                        "id": f.id,
                        "severity": format!("{:?}", f.severity).to_lowercase(),
                        "title": f.title,
                        "description": f.description,
                        "evidence": f.evidence,
                        "fix": f.fix.as_ref().map(|fix| serde_json::json!({
                            "description": fix.description,
                            "command": fix.command,
                            "risk_level": fix.risk_level
                        })),
                        "auto_fixable": f.auto_fixable
                    })
                }).collect::<Vec<_>>()
            })
        })
        .collect();

    serde_json::to_string_pretty(&serde_json::json!({
        "version": env!("CARGO_PKG_VERSION"),
        "status": status,
        "summary": {"info": info, "warning": warning, "critical": critical},
        "modules": report_data
    }))
    .unwrap_or_default()
}

fn generate_html(
    reports: &[ScanReport],
    info: usize,
    warning: usize,
    critical: usize,
) -> String {
    use kylin_doctor_core::Severity;

    let status = if critical > 0 {
        "🔴 存在严重问题"
    } else if warning > 0 {
        "⚠️ 有警告项"
    } else {
        "✅ 系统正常"
    };

    let mut html = format!(
        r#"<!DOCTYPE html>
<html lang="zh-CN">
<head>
<meta charset="UTF-8">
<title>kylin-doctor 诊断报告</title>
<style>
body{{font-family:-apple-system,sans-serif;max-width:900px;margin:0 auto;padding:20px;background:#f5f5f5;color:#333}}
h1{{color:#0f172a;border-bottom:2px solid #06b6d4;padding-bottom:10px}}
.summary{{background:#fff;border-radius:8px;padding:16px;margin:16px 0;box-shadow:0 1px 3px rgba(0,0,0,.1)}}
.module{{background:#fff;border-radius:8px;padding:16px;margin:12px 0;box-shadow:0 1px 3px rgba(0,0,0,.1)}}
.module h2{{margin-top:0;color:#1e293b}}
.finding{{border-left:3px solid #94a3b8;padding:8px 12px;margin:8px 0;background:#f8fafc}}
.finding.critical{{border-color:#ef4444;background:#fef2f2}}
.finding.warning{{border-color:#eab308;background:#fefce8}}
.finding.info{{border-color:#3b82f6;background:#eff6ff}}
.badge{{display:inline-block;padding:2px 8px;border-radius:4px;font-size:12px;font-weight:bold;color:#fff}}
.badge.critical{{background:#ef4444}}.badge.warning{{background:#eab308;color:#000}}.badge.info{{background:#3b82f6}}
.fix{{color:#06b6d4;font-size:13px;margin-top:4px}}
.evidence{{font-size:11px;color:#64748b;font-family:monospace;background:#f1f5f9;padding:6px 10px;border-radius:4px;margin-top:4px;white-space:pre-wrap}}
footer{{text-align:center;color:#94a3b8;font-size:12px;margin-top:40px}}
</style>
</head>
<body>
<h1>🔍 kylin-doctor 诊断报告</h1>
<div class="summary">
<p><strong>状态:</strong> {}</p>
<p><strong>严重:</strong> {} &nbsp; <strong>警告:</strong> {} &nbsp; <strong>信息:</strong> {}</p>
</div>
"#,
        status, critical, warning, info
    );

    for report in reports {
        let (i, w, c) = report.summary();
        html.push_str(&format!(
            r#"<div class="module">
<h2>📋 {} <small>({}ms · 严重:{} 警告:{} 信息:{})</small></h2>
"#,
            report.module, report.duration_ms, c, w, i
        ));

        for f in &report.findings {
            let severity = format!("{:?}", f.severity).to_lowercase();
            let badge = match f.severity {
                Severity::Critical => "严重",
                Severity::Warning => "警告",
                Severity::Info => "信息",
            };
            html.push_str(&format!(
                r#"<div class="finding {}">
<span class="badge {}">{}</span> <strong>{}</strong>
<p>{}</p>
"#,
                severity, severity, badge, f.title, f.description
            ));
            if !f.evidence.is_empty() {
                html.push_str(&format!(
                    r#"<div class="evidence">{}</div>"#,
                    html_escape(&f.evidence)
                ));
            }
            if let Some(ref fix) = f.fix {
                html.push_str(&format!(
                    r#"<div class="fix">💡 {}: <code>{}</code></div>"#,
                    html_escape(&fix.description),
                    html_escape(&fix.command)
                ));
            }
            html.push_str("</div>");
        }

        html.push_str("</div>");
    }

    html.push_str(
        r#"<footer>kylin-doctor 诊断报告</footer>
</body></html>"#,
    );

    html
}

