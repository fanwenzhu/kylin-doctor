use axum::extract::ws::{Message as WsMessage, WebSocket, WebSocketUpgrade};
use axum::extract::Path;
use axum::http::StatusCode;
use axum::response::{Html, Json};
use futures_util::{SinkExt, StreamExt};
use kylin_doctor_core::{
    llm::tools, Config, Detector, HardwareDetector, LlmProvider, Message as LlmMessage,
    OllamaProvider, PerformanceDetector, ScanReport, SecurityDetector, SoftwareDetector,
    SystemDetector,
};
use serde_json::{json, Value};

// ==================== REST API ====================

/// 全量扫描所有模块
pub async fn scan_all() -> Result<Json<Value>, StatusCode> {
    let detectors: Vec<Box<dyn Detector>> = vec![
        Box::new(SystemDetector::new()),
        Box::new(HardwareDetector::new()),
        Box::new(SoftwareDetector::new()),
        Box::new(SecurityDetector::new()),
        Box::new(PerformanceDetector::new()),
    ];

    let reports = run_detectors(&detectors);
    Ok(Json(json!(reports)))
}

/// 扫描指定模块
pub async fn scan_module(Path(module): Path<String>) -> Result<Json<Value>, StatusCode> {
    let detector: Option<Box<dyn Detector>> = match module.as_str() {
        "system" => Some(Box::new(SystemDetector::new())),
        "hardware" => Some(Box::new(HardwareDetector::new())),
        "software" => Some(Box::new(SoftwareDetector::new())),
        "security" => Some(Box::new(SecurityDetector::new())),
        "performance" => Some(Box::new(PerformanceDetector::new())),
        _ => None,
    };

    match detector {
        Some(d) => {
            let report = run_single(&*d);
            Ok(Json(json!(report)))
        }
        None => Err(StatusCode::NOT_FOUND),
    }
}

/// 系统概览（快速信息，不执行完整扫描）
pub async fn status() -> Json<Value> {
    let hostname = std::fs::read_to_string("/etc/hostname")
        .map(|s| s.trim().to_string())
        .unwrap_or_else(|_| "unknown".to_string());

    let kernel = std::fs::read_to_string("/proc/version")
        .ok()
        .and_then(|v| v.split_whitespace().nth(2).map(|s| s.to_string()))
        .unwrap_or_else(|| "unknown".to_string());

    let uptime_secs = std::fs::read_to_string("/proc/uptime")
        .ok()
        .and_then(|s| s.split_whitespace().next()?.parse::<f64>().ok())
        .unwrap_or(0.0);

    let loadavg = std::fs::read_to_string("/proc/loadavg")
        .ok()
        .map(|s| s.trim().to_string())
        .unwrap_or_default();

    let meminfo = std::fs::read_to_string("/proc/meminfo").unwrap_or_default();
    let mut mem_total = 0u64;
    let mut mem_available = 0u64;
    for line in meminfo.lines() {
        if line.starts_with("MemTotal:") {
            mem_total = line
                .split_whitespace()
                .nth(1)
                .and_then(|v| v.parse().ok())
                .unwrap_or(0);
        } else if line.starts_with("MemAvailable:") {
            mem_available = line
                .split_whitespace()
                .nth(1)
                .and_then(|v| v.parse().ok())
                .unwrap_or(0);
        }
    }
    let mem_usage = if mem_total > 0 {
        ((mem_total - mem_available) as f64 / mem_total as f64 * 100.0) as u32
    } else {
        0
    };

    // CPU 使用率快速采样
    let cpu_usage = quick_cpu_usage();

    Json(json!({
        "hostname": hostname,
        "kernel": kernel,
        "uptime_hours": format!("{:.1}", uptime_secs / 3600.0),
        "loadavg": loadavg,
        "cpu_usage_pct": cpu_usage,
        "memory": {
            "total_mb": mem_total / 1024,
            "available_mb": mem_available / 1024,
            "usage_pct": mem_usage
        }
    }))
}

