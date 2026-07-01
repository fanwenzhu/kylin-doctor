use axum::extract::ws::{Message as WsMessage, WebSocket, WebSocketUpgrade};
use axum::extract::{Path, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{Html, Json};
use futures_util::{SinkExt, StreamExt};
use kylin_doctor_core::{
    epoch_secs, html_escape, llm::tools, AnthropicProvider, Config, Detector, Finding,
    HardwareDetector, LlmProvider, Message as LlmMessage, OllamaProvider, OpenAiCompatProvider,
    PerformanceDetector, ScanReport, SecurityDetector, Severity, SoftwareDetector, SystemDetector,
};
use serde_json::{json, Value};
use std::sync::Arc;

use crate::AppState;

// ==================== LLM Provider 工厂 ====================

/// 创建云端 LLM 提供商
fn create_cloud_provider(config: &Config) -> Result<Box<dyn LlmProvider>, String> {
    let cloud = &config.llm.cloud;

    // 解析 API Key
    let api_key = cloud.resolve_api_key().map_err(|e| {
        format!("云端模型 API Key 配置错误: {}", e)
    })?;

    // 验证 endpoint
    if cloud.endpoint.is_empty() {
        return Err("云端模型 endpoint 未配置，请在 ~/.kylin-doctor/config.toml 中设置 [llm.cloud] endpoint".to_string());
    }

    match cloud.provider.as_str() {
        "anthropic" => {
            let p = AnthropicProvider::new(&cloud.endpoint, &cloud.model, &api_key);
            Ok(Box::new(p))
        }
        "openai" | "qwen" | "deepseek" | "moonshot" | "custom" => {
            let p = OpenAiCompatProvider::new(&cloud.endpoint, &cloud.model, &api_key);
            Ok(Box::new(p))
        }
        other => {
            Err(format!("未知的云端 provider: '{}'，支持: openai/qwen/deepseek/moonshot/anthropic/custom", other))
        }
    }
}

/// 创建 LLM 提供商（hybrid 模式：优先本地，回退云端）
async fn create_provider(config: &Config) -> Result<Box<dyn LlmProvider>, String> {
    let strategy = config.llm.strategy.as_str();

    match strategy {
        "cloud" => {
            // 纯云端模式：云端不可用则报错
            create_cloud_provider(config)
        }
        "local" => {
            // 纯本地模式
            let local = OllamaProvider::new(&config.llm.local.endpoint, &config.llm.local.model);
            if local.is_available().await {
                Ok(Box::new(local))
            } else {
                Err(format!("本地 Ollama 服务不可用 (endpoint: {})", config.llm.local.endpoint))
            }
        }
        "hybrid" | _ => {
            // hybrid 模式：优先本地，回退云端
            let local = OllamaProvider::new(&config.llm.local.endpoint, &config.llm.local.model);
            if local.is_available().await {
                return Ok(Box::new(local));
            }

            // 本地不可用，尝试云端
            match create_cloud_provider(config) {
                Ok(provider) => Ok(provider),
                Err(cloud_err) => {
                    // 云端也不可用，给出完整诊断
                    Err(format!(
                        "LLM 服务不可用：\n  - 本地 Ollama: 不可达 ({})\n  - 云端模型: {}\n\n\
                         请检查：\n  1. Ollama 是否已启动: systemctl status ollama\n  2. 云端 API Key 是否配置: ~/.kylin-doctor/config.toml",
                        config.llm.local.endpoint, cloud_err
                    ))
                }
            }
        }
    }
}

// ==================== WebSocket 安全 ====================

/// 检查 WebSocket Origin header，防止 Cross-Site WebSocket Hijacking
fn check_origin(headers: &HeaderMap) -> bool {
    // 如果没有 Origin header（非浏览器客户端），允许连接
    let origin = match headers.get("origin") {
        Some(v) => v.to_str().unwrap_or(""),
        None => return true,
    };

    // 允许的 Origin 列表
    let allowed_origins = [
        "http://127.0.0.1",
        "http://localhost",
        "https://127.0.0.1",
        "https://localhost",
    ];

    // 检查 Origin 是否在白名单中
    for allowed in &allowed_origins {
        if origin.starts_with(allowed) {
            return true;
        }
    }

    // 开发模式：允许所有 Origin（生产环境应移除）
    if cfg!(debug_assertions) {
        return true;
    }

    false
}

// ==================== Detector 工厂 ====================

/// 创建所有检测模块
fn all_detectors() -> Vec<Box<dyn Detector + Send>> {
    vec![
        Box::new(SystemDetector::new()),
        Box::new(HardwareDetector::new()),
        Box::new(SoftwareDetector::new()),
        Box::new(SecurityDetector::new()),
        Box::new(PerformanceDetector::new()),
    ]
}

/// 按名称创建单个检测模块
fn detector_by_name(name: &str) -> Option<Box<dyn Detector + Send>> {
    match name {
        "system" => Some(Box::new(SystemDetector::new())),
        "hardware" => Some(Box::new(HardwareDetector::new())),
        "software" => Some(Box::new(SoftwareDetector::new())),
        "security" => Some(Box::new(SecurityDetector::new())),
        "performance" => Some(Box::new(PerformanceDetector::new())),
        _ => None,
    }
}

// ==================== REST API ====================

/// 全量扫描所有模块
pub async fn scan_all() -> Result<Json<Value>, StatusCode> {
    let reports = run_detectors(&all_detectors());
    Ok(Json(json!(reports)))
}

/// 扫描指定模块
pub async fn scan_module(Path(module): Path<String>) -> Result<Json<Value>, StatusCode> {
    let detector = detector_by_name(&module);

    match detector {
        Some(d) => {
            let report = run_single(&*d);
            Ok(Json(json!(report)))
        }
        None => Err(StatusCode::NOT_FOUND),
    }
}

/// 系统概览（快速信息，不执行完整扫描）
pub async fn status_with_state(State(state): State<Arc<AppState>>) -> Json<Value> {
    let cpu_usage = state.cpu.lock().map(|s| s.usage_pct).unwrap_or(0.0);
    status_json(cpu_usage)
}

fn status_json(cpu_usage: f64) -> Json<Value> {
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
    let detectors = all_detectors();

    let reports = run_detectors_reports(&detectors);
    let (total_info, total_warning, total_critical) =
        reports.iter().fold((0, 0, 0), |(i, w, c), r| {
            let (ri, rw, rc) = r.summary();
            (i + ri, w + rw, c + rc)
        });

    let modules: Vec<Value> = reports.iter().map(|r| serialize_report(r)).collect();
    Ok(Json(json!({
        "version": env!("CARGO_PKG_VERSION"),
        "generated_at": epoch_secs(),
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
pub async fn report_html() -> (axum::http::HeaderMap, Html<String>) {
    let detectors = all_detectors();

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
        status, total_critical, total_warning, total_info, epoch_secs()
    );

    for report in &reports {
        let (info, warning, critical) = report.summary();
        html.push_str(&format!(
            r#"<div class="module">
<h2>📋 {} <small>({}ms, 严重:{} 警告:{} 信息:{})</small></h2>
"#,
            html_escape(&report.module), report.duration_ms, critical, warning, info
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
                severity, severity, badge, html_escape(&f.title), html_escape(&f.description)
            ));
            if let Some(ref fix) = f.fix {
                html.push_str(&format!(
                    r#"<div class="fix">💡 {}: <code>{}</code></div>"#,
                    html_escape(&fix.description), html_escape(&fix.command)
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

    let mut headers = axum::http::HeaderMap::new();
    headers.insert("X-Content-Type-Options", "nosniff".parse().unwrap());
    headers.insert("X-Frame-Options", "DENY".parse().unwrap());
    headers.insert("Content-Security-Policy", "default-src 'self' 'unsafe-inline'; script-src 'self' 'unsafe-inline' https://cdn.jsdelivr.net".parse().unwrap());
    (headers, Html(html))
}

// ==================== WebSocket ====================

/// WebSocket 扫描处理（实时推送进度）
pub async fn ws_scan_handler(
    headers: HeaderMap,
    ws: WebSocketUpgrade,
) -> axum::response::Response {
    // Origin 校验：防止 Cross-Site WebSocket Hijacking
    if !check_origin(&headers) {
        return axum::response::Response::builder()
            .status(StatusCode::FORBIDDEN)
            .body("Origin not allowed".into())
            .unwrap();
    }
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
        if let Some(d) = detector_by_name(module) {
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
pub async fn ws_chat_handler(
    headers: HeaderMap,
    ws: WebSocketUpgrade,
) -> axum::response::Response {
    // Origin 校验：防止 Cross-Site WebSocket Hijacking
    if !check_origin(&headers) {
        return axum::response::Response::builder()
            .status(StatusCode::FORBIDDEN)
            .body("Origin not allowed".into())
            .unwrap();
    }
    ws.on_upgrade(handle_ws_chat)
}

async fn handle_ws_chat(socket: WebSocket) {
    let (mut sender, mut receiver) = socket.split();

    let config = Config::load();
    let provider = match create_provider(&config).await {
        Ok(p) => p,
        Err(e) => {
            let _ = sender
                .send(WsMessage::Text(
                    json!({"type":"error","message":e}).to_string().into(),
                ))
                .await;
            return;
        }
    };

    let system_prompt = "你是银河麒麟桌面系统的 AI 诊断助手。简洁回答用户问题，优先给出可执行的命令。";
    let mut messages: Vec<LlmMessage> = vec![LlmMessage::system(system_prompt)];

    /// 聊天历史最大消息数（防止 OOM 和超出 LLM 上下文窗口）
    const MAX_CHAT_MESSAGES: usize = 50;

    /// 裁剪聊天历史，保留 system prompt + 最近的消息
    /// 确保裁剪后首条非 system 消息为 user 角色（LLM API 要求 user/assistant 交替）
    fn trim_messages(messages: &mut Vec<LlmMessage>) {
        if messages.len() > MAX_CHAT_MESSAGES {
            let system = messages.remove(0);
            let drain_count = messages.len() - (MAX_CHAT_MESSAGES - 1);
            messages.drain(0..drain_count);

            // 确保首条非 system 消息为 user 角色
            // 如果首条是 assistant 或 tool，删除它以保持 user/assistant 交替
            while let Some(first) = messages.first() {
                if first.role == "user" || first.role == "system" {
                    break;
                }
                messages.remove(0);
            }

            messages.insert(0, system);
        }
    }

    let _ = sender
        .send(WsMessage::Text(
            json!({"type":"ready","model":provider.name()}).to_string().into(),
        ))
        .await;

    // 速率限制状态
    let mut message_timestamps: Vec<std::time::Instant> = Vec::new();
    const RATE_LIMIT_WINDOW: std::time::Duration = std::time::Duration::from_secs(10);
    const MAX_MESSAGES_PER_WINDOW: usize = 10;

    while let Some(Ok(msg)) = receiver.next().await {
        match msg {
            WsMessage::Text(text) => {
                let user_input = text.to_string();

                // 消息大小限制（防止内存耗尽）
                const MAX_MESSAGE_BYTES: usize = 64 * 1024; // 64KB
                if user_input.len() > MAX_MESSAGE_BYTES {
                    let _ = sender
                        .send(WsMessage::Text(
                            json!({"type":"error","message":"消息过长，请缩短输入"}).to_string().into(),
                        ))
                        .await;
                    continue;
                }

                // 速率限制（防止消息洪水攻击）
                let now = std::time::Instant::now();
                message_timestamps.retain(|t| now.duration_since(*t) < RATE_LIMIT_WINDOW);
                if message_timestamps.len() >= MAX_MESSAGES_PER_WINDOW {
                    let _ = sender
                        .send(WsMessage::Text(
                            json!({"type":"error","message":"消息发送过于频繁，请稍后再试"}).to_string().into(),
                        ))
                        .await;
                    continue;
                }
                message_timestamps.push(now);

                // 快捷命令（使用 spawn_blocking 避免阻塞 async 上下文）
                if user_input == "/scan" || user_input == "/扫描" {
                    let scan_result = tokio::task::spawn_blocking(|| {
                        tools::execute_tool("scan_all")
                    }).await;

                    match scan_result {
                        Ok(Ok(result)) => {
                            let _ = sender
                                .send(WsMessage::Text(
                                    json!({"type":"tool_result","content":result})
                                        .to_string()
                                        .into(),
                                ))
                                .await;
                        }
                        Ok(Err(e)) => {
                            let _ = sender
                                .send(WsMessage::Text(
                                    json!({"type":"error","message":e.to_string()})
                                        .to_string()
                                        .into(),
                                ))
                                .await;
                        }
                        Err(e) => {
                            let _ = sender
                                .send(WsMessage::Text(
                                    json!({"type":"error","message":format!("扫描任务执行失败: {}", e)})
                                        .to_string()
                                        .into(),
                                ))
                                .await;
                        }
                    }
                    continue;
                }

                messages.push(LlmMessage::user(&user_input));
                trim_messages(&mut messages);

                // 发送 thinking 状态，让用户知道正在处理
                let _ = sender
                    .send(WsMessage::Text(
                        json!({"type":"thinking","message":"正在思考..."}).to_string().into(),
                    ))
                    .await;

                // 带工具的对话（使用 select! 监听 WebSocket close）
                let tools_def = tools::get_tool_definitions();
                let chat_future = provider.chat_with_tools(&messages, &tools_def);
                let ws_close_future = receiver.next();

                match tokio::select! {
                    result = chat_future => Some(result),
                    _ = ws_close_future => None,
                } {
                    Some(Ok(response)) => {
                        // 正常响应
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

                                // 使用 spawn_blocking 执行同步工具调用
                                let tool_name = tc.function.name.clone();
                                let tool_result = if tools::is_valid_tool(&tool_name) {
                                    tokio::task::spawn_blocking(move || {
                                        tools::execute_tool(&tool_name)
                                    }).await
                                } else {
                                    Ok(Ok("未知工具，已拒绝执行".to_string()))
                                };

                                match tool_result {
                                    Ok(Ok(result)) => {
                                        messages
                                            .push(LlmMessage::tool_result(&tc.id, &result));
                                    }
                                    Ok(Err(e)) => {
                                        messages.push(LlmMessage::tool_result(
                                            &tc.id,
                                            &format!("工具执行失败: {}", e),
                                        ));
                                    }
                                    Err(e) => {
                                        messages.push(LlmMessage::tool_result(
                                            &tc.id,
                                            &format!("工具任务执行失败: {}", e),
                                        ));
                                    }
                                }
                            }
                            // 让 LLM 流式生成最终回答
                            let (result, returned_sender) = stream_to_socket(&*provider, &messages, sender).await;
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
                            let (result, returned_sender) = stream_to_socket(&*provider, &messages, sender).await;
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
                    Some(Err(e)) => {
                        // API 调用失败
                        let _ = sender
                            .send(WsMessage::Text(
                                json!({"type":"error","message":e.to_string()})
                                    .to_string()
                                    .into(),
                            ))
                            .await;
                    }
                    None => {
                        // WebSocket 连接已关闭
                        return;
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
    provider: &dyn LlmProvider,
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
    let sender = drain_task.await.expect("WebSocket drain task panicked");

    (full, sender)
}

fn run_detectors(detectors: &[Box<dyn Detector + Send>]) -> Vec<Value> {
    detectors.iter().map(|d| run_single(&**d)).collect()
}

fn run_detectors_reports(detectors: &[Box<dyn Detector + Send>]) -> Vec<ScanReport> {
    detectors
        .iter()
        .map(|d| {
            d.scan().unwrap_or_else(|e| {
                // 扫描失败时创建一个包含错误信息的报告
                let mut report = ScanReport::new(d.name().to_string());
                report.findings.push(Finding {
                    id: "scan_error".to_string(),
                    module: d.name().to_string(),
                    title: "扫描失败".to_string(),
                    description: e.to_string(),
                    severity: Severity::Critical,
                    fix: None,
                    auto_fixable: false,
                    evidence: String::new(),
                });
                report
            })
        })
        .collect()
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
                    "risk_level": fix.risk_level,
                    "program": fix.program,
                    "args": fix.args
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

#[cfg(test)]
mod tests {
    use super::*;
    use kylin_doctor_core::{Finding, FixAction, ScanReport, Severity};

    #[test]
    fn serialize_report_empty() {
        let report = ScanReport::new("test".to_string());
        let val = serialize_report(&report);
        assert_eq!(val["module"], "test");
        assert_eq!(val["duration_ms"], 0);
        assert_eq!(val["summary"]["info"], 0);
        assert_eq!(val["summary"]["warning"], 0);
        assert_eq!(val["summary"]["critical"], 0);
        assert!(val["findings"].as_array().unwrap().is_empty());
    }

    #[test]
    fn serialize_report_with_findings() {
        let mut report = ScanReport::new("hardware".to_string());
        report.findings.push(Finding {
            id: "hw-test".to_string(),
            module: "hardware".to_string(),
            severity: Severity::Warning,
            title: "Test warning".to_string(),
            description: "A test warning".to_string(),
            evidence: "evidence data".to_string(),
            fix: Some(FixAction {
                description: "Fix it".to_string(),
                command: "echo fix".to_string(),
                risk_level: "low".to_string(),
                ..Default::default()
            }),
            auto_fixable: true,
        });
        report.duration_ms = 42;

        let val = serialize_report(&report);
        assert_eq!(val["module"], "hardware");
        assert_eq!(val["duration_ms"], 42);
        assert_eq!(val["summary"]["warning"], 1);
        let findings = val["findings"].as_array().unwrap();
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0]["severity"], "warning");
        assert_eq!(findings[0]["auto_fixable"], true);
        assert!(findings[0]["fix"].is_object());
    }

    #[test]
    fn status_json_has_required_fields() {
        let val = status_json(42.5);
        assert!(val["hostname"].is_string());
        assert!(val["kernel"].is_string());
        assert!(val["uptime_hours"].is_string());
        assert!(val["loadavg"].is_string());
        assert_eq!(val["cpu_usage_pct"], 42.5);
        assert!(val["memory"]["total_mb"].is_number());
        assert!(val["memory"]["available_mb"].is_number());
        assert!(val["memory"]["usage_pct"].is_number());
    }

    #[test]
    fn status_json_cpu_zero() {
        let val = status_json(0.0);
        assert_eq!(val["cpu_usage_pct"], 0.0);
    }

    #[tokio::test]
    async fn scan_all_returns_json_array() {
        let result = scan_all().await;
        assert!(result.is_ok());
        let val = result.unwrap().0;
        let arr = val.as_array().unwrap();
        // 应该有 5 个模块
        assert_eq!(arr.len(), 5);
        // 每个结果应该有 module 字段
        for item in arr {
            assert!(item["module"].is_string());
            assert!(item["findings"].is_array());
            assert!(item["summary"].is_object());
        }
    }

    #[tokio::test]
    async fn scan_module_not_found() {
        let result = scan_module(Path("nonexistent".to_string())).await;
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn scan_module_valid() {
        for module in &["system", "hardware", "software", "security", "performance"] {
            let result = scan_module(Path(module.to_string())).await;
            assert!(result.is_ok(), "Module {} should return OK", module);
            let val = result.unwrap().0;
            assert_eq!(val["module"], *module);
        }
    }

    #[tokio::test]
    async fn report_json_has_structure() {
        let result = report_json().await;
        assert!(result.is_ok());
        let val = result.unwrap().0;
        assert!(val["version"].is_string());
        assert!(val["generated_at"].is_string());
        assert!(val["summary"].is_object());
        assert!(val["modules"].is_array());
    }

    #[tokio::test]
    async fn report_html_contains_title() {
        let (_headers, html) = report_html().await;
        assert!(html.0.contains("kylin-doctor"));
        assert!(html.0.contains("诊断报告"));
    }

    #[test]
    fn html_escape_basic() {
        assert_eq!(html_escape("<script>alert('xss')</script>"), "&lt;script&gt;alert(&#x27;xss&#x27;)&lt;/script&gt;");
        assert_eq!(html_escape("a & b"), "a &amp; b");
        assert_eq!(html_escape("no special chars"), "no special chars");
        assert_eq!(html_escape(""), "");
        assert_eq!(html_escape("\"quoted\""), "&quot;quoted&quot;");
    }
}

