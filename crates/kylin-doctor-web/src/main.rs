mod api;

use axum::response::Html;
use axum::routing::get;
use axum::Router;
use kylin_doctor_core::Config;
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tower_http::cors::CorsLayer;

/// 内嵌的仪表盘 HTML
const DASHBOARD_HTML: &str = include_str!("dashboard.html");

/// CPU 采样数据
pub struct CpuSample {
    pub usage_pct: f64,
}

/// 应用共享状态
pub struct AppState {
    pub cpu: Arc<Mutex<CpuSample>>,
}

/// 读取 /proc/stat 中的 idle 和 total 值
fn read_cpu_stat() -> Option<(u64, u64)> {
    let stat = std::fs::read_to_string("/proc/stat").ok()?;
    for line in stat.lines() {
        if line.starts_with("cpu ") {
            let parts: Vec<u64> = line
                .split_whitespace()
                .skip(1)
                .filter_map(|v| v.parse().ok())
                .collect();
            if parts.len() >= 5 {
                // idle 只算 parts[3]（idle），不算 iowait（parts[4]）
                // iowait 表示 CPU 在等待 I/O，不是真正的空闲
                let idle = parts[3];
                let total: u64 = parts.iter().sum();
                return Some((idle, total));
            }
        }
    }
    None
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let config = Config::load();

    // 优先使用环境变量，其次使用配置文件
    let host = std::env::var("HOST").unwrap_or_else(|_| config.web.host.clone());
    let port: u16 = std::env::var("PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(config.web.port);

    // 初始化 CPU 共享状态
    let cpu_state = Arc::new(Mutex::new(CpuSample { usage_pct: 0.0 }));
    let cpu_clone = cpu_state.clone();

    // 后台 CPU 采样任务：每 2 秒采样，计算使用率
    tokio::spawn(async move {
        let mut prev_idle: u64 = 0;
        let mut prev_total: u64 = 0;
        loop {
            tokio::time::sleep(Duration::from_secs(2)).await;
            if let Some((idle, total)) = read_cpu_stat() {
                if prev_total > 0 {
                    let total_diff = total - prev_total;
                    let idle_diff = idle - prev_idle;
                    if total_diff > 0 {
                        let usage =
                            ((total_diff - idle_diff) as f64 / total_diff as f64) * 100.0;
                        if let Ok(mut sample) = cpu_clone.lock() {
                            sample.usage_pct = usage.round();
                        }
                    }
                }
                prev_idle = idle;
                prev_total = total;
            }
        }
    });

    let app_state = Arc::new(AppState { cpu: cpu_state });

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
        .layer(CorsLayer::permissive())
        .with_state(app_state);

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