/// JSON 格式报告导出
pub async fn report_json() -> Result<Json<Value>, StatusCode> {
    let detectors: Vec<Box<dyn Detector>> = vec![
        Box::new(SystemDetector::new()),
        Box::new(HardwareDetector::new()),
        Box::new(SoftwareDetector::new()),
        Box::new(SecurityDetector::new()),
        Box::new(PerformanceDetector::new()),
    ];

    let reports = run_detectors_reports(&detectors);
    let (total_info, total_warning, total_critical) =
        reports.iter().fold((0, 0, 0), |(i, w, c), r| {
            let (ri, rw, rc) = r.summary();
            (i + ri, w + rw, c + rc)
        });

    let modules: Vec<Value> = reports.iter().map(|r| serialize_report(r)).collect();
    Ok(Json(json!({
        "version": env!("CARGO_PKG_VERSION"),
        "generated_at": chrono_now(),
        "summary": {
            "info": total_info,
            "warning": total_warning,
            "critical": total_critical,
            "status": if total_critical > 0 { "critical" } else if total_warning > 0 { "warning" } else { "ok" }
        },
        "modules": modules
    })))
}

/// HTML 格式报告导出
pub async fn report_html() -> Html<String> {
    let detectors: Vec<Box<dyn Detector>> = vec![
        Box::new(SystemDetector::new()),
        Box::new(HardwareDetector::new()),
        Box::new(SoftwareDetector::new()),
        Box::new(SecurityDetector::new()),
        Box::new(PerformanceDetector::new()),
    ];

    let reports = run_detectors_reports(&detectors);
    let (total_info, total_warning, total_critical) =
        reports.iter().fold((0, 0, 0), |(i, w, c), r| {
            let (ri, rw, rc) = r.summary();
            (i + ri, w + rw, c + rc)
        });

    let status = if total_critical > 0 {
        "🔴 存在严重问题"
    } else if total_warning > 0 {
        "⚠️ 有警告项"
    } else {
        "✅ 系统正常"
    };

    let mut html = format!(
        r#"<!DOCTYPE html>
<html lang="zh-CN">
<head>
<meta charset="UTF-8">
<title>kylin-doctor 诊断报告</title>
<style>
body{{font-family:-apple-system,sans-serif;max-width:900px;margin:0 auto;padding:20px;background:#f5f5f5;color:#333}}
h1{{color:#0f172a;border-bottom:2px solid #06b6d4;padding-bottom:10px}}
.summary{{background:#fff;border-radius:8px;padding:16px;margin:16px 0;box-shadow:0 1px 3px rgba(0,0,0,.1)}}
.module{{background:#fff;border-radius:8px;padding:16px;margin:12px 0;box-shadow:0 1px 3px rgba(0,0,0,.1)}}
.module h2{{margin-top:0;color:#1e293b}}
.finding{{border-left:3px solid #94a3b8;padding:8px 12px;margin:8px 0;background:#f8fafc}}
.finding.critical{{border-color:#ef4444;background:#fef2f2}}
.finding.warning{{border-color:#eab308;background:#fefce8}}
.finding.info{{border-color:#3b82f6;background:#eff6ff}}
.badge{{display:inline-block;padding:2px 8px;border-radius:4px;font-size:12px;font-weight:bold;color:#fff}}
.badge.critical{{background:#ef4444}}.badge.warning{{background:#eab308;color:#000}}.badge.info{{background:#3b82f6}}
.fix{{color:#06b6d4;font-size:13px;margin-top:4px}}
footer{{text-align:center;color:#94a3b8;font-size:12px;margin-top:40px}}
</style>
</head>
<body>
<h1>🔍 kylin-doctor 诊断报告</h1>
<div class="summary">
<p><strong>状态:</strong> {}</p>
<p><strong>严重:</strong> {} &nbsp; <strong>警告:</strong> {} &nbsp; <strong>信息:</strong> {}</p>
<p><strong>生成时间:</strong> {}</p>
</div>
"#,
        status, total_critical, total_warning, total_info, chrono_now()
    );

    for report in &reports {
        let (info, warning, critical) = report.summary();
        html.push_str(&format!(
            r#"<div class="module">
<h2>📋 {} <small>({}ms, 严重:{} 警告:{} 信息:{})</small></h2>
"#,
            report.module, report.duration_ms, critical, warning, info
        ));

        for f in &report.findings {
            let severity = format!("{:?}", f.severity).to_lowercase();
            let badge = match f.severity {
                kylin_doctor_core::Severity::Critical => "严重",
                kylin_doctor_core::Severity::Warning => "警告",
                kylin_doctor_core::Severity::Info => "信息",
            };
            html.push_str(&format!(
                r#"<div class="finding {}">
<span class="badge {}">{}</span> <strong>{}</strong>
<p>{}</p>
"#,
                severity, severity, badge, f.title, f.description
            ));
            if let Some(ref fix) = f.fix {
                html.push_str(&format!(
                    r#"<div class="fix">💡 {}: <code>{}</code></div>"#,
                    fix.description, fix.command
                ));
            }
            html.push_str("</div>");
        }

        html.push_str("</div>");
    }

    html.push_str(
        r#"<footer>kylin-doctor 诊断报告 — 银河麒麟桌面系统自我诊断工具</footer>
</body></html>"#,
    );

    Html(html)
}

// ==================== WebSocket ====================

/// WebSocket 扫描处理（实时推送进度）
pub async fn ws_scan_handler(ws: WebSocketUpgrade) -> axum::response::Response {
    ws.on_upgrade(handle_ws_scan)
}

async fn handle_ws_scan(mut socket: WebSocket) {
    let modules = vec!["system", "hardware", "software", "security", "performance"];
    let total = modules.len();

    // 发送开始消息
    let _ = socket
        .send(WsMessage::Text(
            json!({"type":"start","total":total}).to_string().into(),
        ))
        .await;

    for (i, module) in modules.iter().enumerate() {
        // 发送进度
        let _ = socket
            .send(WsMessage::Text(
                json!({"type":"progress","module":module,"current":i+1,"total":total})
                    .to_string()
                    .into(),
            ))
            .await;

        // 执行扫描
        let detector: Option<Box<dyn Detector + Send>> = match *module {
            "system" => Some(Box::new(SystemDetector::new())),
            "hardware" => Some(Box::new(HardwareDetector::new())),
            "software" => Some(Box::new(SoftwareDetector::new())),
            "security" => Some(Box::new(SecurityDetector::new())),
            "performance" => Some(Box::new(PerformanceDetector::new())),
            _ => None,
        };

        if let Some(d) = detector {
            let report = run_single(&*d);
            let _ = socket
                .send(WsMessage::Text(
                    json!({"type":"result","module":module,"data":report})
                        .to_string()
                        .into(),
                ))
                .await;
        }
    }

    // 发送完成消息
    let _ = socket
        .send(WsMessage::Text(
            json!({"type":"done"}).to_string().into(),
        ))
        .await;
}

/// WebSocket AI 对话处理
pub async fn ws_chat_handler(ws: WebSocketUpgrade) -> axum::response::Response {
    ws.on_upgrade(handle_ws_chat)
}

async fn handle_ws_chat(socket: WebSocket) {
    let (mut sender, mut receiver) = socket.split();

    let config = Config::load();
    let provider = OllamaProvider::new(&config.llm.local.endpoint, &config.llm.local.model);

    // 检查可用性
    if !provider.is_available().await {
        let _ = sender
            .send(WsMessage::Text(
                json!({"type":"error","message":"LLM 服务不可用，请启动 Ollama"})
                    .to_string()
                    .into(),
            ))
            .await;
        return;
    }

    let system_prompt = "你是银河麒麟桌面系统的 AI 诊断助手。简洁回答用户问题，优先给出可执行的命令。";
    let mut messages: Vec<LlmMessage> = vec![LlmMessage::system(system_prompt)];

    let _ = sender
        .send(WsMessage::Text(
            json!({"type":"ready","model":provider.name()}).to_string().into(),
        ))
        .await;

    while let Some(Ok(msg)) = receiver.next().await {
        match msg {
            WsMessage::Text(text) => {
                let user_input = text.to_string();

                // 快捷命令
                if user_input == "/scan" || user_input == "/扫描" {
                    match tools::execute_tool("scan_all") {
                        Ok(result) => {
                            let _ = sender
                                .send(WsMessage::Text(
                                    json!({"type":"tool_result","content":result})
                                        .to_string()
                                        .into(),
                                ))
                                .await;
                        }
                        Err(e) => {
                            let _ = sender
                                .send(WsMessage::Text(
                                    json!({"type":"error","message":e.to_string()})
                                        .to_string()
                                        .into(),
                                ))
                                .await;
                        }
                    }
                    continue;
                }

                messages.push(LlmMessage::user(&user_input));

                // 带工具的对话
                let tools_def = tools::get_tool_definitions();
                match provider.chat_with_tools(&messages, &tools_def).await {
                    Ok(response) => {
                        if let Some(ref tool_calls) = response.tool_calls {
                            messages.push(response.clone());
                            for tc in tool_calls {
                                let _ = sender
                                    .send(WsMessage::Text(
                                        json!({"type":"tool_call","name":tc.function.name})
                                            .to_string()
                                            .into(),
                                    ))
                                    .await;

                                match tools::execute_tool(&tc.function.name) {
                                    Ok(result) => {
                                        messages
                                            .push(LlmMessage::tool_result(&tc.id, &result));
                                    }
                                    Err(e) => {
                                        messages.push(LlmMessage::tool_result(
                                            &tc.id,
                                            &format!("工具执行失败: {}", e),
                                        ));
                                    }
                                }
                            }
                            // 让 LLM 流式生成最终回答
                            let (result, returned_sender) = stream_to_socket(&provider, &messages, sender).await;
                            sender = returned_sender;
                            match result {
                                Ok(final_response) => {
                                    messages.push(LlmMessage::assistant(&final_response));
                                }
                                Err(e) => {
                                    let _ = sender
                                        .send(WsMessage::Text(
                                            json!({"type":"error","message":e.to_string()})
                                                .to_string()
                                                .into(),
                                        ))
                                        .await;
                                }
                            }
                        } else {
                            // 无工具调用，用 chat_stream 流式输出
                            let (result, returned_sender) = stream_to_socket(&provider, &messages, sender).await;
                            sender = returned_sender;
                            match result {
                                Ok(text) => {
                                    messages.push(LlmMessage::assistant(&text));
                                }
                                Err(e) => {
                                    // 流式失败，回退到非流式
                                    eprintln!("流式输出失败，回退到批量输出: {}", e);
                                    messages.push(LlmMessage::assistant(&response.content));
                                    let _ = sender
                                        .send(WsMessage::Text(
                                            json!({"type":"message","role":"assistant","content":response.content})
                                                .to_string()
                                                .into(),
                                        ))
                                        .await;
                                }
                            }
                        }
                    }
                    Err(e) => {
                        let _ = sender
                            .send(WsMessage::Text(
                                json!({"type":"error","message":e.to_string()})
                                    .to_string()
                                    .into(),
                            ))
                            .await;
                    }
                }
            }
            WsMessage::Close(_) => break,
            _ => {}
        }
    }
}

// ==================== 辅助函数 ====================

/// 流式发送 LLM 回复到 WebSocket
/// 使用 std::sync::mpsc 桥接 chat_stream 的同步回调和异步 WebSocket 发送
/// sender 通过 channel 转移给 drain_task，函数结束后归还
async fn stream_to_socket(
    provider: &OllamaProvider,
    messages: &[LlmMessage],
    sender: futures_util::stream::SplitSink<WebSocket, WsMessage>,
) -> (anyhow::Result<String>, futures_util::stream::SplitSink<WebSocket, WsMessage>) {
    // 使用 tokio mpsc 传递所有需要发送的 WebSocket 消息
    let (tx, mut rx) = tokio::sync::mpsc::channel::<String>(32);

    // drain_task 拥有 sender，负责所有 WebSocket 发送
    let mut sender = sender;
    let drain_task = tokio::spawn(async move {
        // 发送 stream_start
        let _ = sender
            .send(WsMessage::Text(
                json!({"type":"stream_start"}).to_string().into(),
            ))
            .await;

        // 持续发送 stream_chunk，直到 channel 关闭
        while let Some(chunk) = rx.recv().await {
            let msg = json!({"type":"stream_chunk","content":chunk}).to_string();
            if sender
                .send(WsMessage::Text(msg.into()))
                .await
                .is_err()
            {
                return sender;
            }
        }

        // 发送 stream_end
        let _ = sender
            .send(WsMessage::Text(
                json!({"type":"stream_end"}).to_string().into(),
            ))
            .await;

        sender
    });

    // chat_stream 的同步回调通过 std::sync::mpsc 发送给 tokio task
    let (sync_tx, sync_rx) = std::sync::mpsc::channel::<String>();
    // 桥接：std::sync::mpsc -> tokio::sync::mpsc
    let bridge_tx = tx.clone();
    let bridge_task = tokio::spawn(async move {
        while let Ok(chunk) = sync_rx.recv() {
            if bridge_tx.send(chunk).await.is_err() {
                break;
            }
        }
    });

    let full = provider
        .chat_stream(messages, Box::new(move |chunk| {
            let _ = sync_tx.send(chunk);
        }))
        .await;

    // 清理：关闭 bridge，等待 drain 完成
    drop(tx); // 关闭 tokio channel
    let _ = bridge_task.await;
    let sender = drain_task.await.unwrap();

    (full, sender)
}

fn run_detectors(detectors: &[Box<dyn Detector>]) -> Vec<Value> {
    detectors.iter().map(|d| run_single(&**d)).collect()
}

fn run_detectors_reports(detectors: &[Box<dyn Detector>]) -> Vec<ScanReport> {
    detectors.iter().filter_map(|d| d.scan().ok()).collect()
}

fn run_single(detector: &dyn Detector) -> Value {
    match detector.scan() {
        Ok(report) => serialize_report(&report),
        Err(e) => json!({
            "module": detector.name(),
            "error": e.to_string(),
            "findings": [],
            "duration_ms": 0,
            "summary": {"info": 0, "warning": 0, "critical": 0}
        }),
    }
}

fn serialize_report(report: &ScanReport) -> Value {
    let (info, warning, critical) = report.summary();
    json!({
        "module": report.module,
        "findings": report.findings.iter().map(|f| {
            json!({
                "id": f.id,
                "module": f.module,
                "severity": format!("{:?}", f.severity).to_lowercase(),
                "title": f.title,
                "description": f.description,
                "evidence": f.evidence,
                "fix": f.fix.as_ref().map(|fix| json!({
                    "description": fix.description,
                    "command": fix.command,
                    "risk_level": fix.risk_level
                })),
                "auto_fixable": f.auto_fixable
            })
        }).collect::<Vec<_>>(),
        "duration_ms": report.duration_ms,
        "summary": {
            "info": info,
            "warning": warning,
            "critical": critical
        }
    })
}

fn quick_cpu_usage() -> f64 {
    let stat = std::fs::read_to_string("/proc/stat").unwrap_or_default();
    for line in stat.lines() {
        if line.starts_with("cpu ") {
            let parts: Vec<u64> = line
                .split_whitespace()
                .skip(1)
                .filter_map(|v| v.parse().ok())
                .collect();
            if parts.len() >= 5 {
                let idle = parts[3] + parts[4];
                let total: u64 = parts.iter().sum();
                if total > 0 {
                    return ((total - idle) as f64 / total as f64 * 100.0).round();
                }
            }
        }
    }
    0.0
}

fn chrono_now() -> String {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| format!("{}", d.as_secs()))
        .unwrap_or_else(|_| "unknown".to_string())
}
