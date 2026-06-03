use clap::{Args, Subcommand};
use colored::Colorize;
use kylin_doctor_core::{Config, KnowledgeStore, LlmProvider, OllamaProvider};

#[derive(Args, Debug)]
pub struct KnowledgeArgs {
    #[command(subcommand)]
    pub command: KnowledgeCommand,
}

#[derive(Subcommand, Debug)]
pub enum KnowledgeCommand {
    /// 添加文档到知识库
    Add {
        /// 文件或目录路径
        path: String,
        /// 递归扫描目录
        #[arg(short, long)]
        recursive: bool,
    },
    /// 列出知识库中的文档
    List,
    /// 显示知识库统计信息
    Status,
    /// 删除文档
    Remove {
        /// 文档 ID
        doc_id: String,
    },
    /// 生成向量嵌入
    Embed,
    /// 测试检索
    Test {
        /// 查询文本
        query: String,
    },
}

pub async fn execute(args: &KnowledgeArgs) -> anyhow::Result<()> {
    let config = Config::load();
    let mut store = KnowledgeStore::new(KnowledgeStore::default_path());
    store.init()?;
    store.load()?;

    match &args.command {
        KnowledgeCommand::Add { path, recursive } => {
            let path_obj = std::path::Path::new(path);
            if path_obj.is_dir() {
                if *recursive {
                    println!("📂 递归添加目录: {}", path);
                    let ids = store.add_directory(path)?;
                    println!("✅ 添加了 {} 个文档", ids.len());
                } else {
                    println!("📂 添加目录: {}", path);
                    let ids = store.add_directory_shallow(path)?;
                    println!("✅ 添加了 {} 个文档", ids.len());
                }
            } else {
                println!("📄 添加文件: {}", path);
                let id = store.add_file(path)?;
                println!("✅ 文档 ID: {}", id);
            }
        }

        KnowledgeCommand::List => {
            let docs = store.list_documents();
            if docs.is_empty() {
                println!("📭 知识库为空。使用 'kylin-doctor knowledge add <路径>' 添加文档。");
            } else {
                println!("📚 知识库文档列表：");
                println!();
                for doc in docs {
                    let chunks_with_emb = doc.chunks.iter().filter(|c| c.embedding.is_some()).count();
                    println!(
                        "  [{}] {} ({} chunks, {} 已向量化)",
                        doc.id.dimmed(),
                        doc.title.bold(),
                        doc.chunks.len(),
                        chunks_with_emb
                    );
                    println!("      来源: {}", doc.source.dimmed());
                    println!("      添加: {}", doc.added_at.dimmed());
                }
            }
        }

        KnowledgeCommand::Status => {
            let stats = store.stats();
            println!("📊 知识库统计：");
            println!("   文档数量: {}", stats.total_docs);
            println!("   分块数量: {}", stats.total_chunks);
            println!(
                "   已向量化: {} ({})",
                stats.embedded_chunks,
                if stats.total_chunks > 0 {
                    format!("{:.0}%", stats.embedded_chunks as f64 / stats.total_chunks as f64 * 100.0)
                } else {
                    "0%".to_string()
                }
            );
        }

        KnowledgeCommand::Remove { doc_id } => {
            store.remove_document(doc_id)?;
            println!("✅ 已删除文档: {}", doc_id);
        }

        KnowledgeCommand::Embed => {
            let provider = OllamaProvider::new(
                &config.llm.local.endpoint,
                &config.llm.local.model,
            );

            if !provider.is_available().await {
                eprintln!("❌ Ollama 服务不可用，请先启动: ollama serve");
                return Ok(());
            }

            println!("🔄 正在生成向量嵌入...");
            let count = store.embed_all(&provider).await?;
            println!("✅ 完成，新向量化 {} 个分块", count);

            let stats = store.stats();
            println!(
                "   总计: {} 文档, {} 分块, {} 已向量化",
                stats.total_docs, stats.total_chunks, stats.embedded_chunks
            );
        }

        KnowledgeCommand::Test { query } => {
            // 先尝试向量检索
            let provider = OllamaProvider::new(
                &config.llm.local.endpoint,
                &config.llm.local.model,
            );

            if provider.is_available().await {
                match provider.embed(&[query.clone()]).await {
                    Ok(embeddings) => {
                        if let Some(query_emb) = embeddings.into_iter().next() {
                            let results = store.search(&query_emb, 5);
                            if !results.is_empty() {
                                println!("🔍 向量检索结果（查询: \"{}\"）：", query);
                                println!();
                                for (i, r) in results.iter().enumerate() {
                                    println!(
                                        "  {}. [相似度: {:.3}] {}",
                                        i + 1,
                                        r.score,
                                        r.source.dimmed()
                                    );
                                    // 截断显示
                                    let preview = if r.chunk_content.len() > 200 {
                                        format!("{}...", &r.chunk_content[..200])
                                    } else {
                                        r.chunk_content.clone()
                                    };
                                    println!("     {}", preview);
                                    println!();
                                }
                                return Ok(());
                            }
                        }
                    }
                    Err(e) => {
                        eprintln!("⚠️  向量化失败，回退到关键词检索: {}", e);
                    }
                }
            }

            // 回退到关键词检索
            let results = store.search_keyword(query, 5);
            if results.is_empty() {
                println!("🔍 未找到与 \"{}\" 相关的内容", query);
            } else {
                println!("🔍 关键词检索结果（查询: \"{}\"）：", query);
                println!();
                for (i, r) in results.iter().enumerate() {
                    println!("  {}. [匹配度: {:.3}] {}", i + 1, r.score, r.source.dimmed());
                    let preview = if r.chunk_content.len() > 200 {
                        format!("{}...", &r.chunk_content[..200])
                    } else {
                        r.chunk_content.clone()
                    };
                    println!("     {}", preview);
                    println!();
                }
            }
        }
    }

    Ok(())
}
