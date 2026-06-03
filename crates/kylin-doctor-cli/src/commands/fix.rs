use clap::Args;
use colored::Colorize;
use kylin_doctor_core::{
    Detector, Finding, HardwareDetector, PerformanceDetector, SecurityDetector,
    Severity, SoftwareDetector, SystemDetector,
};
use std::io::{self, BufRead, Write};

#[derive(Args, Debug)]
pub struct FixArgs {
    /// 只修复指定模块的问题
    #[arg(short, long, value_enum)]
    pub module: Option<FixModule>,

    /// 预览修复操作，不实际执行
    #[arg(long)]
    pub dry_run: bool,

    /// 跳过确认，直接修复（危险模式）
    #[arg(short, long)]
    pub yes: bool,

    /// 只修复可自动修复的问题
    #[arg(long)]
    pub auto_only: bool,

    /// 只修复严重问题
    #[arg(long)]
    pub critical_only: bool,
}

#[derive(clap::ValueEnum, Clone, Debug)]
pub enum FixModule {
    System,
    Hardware,
    Software,
    Security,
    Performance,
}

pub fn execute(args: &FixArgs) -> anyhow::Result<()> {
    println!("{}", "🔧 kylin-doctor 修复工具".bold().cyan());
    println!();

    // 1. 先扫描
    eprintln!("{}", "正在扫描系统问题...".dimmed());
    let detectors: Vec<(&str, Box<dyn Detector>)> = match args.module {
        Some(FixModule::System) => vec![("system", Box::new(SystemDetector::new()))],
        Some(FixModule::Hardware) => vec![("hardware", Box::new(HardwareDetector::new()))],
        Some(FixModule::Software) => vec![("software", Box::new(SoftwareDetector::new()))],
        Some(FixModule::Security) => vec![("security", Box::new(SecurityDetector::new()))],
        Some(FixModule::Performance) => vec![("performance", Box::new(PerformanceDetector::new()))],
        None => vec![
            ("system", Box::new(SystemDetector::new())),
            ("hardware", Box::new(HardwareDetector::new())),
            ("software", Box::new(SoftwareDetector::new())),
            ("security", Box::new(SecurityDetector::new())),
            ("performance", Box::new(PerformanceDetector::new())),
        ],
    };

    let mut all_fixable: Vec<FixableItem> = Vec::new();

    for (name, detector) in &detectors {
        let report = detector.scan()?;
        for finding in &report.findings {
            // 过滤条件
            if args.auto_only && !finding.auto_fixable {
                continue;
            }
            if args.critical_only && finding.severity != Severity::Critical {
                continue;
            }
            if finding.fix.is_some() {
                all_fixable.push(FixableItem {
                    module: name.to_string(),
                    finding: finding.clone(),
                });
            }
        }
    }

    if all_fixable.is_empty() {
        println!("{}", "✅ 没有发现可修复的问题。".green().bold());
        return Ok(());
    }

    // 2. 列出可修复的问题
    println!(
        "{}",
        format!("发现 {} 个可修复的问题：", all_fixable.len())
            .bold()
            .yellow()
    );
    println!();

    for (i, item) in all_fixable.iter().enumerate() {
        let icon = match item.finding.severity {
            Severity::Critical => "🔴",
            Severity::Warning => "⚠️ ",
            Severity::Info => "ℹ️ ",
        };
        let auto = if item.finding.auto_fixable {
            " [自动]".green()
        } else {
            " [手动]".dimmed()
        };
        println!(
            "  {}. {} [{}] {} {}",
            i + 1,
            icon,
            item.module.dimmed(),
            item.finding.title,
            auto
        );
        if let Some(ref fix) = item.finding.fix {
            println!(
                "     💡 {}  风险: {}",
                fix.description,
                risk_color(&fix.risk_level)
            );
            if args.dry_run || cli_verbose() >= 2 {
                println!("     📝 {}", fix.command.dimmed());
            }
        }
        println!();
    }

    // 3. 确认
    if args.dry_run {
        println!("{}", "📋 预览模式，未执行任何修复。".dimmed());
        return Ok(());
    }

    if !args.yes {
        print!(
            "{}",
            "是否执行以上修复？[y/N] ".bold().yellow()
        );
        io::stdout().flush()?;

        let mut input = String::new();
        io::stdin().lock().read_line(&mut input)?;
        let input = input.trim().to_lowercase();

        if input != "y" && input != "yes" {
            println!("{}", "已取消。".dimmed());
            return Ok(());
        }
    }

    // 4. 执行修复
    println!();
    println!("{}", "执行修复中...".bold());
    println!();

    let mut success_count = 0;
    let mut fail_count = 0;

    for item in &all_fixable {
        if item.finding.fix.is_some() {
            print!(
                "  {} [{}] {} ... ",
                "修复".cyan(),
                item.module.dimmed(),
                item.finding.title
            );
            io::stdout().flush()?;

            // 找到对应的 detector 来执行 fix
            let detector: Box<dyn Detector> = match item.module.as_str() {
                "system" => Box::new(SystemDetector::new()),
                "hardware" => Box::new(HardwareDetector::new()),
                "software" => Box::new(SoftwareDetector::new()),
                "security" => Box::new(SecurityDetector::new()),
                "performance" => Box::new(PerformanceDetector::new()),
                _ => {
                    println!("{}", "跳过（未知模块）".yellow());
                    fail_count += 1;
                    continue;
                }
            };

            match detector.fix(&item.finding) {
                Ok(true) => {
                    println!("{}", "✅ 成功".green());
                    success_count += 1;
                }
                Ok(false) => {
                    println!("{}", "❌ 失败".red());
                    fail_count += 1;
                }
                Err(e) => {
                    println!("{} {}", "❌ 错误:".red(), e);
                    fail_count += 1;
                }
            }
        }
    }

    // 5. 总结
    println!();
    println!("{}", "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━".dimmed());
    println!(
        "修复完成: {} 成功, {} 失败",
        success_count.to_string().green().bold(),
        fail_count.to_string().red()
    );

    if success_count > 0 {
        println!(
            "{}",
            "建议重新运行 kylin-doctor scan 验证修复结果。".dimmed()
        );
    }

    Ok(())
}

struct FixableItem {
    module: String,
    finding: Finding,
}

fn risk_color(risk: &str) -> colored::ColoredString {
    match risk {
        "high" => "高".red().bold(),
        "medium" => "中".yellow(),
        _ => "低".green(),
    }
}

fn cli_verbose() -> u8 {
    // 简单获取 verbose 级别
    std::env::args()
        .position(|a| a == "--verbose" || a == "-v")
        .and_then(|i| std::env::args().nth(i + 1))
        .and_then(|v| v.parse().ok())
        .unwrap_or(1)
}
