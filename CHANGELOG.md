# Changelog

本文件记录 kylin-doctor 的所有重要变更。

格式基于 [Keep a Changelog](https://keepachangelog.com/zh-CN/1.0.0/)，
版本号遵循 [语义化版本](https://semver.org/lang/zh-CN/)。

## [0.3.5] - 2026-06-30

### 修复 (Fixed) — 对抗性审查专项
- **`/proc/diskstats` 字段映射错误**: `hardware.rs` 的 `check_disk_io_errors` 误将字段 12（`io_time_ms`）当作 I/O 错误计数，任何有 I/O 活动的系统都会产生误报。改为正确的 I/O 饱和度检测
- **SSE 流式解析器 chunk 边界丢数据**: `openai_compat.rs` 和 `ollama.rs` 的流式解析器在 HTTP chunk 不对齐 SSE 行时丢失内容。增加行缓冲区，跨 chunk 拼接完整行后再解析
- **`[DONE]` 标记只退出内层循环**: 流式解析器收到 `[DONE]` 后继续处理剩余 chunk。改用 `done` 标志位退出双层循环
- **文档 ID 碰撞**: `KnowledgeStore` 使用 `documents.len()` 生成 ID，remove 后 add 产生重复 ID。改用单调递增计数器
- **unsigned 减法无溢出保护**: `performance.rs` 的 `check_disk_io` 使用 `saturating_sub` 防止计数器回绕 panic
- **HTML 报告 XSS**: `report_html()` 直接拼接 finding 数据到 HTML，无转义。新增 `html_escape()` 函数，对所有动态值转义
- **CPU 采样 iowait 不一致**: Web 仪表盘的 `read_cpu_stat()` 未将 iowait 计入空闲，导致 CPU 使用率偏高。与 `performance.rs` 保持一致
- **移除 CORS 完全开放**: 删除 `CorsLayer::permissive()`，浏览器同源策略天然阻止外部网站访问本地 API
- **聊天消息历史无限增长**: 新增 `MAX_CHAT_MESSAGES=50` 上限，超出时自动裁剪旧消息保留 system prompt
- **WebSocket 消息大小限制**: 新增 64KB 消息大小限制，防止超大消息耗尽内存
- **知识库路径遍历**: `remove_document()` 现在验证 `doc_id` 格式为 `doc_N`，拒绝非法路径
- **统一 meminfo 解析**: 3 处重复的 `/proc/meminfo` 解析合并为 `core::util::parse_meminfo()`，`api.rs` 和 `performance.rs` 共用
- **HTML 报告安全头**: 新增 `X-Content-Type-Options`、`X-Frame-Options`、`Content-Security-Policy` 响应头
- **API Key 泄露到错误消息**: LLM 调用失败时错误响应中可能包含 API Key 片段。新增 `sanitize_api_error()` 自动掩码 `sk-*` 和 Bearer token，截断过长响应体
- **`chrono_now()` 重命名为 `epoch_secs()`**: 函数返回 Unix 时间戳秒数，原名暗示人类可读时间，易误导
- **`std::sync::Mutex` 使用文档**: 在 `AppState` 上添加注释说明为何在 async 上下文中使用 `std::sync::Mutex`（短临界区、不跨 await）
- **`sh -c` 命令注入架构重构**: `FixAction` 新增 `program` + `args` 结构化执行字段，`run_fix()` 方法优先使用结构化执行，回退到 `sh -c`。所有 5 个 Detector 的 `fix()` 方法统一调用 `run_fix()`，消除直接 shell 调用
- **LLM 工具名白名单验证**: `execute_tool()` 新增 `is_valid_tool()` 白名单检查，WebSocket chat handler 在执行工具前验证名称，拒绝未知工具

### 测试 (Tests)
- **测试覆盖从 65 增至 93**: 新增 28 个测试
  - Web API handler 测试 (10 个): `scan_all`、`scan_module`、`status`、`report_json`、`report_html`、`serialize_report`、`html_escape`
  - CLI `print_summary` 测试 (6 个): 退出码逻辑（正常/警告/严重/混合）
  - `OpenAiCompatProvider` 测试 (6 个): 消息转换、工具序列化、流式响应解析
  - 知识库 ID 碰撞回归测试 (1 个): 验证 remove+add 后 ID 唯一性
  - `sanitize_api_error` 测试 (5 个): API Key 掩码、Bearer token 掩码、截断

## [0.3.4] - 2026-06-30

### 改进 (Changed)
- **架构重构 — 消除 CLI/Web 代码克隆**: 提取 `kylin-doctor-web/src/lib.rs` 共享层，CLI `serve` 命令复用 Web crate 的 API handler 和路由，消除 ~160 行重复代码
  - 新增 `create_router()` 统一创建 REST API + WebSocket + 前端路由
  - 新增 `spawn_cpu_sampler()` CPU 采样后台任务供 Web 二进制使用
  - CLI serve 命令从 160 行精简至 28 行
- **统一 `/proc/diskstats` 解析器**: 将 hardware/performance 两个 Detector 中的 3 份重复解析器合并为 `core::util::parse_diskstats()`，返回完整的 `DiskStats` 结构体
- **统一 `chrono_now()` 工具函数**: 消除 `api.rs` 和 `knowledge/store.rs` 中的重复定义，统一使用 `core::util::chrono_now()`
- **补全 `OpenAiCompatProvider`**: 新增 `chat_with_tools()` 和 `chat_stream()` 支持，Qwen/DeepSeek/Moonshot 等云端模型现在支持 Function Calling 和流式输出
- **修复硬编码年份**: `hardware.rs` 中 BIOS 年龄检查从硬编码 `2026` 改为动态获取系统年份
- **修复 `cli_verbose()` 脆弱实现**: `fix.rs` 中不再手动解析 `std::env::args()`，改为从 CLI 框架传入 `verbose` 参数
- **清理死代码**: 移除 `render_markdown_chunk()` 空函数（直接返回输入，无实际渲染）

### 测试 (Tests)
- 新增 CLI `print_summary` 测试 (6 个)、`OpenAiCompatProvider` 测试 (6 个)

## [0.3.3] - 2026-06-29

### 修复 (Fixed)
- **Web 仪表盘 CPU 实时不刷新**: 前端 `loadStatus()` 只在页面加载时调用一次，后端每 2 秒采样 CPU 但前端从不主动获取最新值
  - 添加 `setInterval(loadStatus, 2000)` 定时刷新，每 2 秒更新 CPU、内存、负载等指标
  - 添加 pulse 动画效果，CPU 数值变化时有视觉反馈
  - 根据 CPU 使用率动态变色：>90% 红色，>70% 黄色，否则绿色

## [0.3.2] - 2026-06-27

### 修复 (Fixed)
- **Web 仪表盘 CPU 显示 0%**: 修复 `quick_cpu_usage()` 只读一次 `/proc/stat` 导致计算的是开机至今平均值而非当前瞬时值的问题
  - 新增后台 tokio 任务每 2 秒采样 `/proc/stat`，计算 delta 得到真实 CPU 使用率
  - 引入 `AppState` 共享状态，`/api/status` 端点直接读取内存中的采样值，零延迟响应
  - 删除有 bug 的 `quick_cpu_usage()` 函数

### 改进 (Changed)
- **扫描完成后自动刷新概览卡片**: REST 扫描和 WebSocket 扫描完成后自动调用 `loadStatus()`，CPU、内存、负载等信息实时更新

## [0.3.1] - 2026-06-23

### 新增 (Added)
- **配置文件直接写入 API Key**: `config.toml` 新增 `api_key` 字段，支持直接填写 API Key 而无需设置环境变量
  - 优先级：`api_key`（配置文件）> `api_key_env`（环境变量），两者都为空时提示配置
  - 适用于所有云端供应商（Qwen/DeepSeek/Moonshot/Anthropic/自定义）

### 改进 (Changed)
- **云端 Provider 创建逻辑**: `AnthropicProvider` 和 `OpenAiCompatProvider` 改为通过 `CloudLlmConfig::resolve_api_key()` 统一解析 API Key
- **文档更新**: USAGE.md 和 config.toml.example 同步更新，示例改为推荐直接写入 `api_key` 方式

### 修复 (Fixed)
- **Web 仪表盘扫描按钮无响应**: 移除 JavaScript 中的正则 lookbehind 断言 `(?<!)`，兼容工控机等旧版浏览器（ES2018 以下），修复 `SyntaxError: invalid regexp group` 导致整个 script 块不执行的问题
- **Web 仪表盘扫描挂起**: 所有外部系统命令（`apt-get`、`dpkg`、`snap`、`flatpak`、`ss`、`find` 等）增加超时保护，防止 apt 锁被占用时扫描请求永久阻塞导致仪表盘"全面扫描"和单模块扫描无响应
  - 新增 `command_output_with_timeout()` 工具函数，子进程超时自动 kill 并回收
  - 默认超时 10 秒，apt 相关命令 30 秒，超时后优雅降级继续扫描后续项

## [0.3.0] - 2026-06-15

### 新增 (Added)
- **deb 安装包支持**: 新增 `build-deb.sh` 一键打包脚本，支持 amd64/arm64 架构及交叉编译
- **GitHub Release**: v0.3.0 发布 amd64/arm64 两个 deb 包，客户可直接下载安装
- **Anthropic Claude 支持**: 新增 `AnthropicProvider`，支持 Anthropic Messages API 原生协议（流式输出 + Function Calling）
- **Web 端流式输出**: WebSocket 支持 `stream_start`/`stream_chunk`/`stream_end` 消息类型，AI 回复逐 token 实时显示
- **Web 端 Markdown 渲染**: 内联 `renderMd()` 解析器，支持代码块、粗体、斜体、有序/无序列表、标题、链接、表格、引用
- **Web 端工具调用可视化**: 黄色 spinner 显示工具执行状态，扫描结果支持折叠/展开
- **Web 端打字光标动画**: 流式输出时显示 `▊` 光标闪烁动画
- **CLI 思考状态提示**: `chat_with_tools` 调用期间显示 `⠋ 正在思考...` spinner
- **CLI 上下文管理命令**: `clear`/`清屏`/`重置` 重置对话，`history`/`历史` 查看对话摘要
- **CLI 错误恢复机制**: 流式输出失败时自动回退到非流式批量输出

### 改进 (Changed)
- **CLI 流式输出全覆盖**: 普通回复、工具调用后回复、hybrid 回退路径全部走流式输出
- **CLI 统一输出格式**: 所有回复路径使用 `🤖 助手:` 前缀 + 空行分隔，视觉体验一致
- **CLI 辅助函数重构**: 提取 `stream_llm_response()` 统一流式输出逻辑，消除重复代码
- **Web 端后端架构**: `stream_to_socket()` 使用 `tokio::sync::mpsc` + `std::sync::mpsc` 双层 channel 桥接同步回调与异步 WebSocket
- **Web 端非工具调用路径**: 从非流式改为流式输出（需两次 LLM 调用）

### 修复 (Fixed)
- **deb 包依赖问题**: 移除 procps/coreutils 硬依赖，改为 Recommends（缺失不崩溃，自动跳过检测项）
- **CLI 消息恢复错误**: 修复工具调用流式失败时 `messages.pop()` 删除错误消息的问题，改用 `messages.truncate(checkpoint)` 正确恢复
- **CLI 上下文裁剪断裂**: 修复裁剪时可能切断工具调用组（assistant(tool_calls) + tool_results）的问题
- **Web 端有序列表渲染**: 修复有序列表被错误包裹在 `<ul>` 中的 bug，采用逐行分组算法分离处理

## [0.2.0] - 2026-06-12

### 新增 (Added)
- **AI 问答流式输出**: 支持 SSE 流式响应，逐字显示 AI 回复
- **Spinner 动画组件**: 工具调用时显示 Braille 字符动画
- **终端 Markdown 渲染**: 支持代码块、粗体、斜体、列表、标题、链接
- **安装脚本日志系统**: 安装过程记录到 `/var/log/kylin-doctor-install.log`
- **安装失败提示**: 错误时显示日志位置和常见问题解决方案
- **依赖冲突自动修复**: `--fix-deps` 选项自动处理 libssl-dev 版本冲突
- **默认配置文件**: 安装时自动创建 `~/.kylin-doctor/config.toml` 并提供详细注释

### 改进 (Changed)
- **AI 问答体验**: 移除调试输出，使用 ✅/❌ 图标表示操作状态
- **安装脚本**: 新增 `run_cmd`/`run_cmd_warn` 函数，命令输出同时记录到日志
- **默认模型**: 从 qwen2.5:1.5b 改为 qwen2.5:3b（平衡速度和质量）

### 修复 (Fixed)
- **安装脚本静默退出**: 修复 curl 下载失败时脚本无提示退出的问题
- **错误处理机制**: 添加 ERR trap 显示具体错误行号、命令和解决方案
- **网络超时处理**: curl 添加 --connect-timeout 和 --retry 参数
- 清理编译 warnings

## [0.1.0] - 2026-06-02

### 新增 (Added)
- **系统检测模块**: CPU、内存、磁盘、进程、PCI 设备、USB 设备、SMART 等
- **CLI 命令**: `scan`、`fix`、`report`、`chat`、`knowledge`、`serve`
- **Web 仪表盘**: 基于 Axum 的 Web 服务，提供实时监控和 AI 对话
- **AI 对话功能**: 集成 Ollama 本地模型和云端 API（通义千问、DeepSeek、月之暗面）
- **知识库系统**: RAG 检索增强，支持文档导入和语义搜索
- **诊断报告**: 支持 HTML、JSON、Markdown 格式输出
- **一键安装脚本**: 支持自动安装依赖、Rust 工具链、Ollama
- **一键卸载脚本**: 支持清理程序文件、配置、Ollama
- **部署文档**: 包含源码编译、systemd 服务、Docker 部署方式

[0.3.5]: https://github.com/fanwenzhu/kylin-doctor/compare/v0.3.4...v0.3.5
[0.3.4]: https://github.com/fanwenzhu/kylin-doctor/compare/v0.3.3...v0.3.4
[0.3.3]: https://github.com/fanwenzhu/kylin-doctor/compare/v0.3.2...v0.3.3
[0.3.2]: https://github.com/fanwenzhu/kylin-doctor/compare/v0.3.1...v0.3.2
[0.3.1]: https://github.com/fanwenzhu/kylin-doctor/compare/v0.3.0...v0.3.1
[0.3.0]: https://github.com/fanwenzhu/kylin-doctor/compare/v0.2.0...v0.3.0
[0.2.0]: https://github.com/fanwenzhu/kylin-doctor/compare/v0.1.0...v0.2.0
[0.1.0]: https://github.com/fanwenzhu/kylin-doctor/releases/tag/v0.1.0
