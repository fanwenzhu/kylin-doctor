# kylin-doctor

银河麒麟桌面系统自我诊断工具。

## 功能特性

- **全面诊断** — 硬件、系统、软件、安全、性能五大维度
- **分层输出** — 普通用户 / 管理员 / 开发者三种详细程度
- **AI 增强** — 接入本地大模型（Ollama），智能分析问题根因
- **自动修复** — 检测到问题后自动修复，执行前确认
- **双界面** — CLI 命令行 + Web 仪表盘

## 快速开始

### 一键安装

```bash
# 基础安装（编译 + 安装到 /usr/local/bin）
curl -fsSL https://raw.githubusercontent.com/fanwenzhu/kylin-doctor/master/install.sh | sudo bash

# 安装并自动配置 AI 模型（Ollama + qwen2.5:3b）
curl -fsSL https://raw.githubusercontent.com/fanwenzhu/kylin-doctor/master/install.sh | sudo bash -s -- --with-ollama

# 或克隆后手动安装
git clone https://github.com/fanwenzhu/kylin-doctor.git
cd kylin-doctor
sudo ./install.sh
```

### 卸载

```bash
curl -fsSL https://raw.githubusercontent.com/fanwenzhu/kylin-doctor/master/uninstall.sh | sudo bash
```

### 从源码构建

```bash
cargo build --release
```

### CLI 使用

```bash
# 全面扫描
kylin-doctor scan

# 只扫描系统模块
kylin-doctor scan --module system

# 快速扫描（跳过耗时项）
kylin-doctor scan --quick

# 修复问题
kylin-doctor fix                  # 扫描并修复所有问题
kylin-doctor fix --dry-run        # 预览修复，不执行
kylin-doctor fix --yes            # 跳过确认
kylin-doctor fix --auto-only      # 只修复可自动修复的
kylin-doctor fix --critical-only  # 只修复严重问题
kylin-doctor fix --module security # 只修复安全模块

# 生成诊断报告
kylin-doctor report                           # JSON 输出到 stdout
kylin-doctor report --format html             # HTML 输出
kylin-doctor report --output report.html      # 保存到文件

# AI 对话（支持 Function Calling 自动诊断）
kylin-doctor chat
kylin-doctor chat 我的系统为什么变慢了

# 混合模式（本地优先，失败回退云端）
kylin-doctor --provider hybrid chat

# 知识库管理
kylin-doctor knowledge add ./docs/ --recursive
kylin-doctor knowledge embed
kylin-doctor knowledge test "如何配置打印机"
```

### Web 仪表盘

```bash
# 启动 Web 服务
kylin-doctor serve
kylin-doctor serve --port 9090

# 浏览器打开 http://127.0.0.1:8080
```

### 输出示例

```
🔍 kylin-doctor 系统诊断

├── [████████████████████] 1/1 正在扫描 system...
└── 扫描完成

📋 system [⚠️  2 个警告]
   扫描耗时: 12ms
   ⚠️  [system-disk-warning-_home] /home 磁盘空间偏高 (85%)
      💡 建议: 清理临时文件

   ⚠️  [system-zombie-processes] 发现 2 个僵尸进程

━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
⚠️  警告: 2  ℹ️  信息: 0
系统基本正常，但有警告项需要关注。
```

## 项目结构

```
kylin-doctor/
├── Cargo.toml
├── crates/
│   ├── kylin-doctor-core/    # 核心检测逻辑 + LLM 集成
│   ├── kylin-doctor-cli/     # CLI 命令行工具
│   └── kylin-doctor-web/     # Web 仪表盘
└── docs/                     # 设计文档
```

## 技术栈

| 组件 | 技术 |
|------|------|
| CLI 框架 | clap (Rust) |
| Web 后端 | Axum |
| Web 前端 | 单文件 HTML（内嵌 CSS/JS） |
| 本地模型 | Ollama + Qwen2.5 |
| 云端模型 | OpenAI 兼容 API（Qwen/DeepSeek/Moonshot） |
| 系统检测 | Shell 命令 + /proc |

## 路线图

- [x] 第一阶段：项目骨架 + 系统检测模块 + CLI scan 命令
- [x] 第二阶段：硬件检测模块 (hardware.rs)
- [x] 第三阶段：软件生态模块 (software.rs)
- [x] 第四阶段：安全审计模块 (security.rs)
- [x] 第五阶段：性能分析模块 (performance.rs)
- [x] 第六阶段：AI 集成 (Ollama + 云端模型)
- [x] 第七阶段：Web 仪表盘

## 文档

- **[部署文档](docs/DEPLOYMENT.md)** — 环境要求、构建安装、配置详解、Web 部署、AI 模型配置、系统服务、Docker、故障排查
- **[使用说明](docs/USAGE.md)** — CLI 命令详解、Web 仪表盘操作、诊断模块说明、AI 助手使用、常见场景、最佳实践

## License

MIT
