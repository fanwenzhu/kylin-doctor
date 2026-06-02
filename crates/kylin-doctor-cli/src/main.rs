mod commands;

use clap::{Parser, Subcommand};

/// kylin-doctor — 银河麒麟桌面系统自我诊断工具
#[derive(Parser, Debug)]
#[command(name = "kylin-doctor", version, about)]
struct Cli {
    /// 输出详细程度: 0=简要, 1=标准, 2=详细
    #[arg(short, long, default_value = "1", global = true)]
    verbose: u8,

    /// LLM 提供商: local / cloud
    #[arg(short, long, default_value = "local", global = true)]
    provider: String,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// 全面系统扫描
    Scan(commands::scan::ScanArgs),
    /// 生成诊断报告
    Report,
    /// 进入 AI 对话模式
    Chat,
    /// 启动 Web 仪表盘
    Serve,
    /// 修复发现的问题
    Fix,
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Scan(args) => commands::scan::execute(&args, cli.verbose)?,
        Commands::Report => {
            println!("📋 报告功能尚未实现，敬请期待。");
        }
        Commands::Chat => {
            println!("🤖 AI 对话功能尚未实现，敬请期待。");
        }
        Commands::Serve => {
            println!("🌐 Web 仪表盘尚未实现，敬请期待。");
        }
        Commands::Fix => {
            println!("🔧 修复功能尚未实现，敬请期待。");
        }
    }

    Ok(())
}
