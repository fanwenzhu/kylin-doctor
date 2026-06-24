# Changelog

本文件记录 kylin-doctor 的所有重要变更。

格式基于 [Keep a Changelog](https://keepachangelog.com/zh-CN/1.0.0/)，
版本号遵循 [语义化版本](https://semver.org/lang/zh-CN/)。

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

[0.3.1]: https://github.com/fanwenzhu/kylin-doctor/compare/v0.3.0...v0.3.1
[0.3.0]: https://github.com/fanwenzhu/kylin-doctor/compare/v0.2.0...v0.3.0
[0.2.0]: https://github.com/fanwenzhu/kylin-doctor/compare/v0.1.0...v0.2.0
[0.1.0]: https://github.com/fanwenzhu/kylin-doctor/releases/tag/v0.1.0
