use clap::Args;
use colored::Colorize;
use kylin_doctor_web::create_router;
use std::net::SocketAddr;

#[derive(Args, Debug)]
pub struct ServeArgs {
    /// 监听端口
    #[arg(short, long, default_value = "8080")]
    pub port: u16,

    /// 监听地址
    #[arg(long, default_value = "127.0.0.1")]
    pub host: String,
}

pub async fn execute(args: &ServeArgs) -> anyhow::Result<()> {
    // CLI 模式无 CPU 采样，传 None 即可
    let app = create_router(None);

    let addr = SocketAddr::new(args.host.parse()?, args.port);
    println!("{}", "🌐 kylin-doctor Web 仪表盘已启动".bold().cyan());
    println!("   地址: http://{}", addr);
    println!("   按 Ctrl+C 停止");

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}
