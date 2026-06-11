use clap::Args;
use colored::Colorize;
use kylin_doctor_core::{
    llm::tools, Config, KnowledgeStore, LlmProvider, Message, OllamaProvider, OpenAiCompatProvider,
};
use std::io::{self, BufRead, Write};
use crate::spinner::Spinner;
use crate::markdown::render_markdown;

#[derive(Args, Debug)]
pub struct ChatArgs {
    /// 用户的问题（单次提问模式）
    #[arg(trailing_var_arg = true)]
    pub question: Vec<String>,
}

const SYSTEM_PROMPT: &str = r#"你是银河麒麟桌面系统（Kylin OS）的 AI 诊断助手。你的职责是：

1. 回答用户关于银河麒麟系统的问题
2. 帮助用户诊断和解决系统问题
3. 解释系统命令和配置的含义
4. 提供系统维护和优化建议
5. 当需要了解系统状态时，使用提供的诊断工具进行扫描

回答要求：
- 简洁明了，优先给出可执行的命令或操作步骤
- 如果需要用户执行危险操作（如删除文件、修改系统配置），务必提醒风险
- 涉及 sudo 操作时明确告知需要管理员权限
- 如果不确定答案，诚实说明并建议用户查阅官方文档
- 当用户询问系统问题时，优先使用诊断工具获取实际数据，而非凭经验猜测"#;

/// 工具显示名称映射（参考 Claude Code 的简洁风格）
fn get_tool_display_name(tool_name: &str) -> &str {
    match tool_name {
        "scan_all" => "正在全面诊断系统",
        "scan_hardware" => "正在检测硬件",
        "scan_software" => "正在检查软件环境",
        "scan_security" => "正在安全审计",
        "scan_performance" => "正在分析性能",
        "scan_system" => "正在扫描系统",
        _ => "正在执行诊断",
    }
}

/// 创建 LLM 提供商
fn create_provider(config: &Config, provider_override: &str) -> Box<dyn LlmProvider> {
    match provider_override {
        "cloud" => {
            let cloud = &config.llm.cloud;
            match OpenAiCompatProvider::from_env(&cloud.endpoint, &cloud.model, &cloud.api_key_env)
            {
                Ok(p) => Box::new(p),
                Err(e) => {
                    eprintln!("⚠️  云端模型初始化失败: {}", e);
                    eprintln!("   回退到本地模型...");
                    Box::new(OllamaProvider::new(
                        &config.llm.local.endpoint,
                        &config.llm.local.model,
                    ))
                }
            }
        }
        "hybrid" => {
            // hybrid 模式：优先本地，不可用时回退云端
            let local = OllamaProvider::new(&config.llm.local.endpoint, &config.llm.local.model);
            // 我们在运行时检查可用性，这里先返回本地
            // 实际回退逻辑在 chat_with_tools 调用处
            Box::new(local)
        }
        _ => Box::new(OllamaProvider::new(
            &config.llm.local.endpoint,
            &config.llm.local.model,
        )),
    }
}

/// 创建云端提供商（用于 hybrid 回退）
fn create_cloud_provider(config: &Config) -> Option<Box<dyn LlmProvider>> {
    let cloud = &config.llm.cloud;
    OpenAiCompatProvider::from_env(&cloud.endpoint, &cloud.model, &cloud.api_key_env)
        .ok()
        .map(|p| Box::new(p) as Box<dyn LlmProvider>)
}

