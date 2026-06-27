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
cargo test                     # 运行全部测试（当前 56 个）
cargo test -p kylin-doctor-core # 只运行核心库测试
cargo check                    # 检查编译警告
./build-deb.sh                 # 构建当前架构 deb 包（输出到 dist/）
./build-deb.sh --arch arm64    # 交叉编译 arm64 deb 包
```

## 版本管理（强制）

每次将本地更新推送到 GitHub 前，**必须**完成以下全部步骤：

1. **更新版本号** — `Cargo.toml` 的 `[workspace.package].version`（语义化版本）
2. **更新 CHANGELOG.md** — 按 Keep a Changelog 格式记录本次变更（新增/改进/修复）
3. **重新构建 deb 包** — **必须静态编译（musl），禁止动态链接（glibc）**，原因：工控机 glibc 版本较低，动态编译会导致 `GLIBC_x.xx not found` 运行失败。
   ```bash
   ./build-deb.sh --static                # amd64 musl 静态
   cross build --release --target aarch64-unknown-linux-musl  # arm64 musl 静态（用 cross 工具）
   ./build-deb.sh --arch arm64 --static --skip-build          # 打包 arm64 deb
   ```
   **注意**: arm64 必须用 `cross` 工具编译（`cargo install cross`），直接用 `aarch64-linux-gnu-gcc` 编译 musl 目标会失败（符号不兼容）。
4. **按需更新文档** — 如涉及功能变更或用法调整，同步更新 `USAGE.md` 和 `README.md`
5. **打 git tag** — `git tag -a vX.Y.Z -m "描述"`
6. **更新 GitHub Release** — `gh release upload vX.Y.Z dist/*.deb --clobber` 上传 deb 包，`gh release edit vX.Y.Z --notes "..." --draft=false` 更新说明并发布

版本号在 `Cargo.toml` 的 `[workspace.package]` 中统一管理。详见 `CHANGELOG.md` 和 `docs/DEPLOYMENT.md`。

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

**Provider 实现**：
| Provider | 协议 | 流式 | 工具调用 |
|----------|------|------|----------|
| `OllamaProvider` | Ollama 本地 API | ✅ | ✅ |
| `OpenAiCompatProvider` | OpenAI 兼容 `/chat/completions` | ❌（回退批量） | ❌ |
| `AnthropicProvider` | Anthropic Messages API `/v1/messages` | ✅ | ✅ |

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
| `build-deb.sh` | deb 打包脚本 |
| `pkg/deb/` | deb 配套文件（systemd、配置模板） |

## 深入文档

- **[docs/DEPLOYMENT.md](docs/DEPLOYMENT.md)** — 部署与运维：环境要求、构建安装、AI 模型配置、systemd 服务、Docker、故障排查
- **[docs/USAGE.md](docs/USAGE.md)** — 使用说明：CLI 命令详解、Web 仪表盘操作、诊断模块阈值、常见场景
- **[CHANGELOG.md](CHANGELOG.md)** — 版本变更记录
- **[docs/superpowers/specs/2026-05-29-kylin-doctor-design.md](docs/superpowers/specs/2026-05-29-kylin-doctor-design.md)** — 原始设计文档
