pub mod api;

use axum::response::Html;
use axum::routing::get;
use axum::Router;
use kylin_doctor_core::Config;
use std::sync::{Arc, Mutex};
use std::sync::atomic::AtomicUsize;

/// 内嵌的仪表盘 HTML
const DASHBOARD_HTML: &str = include_str!("dashboard.html");

/// CPU 采样数据
pub struct CpuSample {
    pub usage_pct: f64,
}

/// 应用共享状态
///
/// 注意：使用 `std::sync::Mutex` 而非 `tokio::sync::Mutex`，因为：
/// 1. 锁持有时间极短（仅读写一个 f64 字段）
/// 2. 从不跨 `.await` 持有锁
/// 3. `std::sync::Mutex` 在短临界区场景下性能优于 `tokio::sync::Mutex`
/// 如果未来需要跨 await 持有锁，应改用 `tokio::sync::Mutex`。
pub struct AppState {
    pub cpu: Arc<Mutex<CpuSample>>,
    pub config: Config,
    pub active_connections: AtomicUsize,
}

/// 最大并发 WebSocket 连接数
pub const MAX_CONCURRENT_CONNECTIONS: usize = 10;

/// 创建完整的 Web 路由（REST API + WebSocket + 前端页面）
///
/// CLI `serve` 命令和独立 Web 二进制共用此函数。
/// `app_state` 为 None 时，使用默认状态（CPU 使用率返回 0）。
pub fn create_router(app_state: Option<Arc<AppState>>) -> Router {
    let state = app_state.unwrap_or_else(|| {
        Arc::new(AppState {
            cpu: Arc::new(Mutex::new(CpuSample { usage_pct: 0.0 })),
            config: Config::default(),
            active_connections: AtomicUsize::new(0),
        })
    });

    Router::new()
        // 前端页面
        .route("/", get(dashboard))
        // REST API
        .route("/api/scan", get(api::scan_all).post(api::scan_all))
        .route("/api/scan/{module}", get(api::scan_module).post(api::scan_module))
        .route("/api/status", get(api::status_with_state))
        .route("/api/report/json", get(api::report_json))
        .route("/api/report/html", get(api::report_html))
        // WebSocket
        .route("/ws/scan", get(api::ws_scan_handler))
        .route("/ws/chat", get(api::ws_chat_handler))
        .with_state(state)
}

async fn dashboard() -> Html<&'static str> {
    Html(DASHBOARD_HTML)
}

/// CPU 采样后台任务：每 2 秒读取 /proc/stat 计算 CPU 使用率
pub fn spawn_cpu_sampler() -> Arc<Mutex<CpuSample>> {
    let cpu_state = Arc::new(Mutex::new(CpuSample { usage_pct: 0.0 }));
    let cpu_clone = cpu_state.clone();

    tokio::spawn(async move {
        let mut prev_idle: u64 = 0;
        let mut prev_total: u64 = 0;
        loop {
            tokio::time::sleep(std::time::Duration::from_secs(2)).await;
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

    cpu_state
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
                // idle + iowait 都算空闲（与 performance.rs 一致）
                let idle = parts[3] + parts[4];
                let total: u64 = parts.iter().sum();
                return Some((idle, total));
            }
        }
    }
    None
}
