# kylin-doctor 开发规范

> 银河麒麟桌面系统自我诊断工具 — AI 协作开发指令

## 项目概览

Rust workspace 三 crate 架构：
- `kylin-doctor-core` — 核心检测逻辑 + LLM 集成
- `kylin-doctor-cli` — CLI 命令行工具（二进制名 `kylin-doctor`）
- `kylin-doctor-web` — Web 仪表盘（二进制名 `kylin-doctor-web`）

## 构建与测试

```bash
cargo build --release          # 发布构建
cargo test                     # 运行全部测试（当前 52 个）
cargo test -p kylin-doctor-core # 只运行核心库测试
cargo check                    # 检查编译警告
```

## 版本管理（强制）

- 版本号在 `Cargo.toml` 的 `[workspace.package]` 中统一管理
- 每次功能变更必须同步更新 `CHANGELOG.md`（格式：Keep a Changelog）
- 发布版本必须打 git tag：`git tag -a vX.Y.Z -m "描述"`
- 详见 `CHANGELOG.md` 和 `docs/DEPLOYMENT.md`

## 代码规范

- 缩进 4 空格，禁止 Tab
- 命名：snake_case（函数/变量）、PascalCase（类型）、UPPER_SNAKE_CASE（常量）
- 错误处理：使用 `anyhow::Result`，不忽略错误
- 异步：tokio 运行时，`async-trait` 定义异步 trait

## LLM 集成架构

`LlmProvider` trait 三个方法分工：
| 方法 | 流式 | 工具调用 | 用途 |
|------|------|----------|------|
| `chat()` | ❌ | ❌ | 非流式简单对话 |
| `chat_with_tools()` | ❌ | ✅ | 检测工具调用 |
| `chat_stream()` | ✅ | ❌ | 流式输出最终回答 |

**限制**：`chat_stream` 不支持 tool calls，需要两次 LLM 调用。

## 关键文件速查

| 文件 | 职责 |
|------|------|
| `crates/kylin-doctor-core/src/detector.rs` | Finding/FixAction 数据结构 |
| `crates/kylin-doctor-core/src/llm/provider.rs` | LlmProvider trait 定义 |
| `crates/kylin-doctor-core/src/llm/tools.rs` | Function Calling 工具定义 |
| `crates/kylin-doctor-cli/src/commands/chat.rs` | CLI 对话（流式 + 工具调用） |
| `crates/kylin-doctor-web/src/api.rs` | Web WebSocket + 流式后端 |
| `crates/kylin-doctor-web/src/dashboard.html` | 前端单文件（CSS+JS 内嵌） |
| `install.sh` | 一键安装脚本 |

## 深入文档

- **[docs/DEPLOYMENT.md](docs/DEPLOYMENT.md)** — 部署与运维：环境要求、构建安装、AI 模型配置、systemd 服务、Docker、故障排查
- **[docs/USAGE.md](docs/USAGE.md)** — 使用说明：CLI 命令详解、Web 仪表盘操作、诊断模块阈值、常见场景
- **[CHANGELOG.md](CHANGELOG.md)** — 版本变更记录
- **[docs/superpowers/specs/2026-05-29-kylin-doctor-design.md](docs/superpowers/specs/2026-05-29-kylin-doctor-design.md)** — 原始设计文档
