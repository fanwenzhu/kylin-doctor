mod commands;
mod spinner;
mod markdown;

use clap::{Parser, Subcommand};

/// kylin-doctor — 银河麒麟桌面系统自我诊断工具
#[derive(Parser, Debug)]
#[command(name = "kylin-doctor", version, about)]
struct Cli {
    /// 输出详细程度: 0=简要, 1=标准, 2=详细
    #[arg(short, long, default_value = "1", global = true)]
    verbose: u8,

    /// LLM 提供商: local / cloud / hybrid
    #[arg(short, long, default_value = "local", global = true)]
    provider: String,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// 全面系统扫描
    Scan(commands::scan::ScanArgs),
    /// 生成诊断报告 (JSON/HTML)
    Report(commands::report::ReportArgs),
    /// 进入 AI 对话模式（也可单次提问: kylin-doctor chat 你的问题）
    Chat(commands::chat::ChatArgs),
    /// 知识库管理（添加文档、生成向量、测试检索）
    Knowledge(commands::knowledge::KnowledgeArgs),
    /// 启动 Web 仪表盘
    Serve(commands::serve::ServeArgs),
    /// 修复发现的问题
    Fix(commands::fix::FixArgs),
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Scan(args) => {
            let exit_code = commands::scan::execute(&args, cli.verbose)?;
            std::process::exit(exit_code);
        }
        Commands::Report(args) => commands::report::execute(&args)?,
        Commands::Chat(args) => commands::chat::execute(&args, &cli.provider).await?,
        Commands::Knowledge(args) => commands::knowledge::execute(&args).await?,
        Commands::Serve(args) => commands::serve::execute(&args).await?,
        Commands::Fix(args) => commands::fix::execute(&args, cli.verbose)?,
    }

    Ok(())
}
