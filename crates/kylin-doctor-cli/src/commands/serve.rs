use clap::Args;
use colored::Colorize;

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
    use axum::extract::ws::{Message as WsMessage, WebSocket, WebSocketUpgrade};
    use axum::response::Html;
    use axum::routing::get;
    use axum::Router;

    use kylin_doctor_core::*;
    use std::net::SocketAddr;
    use tower_http::cors::CorsLayer;

    const DASHBOARD_HTML: &str = include_str!("../../../kylin-doctor-web/src/dashboard.html");

    // === API handlers (内联) ===

    async fn scan_all() -> axum::response::Json<serde_json::Value> {
        let detectors: Vec<Box<dyn Detector>> = vec![
            Box::new(SystemDetector::new()),
            Box::new(HardwareDetector::new()),
            Box::new(SoftwareDetector::new()),
            Box::new(SecurityDetector::new()),
            Box::new(PerformanceDetector::new()),
        ];
        let reports: Vec<serde_json::Value> = detectors.iter().map(|d| {
            match d.scan() {
                Ok(r) => {
                    let (i, w, c) = r.summary();
                    serde_json::json!({
                        "module": r.module,
                        "findings": r.findings.iter().map(|f| serde_json::json!({
                            "id": f.id, "module": f.module,
                            "severity": format!("{:?}", f.severity).to_lowercase(),
                            "title": f.title, "description": f.description, "evidence": f.evidence,
                            "fix": f.fix.as_ref().map(|fix| serde_json::json!({
                                "description": fix.description, "command": fix.command, "risk_level": fix.risk_level
                            })),
                            "auto_fixable": f.auto_fixable
                        })).collect::<Vec<_>>(),
                        "duration_ms": r.duration_ms,
                        "summary": {"info": i, "warning": w, "critical": c}
                    })
                }
                Err(e) => serde_json::json!({"module": d.name(), "error": e.to_string(), "findings": [], "duration_ms": 0, "summary": {"info":0,"warning":0,"critical":0}})
            }
        }).collect();
        axum::response::Json(serde_json::json!(reports))
    }

    async fn scan_module(axum::extract::Path(module): axum::extract::Path<String>) -> Result<axum::response::Json<serde_json::Value>, axum::http::StatusCode> {
        let detector: Option<Box<dyn Detector>> = match module.as_str() {
            "system" => Some(Box::new(SystemDetector::new())),
            "hardware" => Some(Box::new(HardwareDetector::new())),
            "software" => Some(Box::new(SoftwareDetector::new())),
            "security" => Some(Box::new(SecurityDetector::new())),
            "performance" => Some(Box::new(PerformanceDetector::new())),
            _ => None,
        };
        match detector {
            Some(d) => match d.scan() {
                Ok(r) => {
                    let (i, w, c) = r.summary();
                    Ok(axum::response::Json(serde_json::json!({
                        "module": r.module, "duration_ms": r.duration_ms,
                        "summary": {"info": i, "warning": w, "critical": c},
                        "findings": r.findings.iter().map(|f| serde_json::json!({
                            "id": f.id, "module": f.module,
                            "severity": format!("{:?}", f.severity).to_lowercase(),
                            "title": f.title, "description": f.description, "evidence": f.evidence,
                            "fix": f.fix.as_ref().map(|fix| serde_json::json!({
                                "description": fix.description, "command": fix.command, "risk_level": fix.risk_level
                            })),
                            "auto_fixable": f.auto_fixable
                        })).collect::<Vec<_>>()
                    })))
                }
                Err(_) => Err(axum::http::StatusCode::INTERNAL_SERVER_ERROR),
            },
            None => Err(axum::http::StatusCode::NOT_FOUND),
        }
    }

    async fn status() -> axum::response::Json<serde_json::Value> {
        let hostname = std::fs::read_to_string("/etc/hostname").map(|s| s.trim().to_string()).unwrap_or_else(|_| "unknown".to_string());
        let kernel = std::fs::read_to_string("/proc/version").ok().and_then(|v| v.split_whitespace().nth(2).map(|s| s.to_string())).unwrap_or_else(|| "unknown".to_string());
        let uptime = std::fs::read_to_string("/proc/uptime").ok().and_then(|s| s.split_whitespace().next()?.parse::<f64>().ok()).unwrap_or(0.0);
        let loadavg = std::fs::read_to_string("/proc/loadavg").ok().map(|s| s.trim().to_string()).unwrap_or_default();
        let meminfo = std::fs::read_to_string("/proc/meminfo").unwrap_or_default();
        let mut mem_total = 0u64; let mut mem_avail = 0u64;
        for line in meminfo.lines() {
            if line.starts_with("MemTotal:") { mem_total = line.split_whitespace().nth(1).and_then(|v| v.parse().ok()).unwrap_or(0); }
            else if line.starts_with("MemAvailable:") { mem_avail = line.split_whitespace().nth(1).and_then(|v| v.parse().ok()).unwrap_or(0); }
        }
        let mem_pct = if mem_total > 0 { ((mem_total - mem_avail) as f64 / mem_total as f64 * 100.0) as u32 } else { 0 };
        axum::response::Json(serde_json::json!({
            "hostname": hostname, "kernel": kernel, "uptime_hours": format!("{:.1}", uptime/3600.0),
            "loadavg": loadavg, "cpu_usage_pct": 0,
            "memory": {"total_mb": mem_total/1024, "available_mb": mem_avail/1024, "usage_pct": mem_pct}
        }))
    }

    async fn ws_scan(ws: WebSocketUpgrade) -> axum::response::Response {
        ws.on_upgrade(|mut socket: WebSocket| async move {
            let modules = ["system","hardware","software","security","performance"];
            let _ = socket.send(WsMessage::Text(serde_json::json!({"type":"start","total":modules.len()}).to_string().into())).await;
            for (i, m) in modules.iter().enumerate() {
                let _ = socket.send(WsMessage::Text(serde_json::json!({"type":"progress","module":m,"current":i+1,"total":modules.len()}).to_string().into())).await;
                let d: Option<Box<dyn Detector + Send>> = match *m {
                    "system" => Some(Box::new(SystemDetector::new())),
                    "hardware" => Some(Box::new(HardwareDetector::new())),
                    "software" => Some(Box::new(SoftwareDetector::new())),
                    "security" => Some(Box::new(SecurityDetector::new())),
                    "performance" => Some(Box::new(PerformanceDetector::new())),
                    _ => None,
                };
                if let Some(d) = d {
                    if let Ok(r) = d.scan() {
                        let (i,w,c) = r.summary();
                        let val = serde_json::json!({"module":r.module,"duration_ms":r.duration_ms,"summary":{"info":i,"warning":w,"critical":c},
                            "findings":r.findings.iter().map(|f| serde_json::json!({"id":f.id,"severity":format!("{:?}",f.severity).to_lowercase(),"title":f.title,"description":f.description,"evidence":f.evidence,"auto_fixable":f.auto_fixable})).collect::<Vec<_>>()});
                        let _ = socket.send(WsMessage::Text(serde_json::json!({"type":"result","module":m,"data":val}).to_string().into())).await;
                    }
                }
            }
            let _ = socket.send(WsMessage::Text(serde_json::json!({"type":"done"}).to_string().into())).await;
        })
    }

    // === 路由 ===

    let app = Router::new()
        .route("/", get(|| async { Html(DASHBOARD_HTML) }))
        .route("/api/scan", get(scan_all))
        .route("/api/scan/{module}", get(scan_module))
        .route("/api/status", get(status))
        .route("/ws/scan", get(ws_scan))
        .layer(CorsLayer::permissive());

    let addr = SocketAddr::new(args.host.parse()?, args.port);
    println!("{}", "🌐 kylin-doctor Web 仪表盘已启动".bold().cyan());
    println!("   地址: http://{}", addr);
    println!("   按 Ctrl+C 停止");

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}
