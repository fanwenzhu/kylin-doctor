use kylin_doctor_core::Config;
use kylin_doctor_web::{create_router, spawn_cpu_sampler, AppState};
use std::net::SocketAddr;
use std::sync::Arc;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let config = Config::load();

    // 优先使用环境变量，其次使用配置文件
    let host = std::env::var("HOST").unwrap_or_else(|_| config.web.host.clone());
    let port: u16 = std::env::var("PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(config.web.port);

    // 后台 CPU 采样
    let cpu_state = spawn_cpu_sampler();
    let app_state = Arc::new(AppState { cpu: cpu_state });

    let app = create_router(Some(app_state));

    let addr = SocketAddr::new(host.parse()?, port);
    println!("🌐 kylin-doctor Web 仪表盘已启动");
    println!("   地址: http://{}", addr);
    println!("   WebSocket: ws://{}/ws/scan", addr);
    println!("   按 Ctrl+C 停止");

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}