/// 执行 chat 命令
pub async fn execute(args: &ChatArgs, provider_name: &str) -> anyhow::Result<()> {
    let config = Config::load();

    // 隐私保护：离线模式检查
    if config.general.offline && provider_name == "cloud" {
        eprintln!("❌ 离线模式已启用 (offline=true)，无法使用云端模型。");
        eprintln!("   请使用 --provider local 或修改 ~/.kylin-doctor/config.toml");
        return Ok(());
    }

    let mut provider = create_provider(&config, provider_name);

    // 单次提问模式
    if !args.question.is_empty() {
        let question = args.question.join(" ");
        return ask_once(&*provider, &question, &config).await;
    }

    // 交互式对话模式
    println!("{}", "🤖 麒麟医生 AI 助手已就绪".bold().cyan());
    println!(
        "   模型: {} ({})",
        provider.name().dimmed(),
        provider_name.dimmed()
    );
    println!(
        "{}",
        "   输入问题开始对话，输入 'exit' 退出".dimmed()
    );
    println!(
        "{}",
        "   输入 'scan' 快速诊断系统，'help' 查看帮助".dimmed()
    );
    println!();

    // 检查可用性（hybrid 模式下自动回退）
    let mut actual_provider_name = provider_name.to_string();
    if !provider.is_available().await {
        if provider_name == "hybrid" || provider_name == "local" {
            if let Some(cloud) = create_cloud_provider(&config) {
                if cloud.is_available().await {
                    println!("⚠️  本地模型不可用，自动切换到云端模型");
                    provider = cloud;
                    actual_provider_name = "cloud".to_string();
                } else {
                    print_unavailable(provider_name, &config);
                    return Ok(());
                }
            } else {
                print_unavailable(provider_name, &config);
                return Ok(());
            }
        } else {
            print_unavailable(provider_name, &config);
            return Ok(());
        }
    }

    // 隐私提示：云端模式
    if actual_provider_name == "cloud" {
        println!(
            "{}",
            "⚠️  当前使用云端模型，对话内容将发送至第三方服务。"
                .yellow()
                .dimmed()
        );
        println!();
    }

    // 加载知识库上下文
    let knowledge_context = load_knowledge_context();

    let mut messages: Vec<Message> = vec![Message::system(&format!(
        "{}\n\n{}",
        SYSTEM_PROMPT,
        if knowledge_context.is_empty() {
            String::new()
        } else {
            format!(
                "## 参考知识库\n\n以下是从知识库中检索到的相关信息，可作为回答参考：\n\n{}",
                knowledge_context
            )
        }
    ))];

    let tools = tools::get_tool_definitions();

    loop {
        print!("{}", "🧑 你: ".bold().green());
        io::stdout().flush()?;

        let mut input = String::new();
        match io::stdin().lock().read_line(&mut input) {
            Ok(0) => break,
            Ok(_) => {}
            Err(_) => {
                break;
            }
        }

        let input = input.trim();
        if input.is_empty() {
            continue;
        }

        // 快捷命令
        match input {
            "exit" | "quit" | "退出" => {
                println!("{}", "👋 再见！".dimmed());
                break;
            }
            "help" | "帮助" => {
                print_help();
                continue;
            }
            "scan" | "扫描" => {
                let spinner = Spinner::new("正在全面诊断系统");
                spinner.start();
                match tools::execute_tool("scan_all") {
                    Ok(result) => {
                        spinner.stop(true);
                        println!();
                        println!("{}", render_markdown(&result));
                    }
                    Err(_) => {
                        spinner.stop(false);
                    }
                }
                println!();
                continue;
            }
            _ => {}
        }

        messages.push(Message::user(input));

        print!("{}", "🤖 助手: ".bold().blue());
        io::stdout().flush()?;

        // 带 Function Calling 的对话循环
        match provider.chat_with_tools(&messages, &tools).await {
            Ok(response) => {
                if let Some(ref tool_calls) = response.tool_calls {
                    // 有工具调用
                    messages.push(response.clone());

                    for tc in tool_calls {
                        let display_name = get_tool_display_name(&tc.function.name);
                        let spinner = Spinner::new(display_name);
                        spinner.start();

                        match tools::execute_tool(&tc.function.name) {
                            Ok(result) => {
                                spinner.stop(true);
                                messages.push(Message::tool_result(&tc.id, &result));
                            }
                            Err(e) => {
                                spinner.stop(false);
                                messages.push(Message::tool_result(
                                    &tc.id,
                                    &format!("工具执行失败: {}", e),
                                ));
                            }
                        }
                    }

                    // 工具结果返回给 LLM 生成最终回答
                    println!();
                    print!("{}", "🤖 助手: ".bold().blue());
                    io::stdout().flush()?;

                    // 流式输出最终回答
                    let full_response = std::sync::Arc::new(std::sync::Mutex::new(String::new()));
                    let full_response_clone = full_response.clone();
                    match provider
                        .chat_stream(&messages, Box::new(move |chunk: String| {
                            print!("{}", chunk);
                            io::stdout().flush().unwrap();
                            if let Ok(mut resp) = full_response_clone.lock() {
                                resp.push_str(&chunk);
                            }
                        }))
                        .await
                    {
                        Ok(_) => {
                            println!();
                            if let Ok(resp) = full_response.lock() {
                                messages.push(Message::assistant(&resp));
                            }
                        }
                        Err(e) => {
                            println!();
                            eprintln!("{} {}", "❌ 请求失败:".red(), e);
                            messages.pop(); // 移除用户消息
                        }
                    }
                } else {
                    // 普通文本回复 - 流式输出
                    println!();
                    let rendered = render_markdown(&response.content);
                    print!("{}", rendered);
                    messages.push(response);
                }
            }
            Err(e) => {
                // hybrid 回退
                if actual_provider_name == "hybrid" {
                    if let Some(cloud) = create_cloud_provider(&config) {
                        println!();
                        println!(
                            "{}",
                            "  ⚠️  本地模型失败，切换到云端...".yellow().dimmed()
                        );
                        print!("{}", "🤖 助手: ".bold().blue());
                        io::stdout().flush()?;

                        let full_response = std::sync::Arc::new(std::sync::Mutex::new(String::new()));
                        let full_response_clone = full_response.clone();
                        match cloud
                            .chat_stream(&messages, Box::new(move |chunk: String| {
                                print!("{}", chunk);
                                io::stdout().flush().unwrap();
                                if let Ok(mut resp) = full_response_clone.lock() {
                                    resp.push_str(&chunk);
                                }
                            }))
                            .await
                        {
                            Ok(_) => {
                                println!();
                                if let Ok(resp) = full_response.lock() {
                                    messages.push(Message::assistant(&resp));
                                }
                            }
                            Err(e2) => {
                                println!();
                                eprintln!("{} {}", "❌ 请求失败:".red(), e2);
                                messages.pop();
                            }
                        }
                        continue;
                    }
                }
                eprintln!("{} {}", "❌ 请求失败:".red(), e);
                messages.pop();
            }
        }
        println!();
    }

    Ok(())
}

