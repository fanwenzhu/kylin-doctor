mod api;

use axum::response::Html;
use axum::routing::get;
use axum::Router;
use kylin_doctor_core::Config;
use std::net::SocketAddr;
use tower_http::cors::CorsLayer;

/// 内嵌的仪表盘 HTML
const DASHBOARD_HTML: &str = include_str!("dashboard.html");

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let config = Config::load();

    // 优先使用环境变量，其次使用配置文件
    let host = std::env::var("HOST").unwrap_or_else(|_| config.web.host.clone());
    let port: u16 = std::env::var("PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(config.web.port);

    let app = Router::new()
        // 前端页面
        .route("/", get(dashboard))
        // REST API
        .route("/api/scan", get(api::scan_all).post(api::scan_all))
        .route("/api/scan/{module}", get(api::scan_module).post(api::scan_module))
        .route("/api/status", get(api::status))
        .route("/api/report/json", get(api::report_json))
        .route("/api/report/html", get(api::report_html))
        // WebSocket
        .route("/ws/scan", get(api::ws_scan_handler))
        .route("/ws/chat", get(api::ws_chat_handler))
        // CORS
        .layer(CorsLayer::permissive());

    let addr = SocketAddr::new(host.parse()?, port);
    println!("🌐 kylin-doctor Web 仪表盘已启动");
    println!("   地址: http://{}", addr);
    println!("   WebSocket: ws://{}/ws/scan", addr);
    println!("   按 Ctrl+C 停止");

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}

async fn dashboard() -> Html<&'static str> {
    Html(DASHBOARD_HTML)
}
