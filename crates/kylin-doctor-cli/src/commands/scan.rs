use clap::Args;
use colored::Colorize;
use indicatif::{ProgressBar, ProgressStyle};
use kylin_doctor_core::{Detector, ScanReport, Severity, SystemDetector};
use std::time::Duration;

#[derive(Args, Debug)]
pub struct ScanArgs {
    /// 只扫描指定模块
    #[arg(short, long, value_enum)]
    pub module: Option<Module>,

    /// 快速扫描（跳过耗时项）
    #[arg(short, long)]
    pub quick: bool,
}

#[derive(clap::ValueEnum, Clone, Debug)]
pub enum Module {
    System,
}

/// 执行扫描
pub fn execute(args: &ScanArgs, verbose: u8) -> anyhow::Result<()> {
    println!("{}", "🔍 kylin-doctor 系统诊断".bold().cyan());
    println!();

    let detectors: Vec<Box<dyn Detector>> = match args.module {
        Some(Module::System) | None => {
            vec![Box::new(SystemDetector::new())]
        }
    };

    let pb = ProgressBar::new(detectors.len() as u64);
    pb.set_style(
        ProgressStyle::default_bar()
            .template("├── [{bar:20}] {pos}/{len} {msg}")
            .unwrap()
            .progress_chars("█▓░"),
    );

    let mut reports: Vec<ScanReport> = Vec::new();

    for detector in &detectors {
        pb.set_message(format!("正在扫描 {}...", detector.name()));
        let report = detector.scan()?;
        reports.push(report);
        pb.inc(1);
        std::thread::sleep(Duration::from_millis(100)); // 短暂延迟让进度条可见
    }

    pb.finish_and_clear();
    println!("└── {}", "扫描完成".green().bold());
    println!();

    // 输出结果
    for report in &reports {
        print_report(report, verbose);
    }

    // 总结
    print_summary(&reports);

    Ok(())
}

fn print_report(report: &ScanReport, verbose: u8) {
    let (_info, warning, critical) = report.summary();
    let status = if critical > 0 {
        format!("🔴 {} 个严重", critical).red().bold().to_string()
    } else if warning > 0 {
        format!("⚠️  {} 个警告", warning).yellow().bold().to_string()
    } else {
        "✅ 正常".green().bold().to_string()
    };

    println!("📋 {} [{}]", report.module.bold(), status);
    println!("   扫描耗时: {}ms", report.duration_ms);

    if report.findings.is_empty() {
        println!("   未发现问题");
        println!();
        return;
    }

    for finding in &report.findings {
        let icon = match finding.severity {
            Severity::Critical => "🔴",
            Severity::Warning => "⚠️ ",
            Severity::Info => "ℹ️ ",
        };

        match verbose {
            0 => {
                // 普通用户：只显示问题和建议
                println!("   {} {}", icon, finding.title);
                if let Some(ref fix) = finding.fix {
                    println!("      💡 建议: {}", fix.description);
                }
            }
            1 => {
                // 管理员：显示检测项名称和结果
                println!("   {} [{}] {}", icon, finding.id.dimmed(), finding.title);
                println!("      {}", finding.description);
                if let Some(ref fix) = finding.fix {
                    println!("      💡 修复: {}", fix.command.dimmed());
                }
            }
            _ => {
                // 开发者：显示完整技术细节
                println!("   {} [{}] {}", icon, finding.id.dimmed(), finding.title);
                println!("      描述: {}", finding.description);
                println!("      证据:");
                for line in finding.evidence.lines() {
                    println!("        {}", line.dimmed());
                }
                if let Some(ref fix) = finding.fix {
                    println!("      修复命令: {}", fix.command);
                    println!("      风险等级: {}", fix.risk_level);
                }
                println!("      可自动修复: {}", if finding.auto_fixable { "是" } else { "否" });
            }
        }
        println!();
    }
}

fn print_summary(reports: &[ScanReport]) {
    let mut total_info = 0;
    let mut total_warning = 0;
    let mut total_critical = 0;

    for report in reports {
        let (i, w, c) = report.summary();
        total_info += i;
        total_warning += w;
        total_critical += c;
    }

    println!("{}", "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━".dimmed());

    if total_critical > 0 {
        println!(
            "🔴 严重: {}  ⚠️  警告: {}  ℹ️  信息: {}",
            total_critical.to_string().red().bold(),
            total_warning.to_string().yellow(),
            total_info.to_string().dimmed()
        );
        println!(
            "{}",
            "存在严重问题，建议立即处理。运行 kylin-doctor fix 可尝试自动修复。"
                .red()
                .bold()
        );
    } else if total_warning > 0 {
        println!(
            "⚠️  警告: {}  ℹ️  信息: {}",
            total_warning.to_string().yellow().bold(),
            total_info.to_string().dimmed()
        );
        println!(
            "{}",
            "系统基本正常，但有警告项需要关注。".yellow()
        );
    } else {
        println!(
            "{}",
            "✅ 系统一切正常，未发现需要关注的问题。".green().bold()
        );
    }
}