/// 单次提问
async fn ask_once(
    provider: &dyn LlmProvider,
    question: &str,
    config: &Config,
) -> anyhow::Result<()> {
    if config.general.offline && provider.name() == "openai-compat" {
        eprintln!("❌ 离线模式已启用，无法使用云端模型。");
        return Ok(());
    }

    if !provider.is_available().await {
        eprintln!("❌ LLM 服务不可用，请检查模型配置。");
        return Ok(());
    }

    let tools = tools::get_tool_definitions();
    let mut messages = vec![Message::system(SYSTEM_PROMPT), Message::user(question)];

    // 带工具的单次提问
    match provider.chat_with_tools(&messages, &tools).await {
        Ok(response) => {
            if let Some(ref tool_calls) = response.tool_calls {
                messages.push(response.clone());
                for tc in tool_calls {
                    let display_name = get_tool_display_name(&tc.function.name);
                    let spinner = Spinner::new(display_name);
                    spinner.start();

                    if let Ok(result) = tools::execute_tool(&tc.function.name) {
                        spinner.stop(true);
                        messages.push(Message::tool_result(&tc.id, &result));
                    } else {
                        spinner.stop(false);
                    }
                }

                // 流式输出最终回答
                println!();
                let full_response = std::sync::Arc::new(std::sync::Mutex::new(String::new()));
                let full_response_clone = full_response.clone();
                match provider
                    .chat_stream(&messages, Box::new(move |chunk: String| {
                        print!("{}", chunk);
                        io::stdout().flush().unwrap();
                        if let Ok(mut resp) = full_response_clone.lock() {
                            resp.push_str(&chunk);
                        }
                    }))
                    .await
                {
                    Ok(_) => println!(),
                    Err(e) => {
                        println!();
                        eprintln!("❌ 请求失败: {}", e);
                    }
                }
            } else {
                // 流式输出普通回复
                println!();
                let rendered = render_markdown(&response.content);
                print!("{}", rendered);
            }
        }
        Err(e) => eprintln!("❌ 请求失败: {}", e),
    }

    Ok(())
}

fn print_unavailable(provider_name: &str, config: &Config) {
    let tip = match provider_name {
        "cloud" => "云端模型不可用，请检查 API Key 和网络连接。".to_string(),
        _ => format!(
            "本地 Ollama 服务不可达 ({})。\n\
             {}",
            config.llm.local.endpoint,
            "提示: 请先启动 Ollama 服务: ollama serve"
                .yellow()
                .to_string()
        ),
    };
    eprintln!("❌ {}", tip);
}

fn print_help() {
    println!("{}", "📖 可用命令：".bold());
    println!("  scan/扫描   - 快速全面诊断系统");
    println!("  help/帮助   - 显示此帮助信息");
    println!("  exit/退出   - 退出对话");
    println!();
    println!("{}", "💡 示例问题：".bold());
    println!("  我的系统为什么变慢了？");
    println!("  如何清理磁盘空间？");
    println!("  检查一下我的系统安全状态");
    println!("  SSH 配置有哪些安全风险？");
    println!();
}

/// 加载知识库上下文（简化版）
fn load_knowledge_context() -> String {
    let mut store = KnowledgeStore::new(KnowledgeStore::default_path());
    if store.load().is_err() {
        return String::new();
    }
    let stats = store.stats();
    if stats.total_docs == 0 {
        return String::new();
    }
    format!(
        "知识库中有 {} 个文档、{} 个分块可供参考。",
        stats.total_docs, stats.total_chunks
    )
}
